//! The nine cells about a cell: read through the aligned small tiles
//! they lie in, they are the cells read one by one, however the
//! tiles' edges fall about the cell; and what may be stepped to is
//! what no wall bars less what the collision plane holds.
//!
//! `cargo test --test fast`

use crate::tests::{cell, STONE};
use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use chunk_storage::LayerCodec;
use coordinates::{CellIndex, SuperchunkIndex};
use entity_manager::Entities;
use instructions::{around, walking};
use simulation::Simulation;
use type_registry::{COLLISION, WALL_EAST, WALL_SOUTH};
use utilities::rng::Rng;

/// Cells of the 3x3 superchunks from `(10, 10)`, across and down from
/// its top left: every way a cell lies in its small tile and its word
/// tile, inside a chunk, on the borders of chunks and of superchunks,
/// and at the edge of what is hot.
fn cells_every_way() -> impl Iterator<Item = (u32, u32)> {
    [(0, 0), (248, 248), (1016, 1016), (2040, 500), (500, 2040), (3056, 3056), (1500, 0)].into_iter().flat_map(|(left, top)| (0..16).flat_map(move |down| (0..16).map(move |across| (left + across, top + down))))
}

/// The nine cells about a cell are its cells read one by one.
#[test]
fn the_nine_about_a_cell_are_its_cells_read_one_by_one() {
    let mut random = Rng::new(utilities::seed::counted());
    let mut arena = crate::tests::arena(3, (0..3_000_000).map(move |_| cell(random.below(3072) as u32, random.below(3072) as u32)));
    let report = Simulation::new(1).tick(&mut arena, &mut Entities::new(), utilities::seed::counted(), |turn, _| {
        if turn.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
            return 0;
        }
        let mut set_cells = 0;
        for (x, y) in cells_every_way() {
            let at = cell(x, y);
            let nine = around::layer(turn, STONE, at);
            for bit in 0..9 {
                let held = around::cell(at, bit).map(|cell| turn.holds(STONE, cell));
                let (hot, set) = (matches!(held, Some(Ok(_))), held == Some(Ok(true)));
                assert_eq!((nine.hot >> bit & 1 == 1, nine.set >> bit & 1 == 1), (hot, set), "bit {bit} about ({x}, {y})");
                set_cells += usize::from(set);
            }
        }
        set_cells
    });
    assert!(report.rules > 1000, "the cells read held stone at some, and not at others");
}

/// What may be stepped to about a cell is what no wall bars, less
/// what the collision plane holds -- and nothing where the plane holds
/// the whole ring.
#[test]
fn what_may_be_stepped_to_is_the_unwalled_less_the_held() {
    let (mut codec, mut arena, mut random) = (LayerCodec::new(), BitmapArena::new(), Rng::new(utilities::seed::counted()));
    for layer_type in [WALL_EAST, WALL_SOUTH, COLLISION] {
        for (x, y) in (10..13).flat_map(|y| (10..13).map(move |x| (x, y))) {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                arena.make_hot(BucketKey { layer_type, chunk }, None, &mut codec);
            }
        }
        // The collision plane thick enough to hem cells in, the walls thin.
        let cells = if layer_type == COLLISION { 5_000_000 } else { 1_000_000 };
        (0..cells).for_each(|_| arena.queue(layer_type, Write::cell(cell(random.below(3072) as u32, random.below(3072) as u32), WriteOp::Set)));
    }
    // One cell hemmed in whatever was drawn.
    let hemmed: CellIndex = cell(1023, 1024);
    (0..9).filter(|&bit| bit != 4).for_each(|bit| arena.queue(COLLISION, Write::cell(around::cell(hemmed, bit).unwrap(), WriteOp::Set)));
    assert_eq!(arena.apply().missed, 0);
    let report = Simulation::new(1).tick(&mut arena, &mut Entities::new(), utilities::seed::counted(), |turn, _| {
        if turn.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
            return 0;
        }
        assert_eq!(walking::around_steppable(turn, hemmed) & around::RING, 0, "hemmed in");
        let mut open = 0;
        for (x, y) in cells_every_way() {
            let at = cell(x, y);
            let (steppable, held) = (walking::around_steppable(turn, at), around::layer(turn, COLLISION, at));
            assert_eq!(steppable, walking::around_unwalled(turn, at) & !held.set, "about ({x}, {y})");
            open += (steppable & around::RING).count_ones() as usize;
        }
        open
    });
    assert!(report.rules > 1000, "steps open about the cells asked, and steps barred");
}
