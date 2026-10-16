//! Tessera's bits on every shape, plan and line set on its own -- a
//! family's total can hide a shape that costs more than it should.

use crate::diagnostics::measured::MeasuredBitmaps;
use crate::diagnostics::RAW_CELLS;
use crate::Tessera;
use crate::corpus::{LINE_SETS, CITY_PLANS, GROWN_SHAPES, SPARSE_SHAPES};
use utilities::diagnostics::table::report::Report;
use utilities::diagnostics::table::Table;
use bitmap::Bitmap;

/// Prints Tessera's bits on every shape, sparse shape, plan and line set,
/// each on its own row.
pub fn run_per_shape_tool(report: &mut Report) {
    let mut tessera = Tessera::new();
    let mut table = Table::new(&["corpus", "bitmaps", "cells set\na bitmap", "Tessera\nbits a bitmap", "Tessera bits\na cell set", "of the\nraw cells"]);
    let mut measure_row = |row_name: &str, bitmaps: Vec<Bitmap>| {
        let measured_bitmaps = MeasuredBitmaps::measure_bitmaps(&mut tessera, bitmaps);
        assert!(measured_bitmaps.lost.is_empty(), "{row_name}: Tessera lost cells of cases {:?}", measured_bitmaps.lost);
        let bitmaps = measured_bitmaps.bitmaps;
        table.add_row(&[
            row_name.to_string(),
            bitmaps.to_string(),
            (measured_bitmaps.cells_set / bitmaps).to_string(),
            (measured_bitmaps.total_bits / bitmaps).to_string(),
            format!("{:.2}", measured_bitmaps.total_bits as f64 / measured_bitmaps.cells_set.max(1) as f64),
            format!("{:.1}%", 100.0 * (measured_bitmaps.total_bits / bitmaps) as f64 / RAW_CELLS as f64),
        ]);
    };
    for grown_shape in GROWN_SHAPES.iter().chain(&SPARSE_SHAPES) {
        measure_row(grown_shape.display_name, grown_shape.timed_bitmaps().collect());
    }
    for city_plan in &CITY_PLANS {
        measure_row(city_plan.display_name, city_plan.timed_bitmaps().collect());
    }
    for line_set in &LINE_SETS {
        measure_row(line_set.display_name, line_set.timed_bitmaps().collect());
    }
    report.add_table("every shape, plan and line set", table);
}
