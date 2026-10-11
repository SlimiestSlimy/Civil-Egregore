//! Masks: an aligned tile read at once is its cells read one by one,
//! whole or under another mask; whether a layer holds under a mask is
//! whether it holds at any of the mask's cells; what is set and
//! cleared under a mask is the mask; and a mask's own cells.
//!
//! `cargo test --test fast`

use crate::tests::{cell, STONE};
use bitplane_manager::BitmapArena;
use coordinates::SuperchunkIndex;
use entity_manager::Entities;
use actions::mask::{self, Mask, SIDES};
use simulation::Simulation;
use utilities::rng::Rng;

/// An arena with `STONE` hot over the 3x3 superchunks from `(10, 10)`,
/// cells scattered over them set, some on their borders.
fn arena() -> BitmapArena {
    let mut random = utilities::rng::Rng::new(utilities::seed::counted());
    crate::tests::arena(3, (0..200_000).map(move |_| cell(random.below(3072) as u32, random.below(3072) as u32)))
}

/// A mask's own cells: full, a disc, and those drawn and listed are
/// the ones set.
#[test]
fn a_mask_holds_its_cells() {
    let mut random = Rng::new(utilities::seed::counted());
    for side in SIDES {
        let (full, disc) = (Mask::full(side), Mask::disc(side));
        assert_eq!((full.count(), Mask::empty(side).count()), (side * side, 0));
        // A disc is its square's, less the corners, and the same turned over.
        assert!(disc.count() < full.count() && disc.count() > full.count() / 2, "a disc of {side}");
        assert!(disc.cells().all(|(x, y)| disc.get(side - 1 - x, y) && disc.get(y, x)));
        assert_eq!(disc.cells().count() as u32, disc.count());
        let (x, y) = disc.pick(&mut random).expect("a cell");
        assert!(disc.get(x, y));
        let mut ring = full.clone();
        ring.and_not(&disc);
        assert_eq!(ring.count(), full.count() - disc.count());
        ring.and(&disc);
        assert!(ring.is_empty() && ring.pick(&mut random).is_none());
    }
}

/// An aligned tile read into a mask is its cells read one by one --
/// inside a chunk, a chunk whole, several chunks, off the hot
/// superchunks -- and read under a mask, those of them the mask has;
/// and a layer holds under a mask if it holds at any cell of it.
#[test]
fn a_tile_read_is_its_cells_read_one_by_one() {
    let mut arena = arena();
    let origins = [(300, 300), (1000, 1000), (2047, 5), (2900, 2900), (0, 1500)];
    Simulation::new(1).tick(&mut arena, &mut Entities::new(), utilities::seed::counted(), |turn, _| {
        if turn.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
            return 0;
        }
        for side in [4, 8, 16, 64, 256, 512] {
            let (mut set, mut hot, disc) = (Mask::empty(side), Mask::empty(side), Mask::disc(side));
            let (mut set_under, mut hot_under) = (Mask::empty(side), Mask::empty(side));
            for (x, y) in origins {
                let (at, first) = (cell(x, y), mask::tile(cell(x, y), side));
                assert_eq!((first.cartesian().x % side, first.cartesian().y % side, mask::tile(first, side)), (0, 0, first), "a tile is aligned");
                mask::layer(turn, STONE, at, &mut set, &mut hot);
                mask::layer_under(turn, STONE, at, &disc, &mut set_under, &mut hot_under);
                for (across, down) in (0..side).flat_map(|down| (0..side).map(move |across| (across, down))) {
                    let held = turn.holds(STONE, mask::cell(first, across, down));
                    assert_eq!((hot.get(across, down), set.get(across, down)), (held.is_ok(), held == Ok(true)), "({across}, {down}) of {side} from ({x}, {y})");
                }
                set.and(&disc);
                hot.and(&disc);
                assert_eq!((&set_under, &hot_under), (&set, &hot), "{side} from ({x}, {y}), under a disc");
                assert_eq!(mask::any_under(turn, STONE, at, &disc), !set.is_empty(), "{side} from ({x}, {y}): any under a disc");
                // Under one cell of it: whether that cell is held.
                if let Some((across, down)) = disc.pick(turn.random()) {
                    let mut one = Mask::empty(side);
                    one.set(across, down, true);
                    assert_eq!(mask::any_under(turn, STONE, at, &one), set.get(across, down), "{side} from ({x}, {y}): under ({across}, {down})");
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
    for side in [4, 64, 1024] {
        let mut arena = arena();
        let (origin, disc) = (cell(700, 900), Mask::disc(side));
        let before = std::sync::Mutex::new((Mask::empty(side), Mask::empty(side)));
        let mut simulation = Simulation::new(2);
        let writes = simulation.tick(&mut arena, &mut Entities::new(), utilities::seed::counted(), |turn, _| {
            if turn.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
                return 0;
            }
            let mut before = before.lock().unwrap();
            let (set, hot) = &mut *before;
            mask::layer(turn, STONE, origin, set, hot);
            // The disc cleared where set, and a square in its middle set where clear: each cell written once, from what was seen.
            let mut middle = Mask::empty(side);
            (side / 4..side / 2).flat_map(|y| (side / 4..side / 2).map(move |x| (x, y))).for_each(|(x, y)| middle.set(x, y, true));
            mask::clear_under(turn, STONE, origin, &disc) + mask::set_under(turn, STONE, origin, &middle)
        });
        assert_eq!((writes.writes_applied.missed, writes.writes_applied.refused), (0, 0));
        assert_eq!(writes.writes_applied.changed, writes.rules as u64, "every write queued changes its cell");
        simulation.tick(&mut arena, &mut Entities::new(), utilities::seed::counted(), |turn, _| {
            if turn.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
                return 0;
            }
            let (mut set, mut hot) = (Mask::empty(side), Mask::empty(side));
            mask::layer(turn, STONE, origin, &mut set, &mut hot);
            let before = &before.lock().unwrap().0;
            for (x, y) in (0..side).flat_map(|y| (0..side).map(move |x| (x, y))) {
                let middle = (side / 4..side / 2).contains(&x) && (side / 4..side / 2).contains(&y);
                let expected = if middle { !before.get(x, y) } else { !disc.get(x, y) && before.get(x, y) };
                assert_eq!(set.get(x, y), expected, "({x}, {y}) of {side}");
            }
            0
        });
    }
}
