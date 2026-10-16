//! The cells about a cell: read as the aligned small tiles they lie
//! in, they are the cells read one by one, however the tiles' edges
//! fall about the cell; what may be stepped to is what no wall bars
//! less what the collision plane holds; and one drawn is one of those
//! chosen among, each as likely.
//!
//! `cargo test --test fast`

use crate::tests::{cell, STONE_LAYER};
use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use chunk_storage::LayerCodec;
use coordinates::{CellIndex, SuperchunkIndex};
use entity_manager::WorldEntities;
use actions::{around, walking};
use simulation::Simulation;
use type_registry::{COLLISION, WALL_EAST, WALL_NORTH, WALL_SOUTH, WALL_WEST};
use utilities::rng::RandomSource;

/// Cells of the 3x3 superchunks from `(10, 10)`, across and down from
/// its top left: every way a cell lies in its small tile and its word
/// tile, inside a chunk, on the borders of chunks and of superchunks,
/// and at the edge of what is hot.
fn cells_every_way() -> impl Iterator<Item = (u32, u32)> {
    [(0, 0), (248, 248), (1016, 1016), (2040, 500), (500, 2040), (3056, 3056), (1500, 0)].into_iter().flat_map(|(tile_left, tile_top)| (0..16).flat_map(move |cell_down| (0..16).map(move |cell_across| (tile_left + cell_across, tile_top + cell_down))))
}

/// The nine cells about a cell are its cells read one by one.
#[test]
fn the_nine_about_a_cell_are_its_cells_read_one_by_one() {
    let mut seeded_random = RandomSource::new(utilities::seed::counted_seed());
    let mut bitmap_arena = crate::tests::stone_arena(3, (0..3_000_000).map(move |_| cell(seeded_random.draw_below(3072) as u32, seeded_random.draw_below(3072) as u32)));
    let report = Simulation::new(1).tick(&mut bitmap_arena, &mut WorldEntities::new(), utilities::seed::counted_seed(), |turn| {
        if turn.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
            return 0;
        }
        let mut set_cells = 0;
        for (x, y) in cells_every_way() {
            let asked_cell = cell(x, y);
            let about_tiles = around::tiles_about_cell(asked_cell);
            let stone_about = around::read_layer(turn, STONE_LAYER, &about_tiles);
            for way in 0..around::WAYS {
                let holds_there = around::cell(asked_cell, way).map(|cell| turn.layer_holds(STONE_LAYER, cell));
                let (hot, is_set) = (matches!(holds_there, Some(Ok(_))), holds_there == Some(Ok(true)));
                let cell_that_way = about_tiles.cell_towards(way);
                assert_eq!((!(stone_about.hot & cell_that_way).is_empty(), !(stone_about.set_cells & cell_that_way).is_empty()), (hot, is_set), "the way {way} from ({x}, {y})");
                assert_eq!((cell_that_way.count(), around::way_of(asked_cell, around::cell(asked_cell, way).unwrap())), (1, way));
                set_cells += usize::from(is_set);
            }
            assert_eq!((about_tiles.nine_cells().count(), about_tiles.neighbour_cells().count(), about_tiles.centre_cell(), stone_about.set_cells.count()), (9, 8, about_tiles.cell_towards(around::HERE), (stone_about.set_cells & about_tiles.nine_cells()).count()));
            // One drawn is the one of that rank among the ways chosen among, in the ways' order: the same neighbour wherever the cell lies in its tiles.
            let ways: Vec<u32> = (0..around::WAYS).filter(|&way| !(stone_about.set_cells & about_tiles.cell_towards(way)).is_empty()).collect();
            let draw_stream = turn.random_stream().next_draw();
            for draw_index in 0..8 * ways.len() as u64 {
                let (mut picking_random, mut ranking_random) = (RandomSource::for_stream(draw_stream, draw_index), RandomSource::for_stream(draw_stream, draw_index));
                assert_eq!(around::pick_way(&mut picking_random, &about_tiles, stone_about.set_cells), Some(ways[ranking_random.draw_below(ways.len() as u64) as usize]), "about ({x}, {y})");
            }
            // One drawn among those set is one of them, and with draws enough each is drawn.
            let mut drawn_cells = around::Cells::default();
            for _ in 0..40 * stone_about.set_cells.count() {
                let way = around::pick_way(turn.random_stream(), &about_tiles, stone_about.set_cells).expect("one of those set");
                assert!(!(stone_about.set_cells & about_tiles.cell_towards(way)).is_empty(), "drawn out of those chosen among");
                drawn_cells = drawn_cells | about_tiles.cell_towards(way);
            }
            assert!(stone_about.set_cells.is_empty() && around::pick_way(turn.random_stream(), &about_tiles, stone_about.set_cells).is_none() || drawn_cells == stone_about.set_cells, "every one of those set drawn, about ({x}, {y})");
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
    let (mut layer_codec, mut bitmap_arena, mut seeded_random) = (LayerCodec::new(), BitmapArena::new(), RandomSource::new(utilities::seed::counted_seed()));
    for layer_type in [WALL_NORTH, WALL_WEST, WALL_EAST, WALL_SOUTH, COLLISION] {
        for (x, y) in (10..13).flat_map(|y| (10..13).map(move |x| (x, y))) {
            for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
                bitmap_arena.make_hot(BucketKey { layer_type, chunk }, None, &mut layer_codec);
            }
        }
        // The collision plane thick enough to hem cells in, the walls thin.
        let cells = if layer_type == COLLISION { 5_000_000 } else { 1_000_000 };
        (0..cells).for_each(|_| bitmap_arena.queue_write(layer_type, Write::cell(cell(seeded_random.draw_below(3072) as u32, seeded_random.draw_below(3072) as u32), WriteOp::SetCell)));
    }
    // One cell hemmed in whatever was drawn.
    let hemmed_cell: CellIndex = cell(1023, 1024);
    (0..around::WAYS).filter(|&way_checked| way_checked != around::HERE).for_each(|way_held| bitmap_arena.queue_write(COLLISION, Write::cell(around::cell(hemmed_cell, way_held).unwrap(), WriteOp::SetCell)));
    assert_eq!(bitmap_arena.apply().missed, 0);
    let report = Simulation::new(1).tick(&mut bitmap_arena, &mut WorldEntities::new(), utilities::seed::counted_seed(), |turn| {
        if turn.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
            return 0;
        }
        assert!(walking::around_steppable(turn, &around::tiles_about_cell(hemmed_cell)).is_empty(), "hemmed in");
        let mut open_steps = 0;
        for (x, y) in cells_every_way() {
            let asked_cell = cell(x, y);
            let about_tiles = around::tiles_about_cell(asked_cell);
            let (steppable_cells, collision_about) = (walking::around_steppable(turn, &about_tiles), around::read_layer(turn, COLLISION, &about_tiles));
            assert_eq!(steppable_cells, (walking::around_unwalled(turn, &about_tiles) & about_tiles.neighbour_cells()).cells_not_in(collision_about.set_cells), "about ({x}, {y})");
            open_steps += steppable_cells.count() as usize;
        }
        open_steps
    });
    assert!(report.rules > 1000, "steps open about the cells asked, and steps barred");
}
