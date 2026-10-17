//! Wall-clock time to encode and to decode, averaged over a large
//! corpus built before any of it is timed
//! (`docs/testing_protocol.md`, "Speed: instructions and time").

use crate::diagnostics::adversarial::worst;
use crate::diagnostics::examination::first_difference;
use crate::BitStream;
use crate::Tessera;
use crate::corpus::{corpus_families, HowMany};
use utilities::commands::GivenCommand;
use utilities::diagnostics::table::report::Report;
use utilities::diagnostics::table::Table;
use bitmap::Bitmap;
use std::time::{Duration, Instant};

/// Times each saved adversarial bitmap is encoded: they are few, so each
/// is timed often enough to average.
const SAVED_REPEATS: usize = 20;

/// Percentile reported as the tail, besides the worst.
const TAIL_PERCENT: usize = 90;

/// The median's percentile.
const MEDIAN_PERCENT: usize = 50;

/// The parameter: bitmaps grown from each generator.
pub const PER_GENERATOR: &str = "bitmaps a generator";

/// Builds the corpus, times encoding every bitmap of it once, then
/// decoding, and reports both a family at a time.
pub fn run_timing_tool(report: &mut Report, given_parameters: &GivenCommand) -> Result<(), String> {
    let per_generator: u64 = given_parameters.argument_number(PER_GENERATOR)?;
    let timed_families = corpus_families(HowMany::SameCountEach(per_generator));

    let (mut tessera, mut stream, mut decoded_bitmap) = (Tessera::new(), BitStream::default(), Bitmap::new());
    // One encode first, so the first timed one pays for nothing extra.
    tessera.encode(&timed_families[0].1[0], &mut stream);

    let tail_heading = format!("p{TAIL_PERCENT} us");
    let mut table = Table::new(&["family", "bitmaps", "mean us", "median us", &tail_heading, "max us", "decode us"]);
    let (mut every_encode, mut every_decode) = (Vec::new(), Vec::new());
    for (family_name, bitmaps) in &timed_families {
        let (mut encode_times, decode_times) = time_family(&mut tessera, &mut stream, &mut decoded_bitmap, family_name, bitmaps);
        table.add_row(&timing_row(family_name, &mut encode_times, &decode_times));
        every_encode.extend(encode_times);
        every_decode.extend(decode_times);
    }
    table.divider();
    table.add_row(&timing_row("all", &mut every_encode, &every_decode));

    // The saved adversarial bitmaps, apart from the corpus: few, and
    // each among the worst found against one encoder, so each is encoded
    // several times.
    let saved_bitmaps = worst::every_saved();
    let repeated_bitmaps: Vec<Bitmap> = (0..SAVED_REPEATS).flat_map(|_| saved_bitmaps.iter().map(|(_, bitmap)| bitmap.clone())).collect();
    let saved_family_name = "adversarial, saved";
    let (mut encode_times, decode_times) = time_family(&mut tessera, &mut stream, &mut decoded_bitmap, saved_family_name, &repeated_bitmaps);
    table.divider();
    table.add_row(&timing_row(saved_family_name, &mut encode_times, &decode_times));
    report.add_note(format!("{per_generator} bitmaps a generator, the saved adversarial bitmaps {SAVED_REPEATS} times each"));
    report.add_table("encode and decode times", table);
    Ok(())
}

/// Encodes every bitmap of `bitmaps`, the family `family_name`, once, timing
/// each, then decodes each stream, timing each and checking it round
/// trips: the encode times and the decode times.
fn time_family(tessera: &mut Tessera, stream: &mut BitStream, decoded_bitmap: &mut Bitmap, family_name: &str, bitmaps: &[Bitmap]) -> (Vec<Duration>, Vec<Duration>) {
    let mut encode_times = Vec::with_capacity(bitmaps.len());
    let mut streams = Vec::with_capacity(bitmaps.len());
    for bitmap in bitmaps {
        let encode_start = Instant::now();
        tessera.encode(bitmap, stream);
        encode_times.push(encode_start.elapsed());
        streams.push(stream.clone());
    }
    let mut decode_times = Vec::with_capacity(streams.len());
    for (encoded_stream, bitmap) in streams.iter().zip(bitmaps) {
        let decode_start = Instant::now();
        tessera.decode(encoded_stream, decoded_bitmap);
        decode_times.push(decode_start.elapsed());
        assert_eq!(first_difference(decoded_bitmap, bitmap), None, "{family_name} did not round trip");
    }
    (encode_times, decode_times)
}

/// One row: how many, the encode times' mean, median, tail and worst,
/// and the decode times' mean, in microseconds.
fn timing_row(family_name: &str, encode_times: &mut [Duration], decode_times: &[Duration]) -> Vec<String> {
    encode_times.sort_unstable();
    let to_micros = |time_taken: Duration| time_taken.as_secs_f64() * 1e6;
    let mean_micros = |family_times: &[Duration]| family_times.iter().map(|&one_time| to_micros(one_time)).sum::<f64>() / family_times.len() as f64;
    let at_percent = |time_percentile: usize| to_micros(encode_times[(encode_times.len() - 1) * time_percentile / 100]);
    vec![
        family_name.to_string(),
        encode_times.len().to_string(),
        format!("{:.1}", mean_micros(encode_times)),
        format!("{:.1}", at_percent(MEDIAN_PERCENT)),
        format!("{:.1}", at_percent(TAIL_PERCENT)),
        format!("{:.1}", to_micros(encode_times[encode_times.len() - 1])),
        format!("{:.1}", mean_micros(decode_times)),
    ]
}
