//! Instructions to encode and to decode a corpus, counted exactly by
//! callgrind, on one seed; and the corpus alone, for any profiler
//! (`docs/testing_protocol.md`, "Speed: instructions and time").

use std::path::Path;
use std::process::Command;
use crate::diagnostics::adversarial::worst;
use crate::diagnostics::examination::first_difference;
use crate::diagnostics::RAW_CELLS;
use crate::transient_data;
use crate::BitStream;
use crate::Tessera;
use crate::corpus::checkerboards::one_checkerboard;
use crate::corpus::seed::seed_in_use;
use crate::corpus::{corpus_families, grown_bitmaps, corpus_seed, HowMany};
use utilities::commands::GivenCommand;
use utilities::diagnostics::table::report::Report;
use utilities::diagnostics::table::Table;
use bitmap::Bitmap;

/// Bitmaps each generator makes: the first of those the timed corpus
/// takes, so the counts weigh each family as the times do.
const BITMAPS_PER_GENERATOR: u64 = 5;

/// The checkerboard in the corpus: odd squares, so nothing lines up.
const CHECKERBOARD_SQUARE: u8 = 7;
/// Noise in the corpus: half the cells, scattered...
const NOISE_DENSITY: f64 = 0.5;
/// ...one bitmap of it.
const NOISE_BITMAPS: u64 = 1;

/// The tool that runs the corpus alone, for callgrind to count.
pub const CORPUS_TOOL: &str = "instruction_corpus";

/// The variable the corpus' seed is pinned by, for both runs.
const SEED_VARIABLE: &str = utilities::seed::SEED_VARIABLE;

/// The two parts counted: a name, and the function callgrind collects
/// inside, every call counted whole.
// Methods as the profilers name them: `<type>::method`.
const PARTS: [(&str, &str); 2] = [("encode", "<tessera::Tessera>::encode"), ("decode", "<tessera::Tessera>::decode")];

/// The fixed corpus.
fn corpus() -> Vec<Bitmap> {
    let mut corpus: Vec<Bitmap> = corpus_families(HowMany::SameCountEach(BITMAPS_PER_GENERATOR)).into_iter().flat_map(|(_, bitmaps)| bitmaps).collect();
    corpus.push(one_checkerboard(CHECKERBOARD_SQUARE));
    corpus.extend(worst::every_saved().into_iter().map(|(_, bitmap)| bitmap));
    corpus.extend(grown_bitmaps(corpus_seed(), NOISE_DENSITY, 0.0, NOISE_BITMAPS));
    corpus
}

/// Encodes and decodes every bitmap of the corpus, in one Tessera, checking
/// each round trips: what callgrind counts.
pub fn run_corpus() {
    let corpus = corpus();
    // One `Tessera`, stream and bitmap for the whole corpus, as a caller
    // encoding many would keep them.
    let (mut tessera, mut stream, mut decoded_bitmap) = (Tessera::new(), BitStream::default(), Bitmap::new());
    for bitmap in &corpus {
        tessera.encode(bitmap, &mut stream);
        tessera.decode(&stream, &mut decoded_bitmap);
        assert_eq!(first_difference(bitmap, &decoded_bitmap), None, "a bitmap did not round trip");
    }
}

/// Counts the corpus' encoding and decoding instructions under
/// callgrind, and reports them.
pub fn run_instruction_count_tool(report: &mut Report, given_parameters: &GivenCommand) -> Result<(), String> {
    // The corpus built here settles the seed, counted as this run's use;
    // both callgrind runs are pinned to it.
    let bitmaps = corpus().len();
    let (seed, _) = seed_in_use().expect("the corpus settled a seed");
    let folder = transient_data::callgrind_folder();
    std::fs::create_dir_all(&folder).expect("the output folder made");
    let this_tool_path = std::env::current_exe().expect("this tool's own path");

    let mut table = Table::new(&["part", "bitmaps", "instructions", "a bitmap", "a cell"]);
    for (part, collected_function) in PARTS {
        let output_path = folder.join(format!("callgrind.{part}.out"));
        let instructions = count(&this_tool_path, given_parameters, collected_function, &output_path, seed);
        let a_bitmap = instructions / bitmaps as u64;
        table.add_row(&[
            part.to_string(),
            bitmaps.to_string(),
            instructions.to_string(),
            a_bitmap.to_string(),
            format!("{:.1}", a_bitmap as f64 / RAW_CELLS as f64),
        ]);
    }
    report.add_note(format!(
        "{BITMAPS_PER_GENERATOR} bitmaps a generator, a checkerboard, the saved adversarial bitmaps and noise; \
         counted by callgrind, output in transient_data/callgrind/"
    ));
    report.add_table("instructions", table);
    Ok(())
}

/// The instructions callgrind counts inside `collected_function` while `tool_path` runs
/// the corpus on `seed`, reached as `given_parameters` was, its output kept at `output_path`.
fn count(tool_path: &Path, given_parameters: &GivenCommand, collected_function: &str, output_path: &Path, seed: u64) -> u64 {
    let callgrind_run = Command::new("valgrind")
        .arg("--tool=callgrind")
        .arg(format!("--callgrind-out-file={}", output_path.display()))
        .arg(format!("--toggle-collect={collected_function}"))
        .arg(tool_path)
        // Reached as this tool was: the program's own words before it.
        .args(given_parameters.route_words())
        .arg(CORPUS_TOOL)
        .env(SEED_VARIABLE, utilities::seed::seed_as_hex(seed))
        .output()
        .expect("valgrind runs: it must be installed (apt-get install valgrind)");
    assert!(callgrind_run.status.success(), "callgrind's run failed:\n{}", String::from_utf8_lossy(&callgrind_run.stderr));
    let callgrind_output = std::fs::read_to_string(output_path).expect("callgrind's output read");
    // Callgrind's output ends with the events it collected in all.
    callgrind_output
        .lines()
        .find_map(|line| line.strip_prefix("totals:").or_else(|| line.strip_prefix("summary:")))
        .and_then(|total_text| total_text.trim().parse().ok())
        .expect("callgrind's output names its total")
}
