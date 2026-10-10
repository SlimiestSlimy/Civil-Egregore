//! The area about a cell: read at once, it is its cells read one by
//! one.
//!
//! `cargo test --test fast`

use crate::tests::{arena, cell, STONE};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex};
use entity_manager::Entities;
use instructions::area::{self, AREA_CENTRE, AREA_SIDE};
use simulation::Simulation;

/// Cells scattered over the 3x3 superchunks from `(10, 10)`, some on
/// their borders.
fn scattered() -> impl Iterator<Item = CellIndex> {
    (0..3000u32).map(|at| cell((at * 7919) % 3072, (at * 104_729) % 3072))
}

/// The area about a cell read at once is its cells read one by one:
/// inside a superchunk, across the borders of four, and at the edge of
/// the hot superchunks, where some cells are not hot.
#[test]
fn areas_read_at_once_are_the_cells_read_one_by_one() {
    let mut arena = arena(3, scattered());
    let start = cell(0, 0).cartesian();
    let centres = [(300, 300), (1024, 1024), (1020, 1029), (5, 5), (2040, 2047), (1024, 3), (777, 1023)];
    let checked = Simulation::new(1).tick(&mut arena, &mut Entities::new(), 0, |turn, _| {
        if turn.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
            return 0;
        }
        for (x, y) in centres {
            let centre = CellCartesian { x: start.x + x, y: start.y + y };
            let area = area::layer(turn, STONE, centre.into());
            for (across, down) in (0..AREA_SIDE as u32).flat_map(|down| (0..AREA_SIDE as u32).map(move |across| (across, down))) {
                let cell = CellCartesian { x: centre.x + across - AREA_CENTRE as u32, y: centre.y + down - AREA_CENTRE as u32 };
                let held = turn.holds(STONE, cell.into());
                let read = |rows: [u16; AREA_SIDE]| rows[down as usize] >> across & 1 == 1;
                assert_eq!((read(area.hot), read(area.set)), (held.is_ok(), held == Ok(true)), "({across}, {down}) of the area about ({x}, {y})");
            }
        }
        centres.len()
    });
    assert_eq!(checked.rules, centres.len());
}
