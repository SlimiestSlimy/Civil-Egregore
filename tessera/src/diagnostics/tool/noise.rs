//! Tessera's bits on noise -- cells set at random, scattered -- at several
//! densities, against the raw cells: what Tessera pays where nothing
//! compresses, or little does.

use crate::diagnostics::measured::MeasuredBitmaps;
use crate::diagnostics::RAW_CELLS;
use crate::Tessera;
use crate::corpus::{grown_bitmaps, corpus_seed};
use utilities::diagnostics::table::report::Report;
use utilities::diagnostics::table::Table;

/// The densities looked at, from all but incompressible to half.
const NOISE_DENSITIES: [f64; 4] = [0.5, 0.35, 0.2, 0.1];

/// Bitmaps a density: noise is noise, a few say it.
const BITMAPS_PER_DENSITY: u64 = 3;

/// Prints Tessera's bits on noise at every density, against the raw cells.
pub fn run_noise_tool(report: &mut Report) {
    let mut tessera = Tessera::new();
    let mut table = Table::new(&["density", "Tessera\nbits a bitmap", "Tessera\nover raw cells"]);
    for noise_density in NOISE_DENSITIES {
        let measured_bitmaps = MeasuredBitmaps::measure_bitmaps(&mut tessera, grown_bitmaps(corpus_seed(), noise_density, 0.0, BITMAPS_PER_DENSITY));
        assert!(measured_bitmaps.lost.is_empty(), "noise at {noise_density}: Tessera lost cells of cases {:?}", measured_bitmaps.lost);
        let tessera_bits = measured_bitmaps.total_bits / measured_bitmaps.bitmaps;
        table.add_row(&[
            format!("{:.0}%", noise_density * 100.0),
            tessera_bits.to_string(),
            format!("{:+}", tessera_bits as i64 - RAW_CELLS as i64),
        ]);
    }
    report.add_table("noise", table);
    report.add_note(format!("{BITMAPS_PER_DENSITY} bitmaps of noise a density"));
}
