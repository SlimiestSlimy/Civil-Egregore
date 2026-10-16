//! What Tessera spends: bits a bitmap from every corpus generator, one
//! table a generator and one row a parameter set, then what its trees
//! are made of, family by family.

use crate::diagnostics::adversarial::worst;
use crate::diagnostics::measured::MeasuredBitmaps;
use crate::diagnostics::tree_stats::TreeStats;
use crate::diagnostics::RAW_CELLS;
use crate::BitStream;
use crate::Tessera;
use crate::corpus::checkerboards::every_checkerboard;
use crate::corpus::{corpus_families, HowMany, LINE_SETS, CITY_PLANS, GROWN_SHAPES, SPARSE_SHAPES};
use utilities::diagnostics::table::report::Report;
use utilities::diagnostics::table::Table;
use bitmap::Bitmap;

/// `part` as a percentage of `whole_amount`; 0 of nothing.
fn percent_of(part: usize, whole_amount: usize) -> f64 {
    if whole_amount == 0 {
        0.0
    } else {
        100.0 * part as f64 / whole_amount as f64
    }
}

/// `measured_bitmaps`'s row, after the parameter set's name and parameters.
fn measured_row(measured_bitmaps: &MeasuredBitmaps, set_name: &str, parameter_values: &str) -> Vec<String> {
    let bitmaps = measured_bitmaps.bitmaps.max(1);
    vec![
        set_name.to_string(),
        parameter_values.to_string(),
        measured_bitmaps.bitmaps.to_string(),
        (measured_bitmaps.cells_set / bitmaps).to_string(),
        (measured_bitmaps.total_bits / bitmaps).to_string(),
        measured_bitmaps.fewest_bits.to_string(),
        measured_bitmaps.most_bits.to_string(),
        format!("{:.1}%", percent_of(measured_bitmaps.total_bits / bitmaps, RAW_CELLS)),
        (measured_bitmaps.encode_micros / bitmaps as u128).to_string(),
    ]
}

/// One generator's table, added to `report` under its name: a row a
/// parameter set, then their total.
fn generator_table(
    tessera: &mut Tessera,
    report: &mut Report,
    generator_name: &str,
    parameter_names: &str,
    parameter_sets: Vec<(String, String, Vec<Bitmap>)>,
) {
    let mut table = Table::new(&[
        generator_name,
        parameter_names,
        "bitmaps",
        "cells set\na bitmap",
        "Tessera bits\na bitmap",
        "fewest",
        "most",
        "of the\nraw cells",
        "encode us\na bitmap",
    ]);
    let mut all_measured = MeasuredBitmaps::default();
    for (set_name, parameter_values, bitmaps) in parameter_sets {
        let measured_bitmaps = MeasuredBitmaps::measure_bitmaps(tessera, bitmaps);
        assert!(measured_bitmaps.lost.is_empty(), "{set_name}: Tessera lost cells of cases {:?}", measured_bitmaps.lost);
        table.add_row(&measured_row(&measured_bitmaps, &set_name, &parameter_values));
        all_measured.add_measured(&measured_bitmaps);
    }
    table.divider();
    table.add_row(&measured_row(&all_measured, "all", ""));
    report.add_table(generator_name, table);
}

/// Adds one table a corpus generator -- grown, laid out as a city,
/// drawn with lines, checkerboards -- with a row a parameter set, then a
/// row for each saved adversarial bitmap, then what Tessera's trees hold,
/// family by family.
pub fn run_measurement_tool(report: &mut Report) {
    let mut tessera = Tessera::new();
    let grown_sets = GROWN_SHAPES
        .iter()
        .chain(&SPARSE_SHAPES)
        .map(|grown_shape| {
            let parameter_values = format!("{} density, {} cluster", grown_shape.cell_density, grown_shape.cluster_weight);
            (grown_shape.display_name.to_string(), parameter_values, grown_shape.timed_bitmaps().collect())
        })
        .collect();
    let city_sets = CITY_PLANS
        .iter()
        .map(|city_plan| {
            let parameter_values = format!("pitch {}, street {}, {} courtyards", city_plan.street_pitch, city_plan.street_width, city_plan.courtyards_per_block);
            (city_plan.display_name.to_string(), parameter_values, city_plan.timed_bitmaps().collect())
        })
        .collect();
    let lines = LINE_SETS
        .iter()
        .map(|line_set| (line_set.display_name.to_string(), format!("{} lines", line_set.lines), line_set.timed_bitmaps().collect()))
        .collect();
    let checkerboard_sets = every_checkerboard()
        .map(|(square_side, bitmap)| (format!("{square_side}x{square_side} squares"), format!("side {square_side}"), vec![bitmap]))
        .collect();
    let adversarial_sets = worst::every_saved().into_iter().map(|(saved_name, bitmap)| (saved_name, "saved".to_string(), vec![bitmap])).collect();
    generator_table(&mut tessera, report, "grown", "density, cluster", grown_sets);
    generator_table(&mut tessera, report, "laid out like a city", "pitch, street, courtyards", city_sets);
    generator_table(&mut tessera, report, "drawn with lines", "lines", lines);
    generator_table(&mut tessera, report, "checkerboard", "square side", checkerboard_sets);
    generator_table(&mut tessera, report, "adversarial, saved", "", adversarial_sets);
    add_structure(&mut tessera, report);
}

/// What Tessera's trees hold, family by family -- every bitmap's tree, even
/// where the stream is its binary count tree: how many streams are,
/// complex tiles, and nodes naming children.
fn add_structure(tessera: &mut Tessera, report: &mut Report) {
    let mut structure_table = Table::new(&[
        "family",
        "binary count\ntree streams",
        "complex tiles\na bitmap",
        "payload values\na complex tile",
        "tiles\na bitmap",
        "copies naming\nchildren a bitmap",
        "flipping divides\na bitmap",
        "cell lists\na bitmap",
    ]);
    let mut stream = BitStream::default();
    for (family_name, maps) in corpus_families(HowMany::TimedCount) {
        let (mut tree_stats, mut binary_count_trees) = (TreeStats::default(), 0);
        for bitmap in &maps {
            tessera.encode(bitmap, &mut stream);
            binary_count_trees += (stream.words()[0] & 1 == 1) as usize;
            tree_stats.add_counts(&TreeStats::measure_tree(tessera.tree()));
        }

        let bitmaps = maps.len();
        let per_bitmap = |count: usize| format!("{:.1}", count as f64 / bitmaps as f64);
        let share_of = |part: usize, whole_amount: usize| format!("{:.2}%", percent_of(part, whole_amount));
        let row_name = format!("{family_name}, {bitmaps} bitmaps");
        structure_table.add_row(&[
            row_name,
            share_of(binary_count_trees, bitmaps),
            per_bitmap(tree_stats.complex_tiles),
            format!("{:.1}", tree_stats.payload_values as f64 / tree_stats.complex_tiles.max(1) as f64),
            per_bitmap(tree_stats.tiles),
            per_bitmap(tree_stats.copies_naming_children),
            per_bitmap(tree_stats.flipping_divides),
            per_bitmap(tree_stats.cell_lists),
        ]);
    }

    report.add_table("what the trees hold", structure_table);
}
