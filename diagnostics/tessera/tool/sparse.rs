//! Tessera on sparse bitmaps, density by density, scattered and
//! clustered: the tree's bits, the binary count tree's, the stream's,
//! and the least any encoding could take
//! (`docs/lab.md`, "`src/diagnostics/tool/`").

use crate::diagnostics::examination::BitmapExamination;
use crate::BitStream;
use crate::Tessera;
use crate::corpus::{grown_bitmaps, corpus_seed};
use utilities::diagnostics::table::report::Report;
use utilities::diagnostics::table::Table;
use bitmap::{Bitmap, HEIGHT, WIDTH};

/// The densities looked at, from under a cell on average to a sixth of
/// them -- closest around a tenth of a percent, where the two encodings
/// cross for scattered cells.
const SPARSE_DENSITIES: [f64; 15] = [0.00001, 0.00003, 0.0001, 0.0003, 0.0005, 0.0007, 0.001, 0.0015, 0.002, 0.003, 0.01, 0.02, 0.05, 0.1, 0.15];

/// How clustered the set cells are: scattered, ragged, blobs.
const CLUSTER_WEIGHTS: [f64; 3] = [0.0, 0.7, 0.95];

/// Bitmaps a density and clustering.
const BITMAPS_PER_CASE: u64 = 20;

/// Cells in the bitmap, as a float for the bound.
const CELLS: f64 = (WIDTH * HEIGHT) as f64;

/// log2 of how many ways `set_cells` cells can be placed among the bitmap's:
/// the fewest bits, on average, any encoding of scattered cells takes.
fn placements_bits(set_cells: u64) -> f64 {
    (0..set_cells).map(|placed_cells| ((CELLS - placed_cells as f64) / (set_cells - placed_cells) as f64).log2()).sum()
}

/// `cell_density` as a percentage, no longer than it needs to be.
fn percent_label(cell_density: f64) -> String {
    let percent_text = format!("{:.4}", cell_density * 100.0);
    format!("{}%", percent_text.trim_end_matches('0').trim_end_matches('.'))
}

/// Reports every density and clustering: the tree's bits, the count
/// split's, the stream's, and the bound for scattered cells.
pub fn run_sparse_tool(report: &mut Report) {
    let (mut tessera, mut stream, mut decoded_bitmap) = (Tessera::new(), BitStream::default(), Bitmap::new());
    let mut table = Table::new(&[
        "cluster",
        "density",
        "set cells\na bitmap",
        "tree\nbits",
        "binary count tree\nbits",
        "stream\nbits",
        "streams that\nare binary count trees",
        "scattered\nbound",
    ]);
    for cluster_weight in CLUSTER_WEIGHTS {
        for cell_density in SPARSE_DENSITIES {
            let (mut set_cells_total, mut tree, mut split_bits_total, mut written_bits_total, mut binary_count_tree_streams, mut bound_bits_total) = (0, 0, 0, 0, 0, 0.0);
            for bitmap in grown_bitmaps(corpus_seed(), cell_density, cluster_weight, BITMAPS_PER_CASE) {
                let bitmap_examination = BitmapExamination::examine_bitmap(&mut tessera, &mut stream, &mut decoded_bitmap, &bitmap);
                assert_eq!(bitmap_examination.first_difference, None, "cluster {cluster_weight}, density {cell_density}: a bitmap did not round trip");
                let set_cells = bitmap.count_set() as u64;
                set_cells_total += set_cells;
                let (tree_bits, binary_count_tree_bits) = tessera.stream_bits(&bitmap);
                tree += tree_bits;
                split_bits_total += binary_count_tree_bits;
                written_bits_total += bitmap_examination.written_bits as u64;
                binary_count_tree_streams += bitmap_examination.binary_count_tree as u64;
                bound_bits_total += placements_bits(set_cells);
            }
            table.add_row(&[
                format!("{cluster_weight}"),
                percent_label(cell_density),
                (set_cells_total / BITMAPS_PER_CASE).to_string(),
                (tree / BITMAPS_PER_CASE).to_string(),
                (split_bits_total / BITMAPS_PER_CASE).to_string(),
                (written_bits_total / BITMAPS_PER_CASE).to_string(),
                format!("{binary_count_tree_streams} of {BITMAPS_PER_CASE}"),
                if cluster_weight == 0.0 { format!("{:.0}", bound_bits_total / BITMAPS_PER_CASE as f64) } else { String::new() },
            ]);
        }
        table.divider();
    }
    report.add_table("sparse bitmaps", table);
    report.add_note(format!("{BITMAPS_PER_CASE} bitmaps a density and clustering"));
}
