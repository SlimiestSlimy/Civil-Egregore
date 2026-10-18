//! Masks: an aligned tile read at once is its cells read one by one,
//! whole or under another mask; whether a layer holds under a mask is
//! whether it holds at any of the mask's cells; what is set and
//! cleared under a mask is the mask; and a mask's own cells.
//!
//! `cargo test --test fast`

use crate::arenas::{cell, STONE_LAYER};
use server::bitplane_manager::BitmapArena;
use utilities::coordinates::SuperchunkIndex;
use server::entity_manager::WorldEntities;
use server::simulation::simulation_api::tile_masks::{self, Mask, MASK_SIDES};
use server::simulation::Simulation;
use utilities::rng::RandomSource;

/// An arena with `STONE` hot over the 3x3 superchunks from `(10, 10)`,
/// cells scattered over them set, some on their borders, drawn from
/// `seed`.
fn scattered_stone_arena(seed: u64) -> BitmapArena {
    let mut seeded_random = utilities::rng::RandomSource::new(seed);
    crate::arenas::stone_arena(3, (0..200_000).map(move |_| cell(seeded_random.draw_below(3072) as u32, seeded_random.draw_below(3072) as u32)))
}

/// A mask's own cells: full, a disc, and those drawn and listed are
/// the ones set.
#[test]
fn a_mask_holds_its_cells() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut seeded_random = RandomSource::new(seed);
    for mask_side in MASK_SIDES {
        let (full_mask, disc_mask) = (Mask::all_set(mask_side), Mask::filling_disc(mask_side));
        assert_eq!((full_mask.count(), Mask::all_clear(mask_side).count()), (mask_side * mask_side, 0));
        // A disc is its square's, less the corners, and the same turned over.
        assert!(disc_mask.count() < full_mask.count() && disc_mask.count() > full_mask.count() / 2, "a disc of {mask_side}");
        assert!(disc_mask.cells().all(|(x, y)| disc_mask.get(mask_side - 1 - x, y) && disc_mask.get(y, x)));
        assert_eq!(disc_mask.cells().count() as u32, disc_mask.count());
        let (x, y) = disc_mask.pick_set_cell(&mut seeded_random).expect("a cell");
        assert!(disc_mask.get(x, y));
        let mut ring_mask = full_mask.clone();
        ring_mask.and_not(&disc_mask);
        assert_eq!(ring_mask.count(), full_mask.count() - disc_mask.count());
        ring_mask.and_with(&disc_mask);
        assert!(ring_mask.is_empty() && ring_mask.pick_set_cell(&mut seeded_random).is_none());
    }
}

/// An aligned tile read into a mask is its cells read one by one --
/// inside a chunk, a chunk whole, several chunks, off the hot
/// superchunks -- and read under a mask, those of them the mask has;
/// and a layer holds under a mask if it holds at any cell of it.
#[test]
fn a_tile_read_is_its_cells_read_one_by_one() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    let mut bitmap_arena = scattered_stone_arena(seed);
    let origins = [(300, 300), (1000, 1000), (2047, 5), (2900, 2900), (0, 1500)];
    Simulation::new(1).tick(&mut bitmap_arena, &mut WorldEntities::new(), seed, |tick_state| {
        if tick_state.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
            return 0;
        }
        for mask_side in [4, 8, 16, 64, 256, 512] {
            let (mut set_mask, mut hot, disc_mask) = (Mask::all_clear(mask_side), Mask::all_clear(mask_side), Mask::filling_disc(mask_side));
            let (mut set_under, mut hot_under) = (Mask::all_clear(mask_side), Mask::all_clear(mask_side));
            for (x, y) in origins {
                let (origin_cell, tile_first_cell) = (cell(x, y), tile_masks::tile(cell(x, y), mask_side));
                assert_eq!((tile_first_cell.cartesian().x % mask_side, tile_first_cell.cartesian().y % mask_side, tile_masks::tile(tile_first_cell, mask_side)), (0, 0, tile_first_cell), "a tile is aligned");
                tile_masks::read_layer(tick_state, STONE_LAYER, origin_cell, &mut set_mask, &mut hot);
                tile_masks::layer_under(tick_state, STONE_LAYER, origin_cell, &disc_mask, &mut set_under, &mut hot_under);
                for (across_in_tile, down_in_tile) in (0..mask_side).flat_map(|row_down| (0..mask_side).map(move |column_across| (column_across, row_down))) {
                    let cell_holds = tick_state.layer_holds(STONE_LAYER, tile_masks::cell(tile_first_cell, across_in_tile, down_in_tile));
                    assert_eq!((hot.get(across_in_tile, down_in_tile), set_mask.get(across_in_tile, down_in_tile)), (cell_holds.is_ok(), cell_holds == Ok(true)), "({across_in_tile}, {down_in_tile}) of {mask_side} from ({x}, {y})");
                }
                set_mask.and_with(&disc_mask);
                hot.and_with(&disc_mask);
                assert_eq!((&set_under, &hot_under), (&set_mask, &hot), "{mask_side} from ({x}, {y}), under a disc");
                assert_eq!(tile_masks::any_under(tick_state, STONE_LAYER, origin_cell, &disc_mask), !set_mask.is_empty(), "{mask_side} from ({x}, {y}): any under a disc");
                // Under one cell of it: whether that cell is held.
                if let Some((picked_across, picked_down)) = disc_mask.pick_set_cell(tick_state.random_stream()) {
                    let mut one_cell_mask = Mask::all_clear(mask_side);
                    one_cell_mask.set(picked_across, picked_down, true);
                    assert_eq!(tile_masks::any_under(tick_state, STONE_LAYER, origin_cell, &one_cell_mask), set_mask.get(picked_across, picked_down), "{mask_side} from ({x}, {y}): under ({picked_across}, {picked_down})");
                }
            }
        }
        1
    });
}

