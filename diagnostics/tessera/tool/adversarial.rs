//! Adversarial bitmaps against the raw cells: the search scored by what
//! Tessera costs beyond the raw cells, four searches at once, and a
//! worst bitmap saved under a name
//! (`docs/testing_protocol.md`, "Adversarial searches").

use super::super::adversarial::{worst, search_at_once, SearchEffort, BitmapScore, SEARCHES_AT_ONCE};
use crate::diagnostics::examination::BitmapExamination;
use crate::encode_bitmap;
use crate::BitStream;
use crate::tile::{cells_in_tile, Tile};
use crate::Tessera;
use crate::corpus::corpus_seed;
use crate::transient_data;
use utilities::commands::GivenCommand;
use utilities::diagnostics::table::report::Report;
use utilities::diagnostics::table::Table;
use bitmap::Bitmap;

/// The parameter: how many changes each search tries on the whole plane from each start.
pub const CHANGES_PARAMETER: &str = "changes a plane start";
/// The parameter: the worst bitmap saved.
pub const FROM_PARAMETER: &str = "worst";
/// The parameter: what it is saved as.
pub const NAME_PARAMETER: &str = "saved name";
/// The parameter: a line describing it.
pub const DESCRIPTION_PARAMETER: &str = "description";

/// Copies the worst bitmap named to the saved bitmap named, described.
pub fn save(given_parameters: &GivenCommand) -> Result<(), String> {
    let (worst_name, saved_name, saved_description) = (given_parameters.argument_text(FROM_PARAMETER)?, given_parameters.argument_text(NAME_PARAMETER)?, given_parameters.argument_text(DESCRIPTION_PARAMETER)?);
    let bitmap = worst::read_worst(worst_name).ok_or_else(|| format!("no worst bitmap named {worst_name}"))?;
    let mut comment_notes = vec![format!("{saved_name}: {saved_description}")];
    comment_notes.extend(worst::notes_from(&worst::path(worst_name)).into_iter().map(|worst_note| format!("from worst {worst_note}")));
    worst::save(saved_name, &bitmap, &comment_notes);
    let mut table =
        Table::new(&["saved", "from worst", "cells\nset", "file", "description"]).left_aligned(&["from worst", "file", "description"]);
    let saved_file = worst::saved_path(saved_name);
    let relative_file = saved_file.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap_or(&saved_file);
    table.add_row(&[saved_name, worst_name, &bitmap.count_set().to_string(), &relative_file.display().to_string(), saved_description]);
    table.print_rendered();
    Ok(())
}

/// What the worst bitmap is kept under.
const WORST: &str = "against_raw";

/// What the search maximizes. Not Tessera's bits alone -- noise maximizes
/// those for any encoder, and says nothing -- but what Tessera costs beyond
/// the raw cells of `area`, the area searched.
fn score_against_raw(bitmap: &Bitmap, area: Tile) -> BitmapScore {
    let tessera_bits = encode_bitmap(bitmap).len() as u64;
    BitmapScore { gap: tessera_bits as i64 - cells_in_tile(area.tile_level) as i64, tessera_bits }
}

/// Runs the searches in parallel, keeps the worst bitmap if it beats the
/// one kept, and reports what each search found and what the worst kept
/// is now.
pub fn run_adversarial_tool(given_parameters: &GivenCommand) -> Result<(), String> {
    let seed = corpus_seed();
    let search_effort = SearchEffort { whole_bitmap: given_parameters.argument_number(CHANGES_PARAMETER)?, ..SearchEffort::default() };
    let kept_worst = worst::read_worst(WORST);
    let search_outcomes = search_at_once(seed, kept_worst.clone(), search_effort, &|| score_against_raw);

    let mut table = Table::new(&[
        "search",
        "worst window\ngap, bits",
        "window\nfrom",
        "worst plane\ngap, bits",
        "plane\nfrom",
        "Tessera\nbits",
    ])
    .left_aligned(&["window\nfrom", "plane\nfrom"]);
    for (search_index, search_outcome) in search_outcomes.iter().enumerate() {
        table.add_row(&[
            search_index.to_string(),
            search_outcome.window.bitmap_score.gap.to_string(),
            search_outcome.window_from.to_string(),
            search_outcome.worst.bitmap_score.gap.to_string(),
            search_outcome.worst_from.to_string(),
            search_outcome.worst.bitmap_score.tessera_bits.to_string(),
        ]);
    }
    let mut report = Report::new(given_parameters.command_name(), &given_parameters.resolved_line());
    report.add_note(format!(
        "{SEARCHES_AT_ONCE} searches, {} changes a window start, {} a plane start; gap: Tessera's bits less the raw cells searched",
        search_effort.window, search_effort.whole_bitmap
    ));
    report.add_table("the searches", table);

    let worst = search_outcomes.iter().map(|search_outcome| &search_outcome.worst).max_by_key(|found_worst| found_worst.bitmap_score.gap).expect("a search");
    let kept_gap = kept_worst.map(|bitmap| score_against_raw(&bitmap, Tile::WHOLE_BITMAP).gap);
    let kept_beaten = kept_gap.is_none_or(|gap| worst.bitmap_score.gap > gap);
    if kept_beaten {
        worst::write(WORST, &worst.bitmap, &format!("{WORST}: gap {} bits", worst.bitmap_score.gap));
    }
    let bitmap = worst::read_worst(WORST).expect("kept");
    let bitmap_examination = BitmapExamination::examine_bitmap(&mut Tessera::new(), &mut BitStream::default(), &mut Bitmap::new(), &bitmap);
    assert_eq!(bitmap_examination.first_difference, None, "{WORST} does not round trip");

    let kept_gap_now = if kept_beaten { worst.bitmap_score.gap } else { kept_gap.expect("a worst bitmap not beaten is there") };
    let mut table = Table::new(&["worst", "worst gap\nthis run", "kept gap\nbefore", "kept gap\nnow", "replaced"]);
    table.add_row(&[
        WORST.to_string(),
        worst.bitmap_score.gap.to_string(),
        kept_gap.map_or("none".to_string(), |gap| gap.to_string()),
        kept_gap_now.to_string(),
        if kept_beaten { "yes" } else { "no" }.to_string(),
    ]);
    report.add_table("the worst kept, which round trips", table);
    transient_data::publish_report(report);
    Ok(())
}
