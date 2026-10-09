//! The area about a cell: read at once, it is its cells read one by
//! one.
//!
//! `cargo test --test fast`

use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, SuperchunkIndex};
use entity_manager::Entities;
use instructions::area::{AREA_CENTRE, AREA_SIDE};
use instructions::read::area;
use instructions::Simulation;

/// The layer type the tests run on.
const STONE: LayerType = LayerType(6);

/// An arena with `STONE` hot over the `side` by `side` superchunks from
/// `(10, 10)`, the cells `cells` set.
fn arena(side: u32, cells: impl Iterator<Item = CellCartesian>) -> BitmapArena {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    for y in 10..10 + side {
        for x in 10..10 + side {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                arena.make_hot(BucketKey { layer_type: STONE, chunk }, None, &mut codec);
            }
        }
    }
    cells.for_each(|cell| arena.queue(STONE, Write::cell(cell.into(), WriteOp::Set)));
    assert_eq!(arena.apply().missed, 0);
    arena
}

/// The top left cell of the superchunk `(x, y)`, cartesian.
fn corner(x: u32, y: u32) -> CellCartesian {
    SuperchunkIndex::from_cartesian(x, y).top_left().cartesian()
}

/// Cells scattered over the 3x3 superchunks from `(10, 10)`, some on
/// their borders.
fn scattered() -> impl Iterator<Item = CellCartesian> {
    let start = corner(10, 10);
    (0..3000u32).map(move |at| CellCartesian { x: start.x + (at * 7919) % 3072, y: start.y + (at * 104_729) % 3072 })
}

/// The area about a cell read at once is its cells read one by one:
/// inside a superchunk, across the borders of four, and at the edge of
/// the hot superchunks, where some cells are not hot.
#[test]
fn areas_read_at_once_are_the_cells_read_one_by_one() {
    let mut arena = arena(3, scattered());
    let start = corner(10, 10);
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
