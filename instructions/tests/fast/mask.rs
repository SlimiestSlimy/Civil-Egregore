//! Masks: a square read at once is its cells read one by one, whole
//! or under another mask; what is set and cleared under a mask is
//! the mask, in a few rectangles; and a mask's own cells.
//!
//! `cargo test --test fast`

use bitplane_manager::{BitmapArena, BucketKey, Write, WriteOp};
use chunk_storage::{LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, SuperchunkIndex};
use entity_manager::Entities;
use instructions::mask::{self, Mask, SIDES};
use instructions::{read, write};
use instructions::Simulation;
use utilities::rng::Rng;

/// The layer type the tests run on.
const STONE: LayerType = LayerType(6);

/// An arena with `STONE` hot over the 3x3 superchunks from `(10, 10)`,
/// cells scattered over them set, some on their borders.
fn arena() -> BitmapArena {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    for (x, y) in (10..13).flat_map(|y| (10..13).map(move |x| (x, y))) {
        for chunk in SuperchunkIndex::from_cartesian(x, y).chunks() {
            arena.make_hot(BucketKey { layer_type: STONE, chunk }, None, &mut codec);
        }
    }
    let start = corner();
    for at in 0..200_000u64 {
        let cell = CellCartesian { x: start.x + (at * 7919 % 3072) as u32, y: start.y + (at * 104_729 % 3067) as u32 };
        arena.queue(STONE, Write::cell(cell.into(), WriteOp::Set));
    }
    assert_eq!(arena.apply().missed, 0);
    arena
}

/// The top left cell of the superchunk `(10, 10)`, cartesian.
fn corner() -> CellCartesian {
    SuperchunkIndex::from_cartesian(10, 10).top_left().cartesian()
}

/// The cell `(x, y)` cells from the top left of the superchunk `(10, 10)`.
fn cell(x: u32, y: u32) -> CellIndex {
    CellCartesian { x: corner().x + x, y: corner().y + y }.into()
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

/// A square read into a mask is its cells read one by one -- inside a
/// superchunk, across borders, off the hot superchunks -- and read
/// under a mask, those of them the mask has.
#[test]
fn a_square_read_is_its_cells_read_one_by_one() {
    let mut arena = arena();
    let origins = [(300, 300), (1000, 1000), (2047, 5), (2900, 2900), (0, 1500)];
    Simulation::new(1).tick(&mut arena, &mut Entities::new(), 0, |turn, _| {
        if turn.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
            return 0;
        }
        for side in [4, 8, 16, 64, 256] {
            let (mut set, mut hot, disc) = (Mask::empty(side), Mask::empty(side), Mask::disc(side));
            let (mut set_under, mut hot_under) = (Mask::empty(side), Mask::empty(side));
            for (x, y) in origins {
                let origin = cell(x, y);
                read::mask::layer(turn, STONE, origin, &mut set, &mut hot);
                read::mask::layer_under(turn, STONE, origin, &disc, &mut set_under, &mut hot_under);
                for (across, down) in (0..side).flat_map(|down| (0..side).map(move |across| (across, down))) {
                    let held = turn.holds(STONE, mask::cell(origin, across, down).expect("in the world"));
                    assert_eq!((hot.get(across, down), set.get(across, down)), (held.is_ok(), held == Ok(true)), "({across}, {down}) of {side} from ({x}, {y})");
                }
                set.and(&disc);
                hot.and(&disc);
                assert_eq!((&set_under, &hot_under), (&set, &hot), "{side} from ({x}, {y}), under a disc");
            }
        }
        1
    });
}

/// Cells set and cleared under a mask are the mask's, the rest left
/// as they were, and a whole square or a disc takes few writes.
#[test]
fn a_mask_written_is_read_back() {
    for side in [4, 64, 1024] {
        let mut arena = arena();
        let (origin, disc) = (cell(700, 900), Mask::disc(side));
        let before = std::sync::Mutex::new((Mask::empty(side), Mask::empty(side)));
        let mut simulation = Simulation::new(2);
        let writes = simulation.tick(&mut arena, &mut Entities::new(), 0, |turn, _| {
            if turn.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
                return 0;
            }
            let mut before = before.lock().unwrap();
            let (set, hot) = &mut *before;
            read::mask::layer(turn, STONE, origin, set, hot);
            // The disc cleared, then a square in its middle set: the later write wins.
            let mut middle = Mask::empty(side);
            (side / 4..side / 2).flat_map(|y| (side / 4..side / 2).map(move |x| (x, y))).for_each(|(x, y)| middle.set(x, y, true));
            write::mask::clear(turn, STONE, origin, &disc) + write::mask::set(turn, STONE, origin, &middle)
        });
        assert!(writes.rules <= 2 * side as usize + 1, "{} writes for a disc and a square of {side}", writes.rules);
        assert_eq!(writes.writes_applied.missed, 0);
        simulation.tick(&mut arena, &mut Entities::new(), 0, |turn, _| {
            if turn.superchunk() != SuperchunkIndex::from_cartesian(10, 10) {
                return 0;
            }
            let (mut set, mut hot) = (Mask::empty(side), Mask::empty(side));
            read::mask::layer(turn, STONE, origin, &mut set, &mut hot);
            let before = &before.lock().unwrap().0;
            for (x, y) in (0..side).flat_map(|y| (0..side).map(move |x| (x, y))) {
                let middle = (side / 4..side / 2).contains(&x) && (side / 4..side / 2).contains(&y);
                let expected = middle || !disc.get(x, y) && before.get(x, y);
                assert_eq!(set.get(x, y), expected, "({x}, {y}) of {side}");
            }
            0
        });
    }
}