/// Cells set and cleared under a mask are the mask's that were
/// otherwise, the rest left as they were, no cell written twice.
#[test]
fn a_mask_written_is_read_back() {
    let seed = crate::test_seed::TestSeedHandle::open().seed();
    for mask_side in [4, 64, 1024] {
        let mut bitmap_arena = scattered_stone_arena(seed);
        let (origin, disc_mask) = (cell(700, 900), Mask::filling_disc(mask_side));
        let masks_before = std::sync::Mutex::new((Mask::all_clear(mask_side), Mask::all_clear(mask_side)));
        let mut test_simulation = Simulation::new(2);
        let writes = test_simulation.tick(&mut bitmap_arena, &mut WorldEntities::new(), seed, |tick_state| {
            if tick_state.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
                return 0;
            }
            let mut masks_before_guard = masks_before.lock().unwrap();
            let (set_mask_before, hot) = &mut *masks_before_guard;
            tile_masks::read_layer(tick_state, STONE_LAYER, origin, set_mask_before, hot);
            // The disc cleared where set, and a square in its middle set where clear: each cell written once, from what was seen.
            let mut middle_mask = Mask::all_clear(mask_side);
            (mask_side / 4..mask_side / 2).flat_map(|y| (mask_side / 4..mask_side / 2).map(move |x| (x, y))).for_each(|(x, y)| middle_mask.set(x, y, true));
            tile_masks::clear_under(tick_state, STONE_LAYER, origin, &disc_mask) + tile_masks::set_under(tick_state, STONE_LAYER, origin, &middle_mask)
        });
        assert_eq!((writes.writes_applied.missed, writes.writes_applied.writes_refused), (0, 0));
        assert_eq!(writes.writes_applied.cells_changed, writes.rules as u64, "every write queued changes its cell");
        test_simulation.tick(&mut bitmap_arena, &mut WorldEntities::new(), seed, |tick_state| {
            if tick_state.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
                return 0;
            }
            let (mut set_mask_after, mut hot) = (Mask::all_clear(mask_side), Mask::all_clear(mask_side));
            tile_masks::read_layer(tick_state, STONE_LAYER, origin, &mut set_mask_after, &mut hot);
            let set_before = &masks_before.lock().unwrap().0;
            for (x, y) in (0..mask_side).flat_map(|y| (0..mask_side).map(move |x| (x, y))) {
                let in_middle = (mask_side / 4..mask_side / 2).contains(&x) && (mask_side / 4..mask_side / 2).contains(&y);
                let expected_set = if in_middle { !set_before.get(x, y) } else { !disc_mask.get(x, y) && set_before.get(x, y) };
                assert_eq!(set_mask_after.get(x, y), expected_set, "({x}, {y}) of {mask_side}");
            }
            0
        });
    }
}
