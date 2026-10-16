//! What Tessera's tree is made of: for each bitmap looked at, how many
//! nodes of each kind at each level -- the tree is made whichever stream
//! is written -- and the bits written.

use crate::diagnostics::bitmaps::looked_at;
use crate::diagnostics::census::census;
use crate::BitStream;
use crate::tile::{Tile, FLOOR_LEVEL};
use crate::Tessera;
use utilities::diagnostics::table::report::Report;
use utilities::diagnostics::table::Table;

/// Prints the census of every bitmap looked at.
pub fn run_census_tool(report: &mut Report) {
    let (mut tessera, mut stream) = (Tessera::new(), BitStream::default());
    for (bitmap_name, bitmap) in looked_at() {
        tessera.encode(&bitmap, &mut stream);
        let tree = tessera.tree();
        let level_headings: Vec<String> = std::iter::once("node".to_string())
            .chain((0..=FLOOR_LEVEL).map(|tile_level| format!("level {tile_level}\n{0}x{0}", Tile { tile_level, x: 0, y: 0 }.side_in_cells())))
            .collect();
        let mut table = Table::new(&level_headings.iter().map(String::as_str).collect::<Vec<_>>());
        for (node_kind, by_level) in census(tree) {
            let row_cells: Vec<String> = std::iter::once(node_kind.to_string()).chain(by_level.iter().map(|count| count.to_string())).collect();
            table.add_row(&row_cells);
        }
        report.add_table(format!("{bitmap_name}: {} bits written", stream.len()), table);
    }
}
