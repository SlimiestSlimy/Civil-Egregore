//! Writes into the bitplanes, batched: queued, then applied in order,
//! the latest winning; each shape covering exactly its cells, across
//! chunks, superchunks and the world's edge.
//!
//! `cargo test`

use bitplane_manager::{WritesApplied, BitmapArena, BucketKey, Shape, Write, WriteOp};
use crate::tests::{grass_on_dirt, MOCK_DIRT, MOCK_GRASS};
use chunk_storage::{ChunkStorage, LayerCodec, LayerType};
use coordinates::{CellCartesian, CellIndex, WORLD_MIDDLE};

/// The layer type the tests write.
const STONE: LayerType = LayerType(9);

/// An arena with `STONE` hot and empty in every chunk holding one of
/// `cells`.
fn arena_over(cells: &[CellCartesian]) -> BitmapArena {
    let (mut codec, mut arena) = (LayerCodec::new(), BitmapArena::new());
    for cell in cells {
        arena.make_hot(BucketKey { layer_type: STONE, chunk: CellIndex::from(*cell).chunk() }, None, &mut codec);
    }
    arena
}

/// `op` over `shape` in the `STONE` bitplane.
fn stone(arena: &mut BitmapArena, op: WriteOp, at: CellCartesian, shape: Shape) {
    arena.queue(STONE, Write { at: at.into(), op, shape });
}

/// Whether `STONE` holds at `(x, y)`.
fn holds(arena: &BitmapArena, x: u32, y: u32) -> bool {
    arena.holds(STONE, CellCartesian { x, y }.into()).expect("hot")
}

#[test]
fn a_write_is_16_bytes() {
    assert_eq!(size_of::<Write>(), 16);
}

/// Queued writes change nothing until applied; applying empties the
/// queue.
#[test]
fn nothing_changes_until_applied() {
    let cell = CellCartesian { x: 1000, y: 2000 };
    let mut arena = arena_over(&[cell]);
    stone(&mut arena, WriteOp::Set, cell, Shape::Cell);
    assert_eq!((arena.queued(), holds(&arena, cell.x, cell.y)), (1, false));
    assert_eq!(arena.apply(), WritesApplied { writes: 1, changed: 1, missed: 0, refused: 0 });
    assert_eq!((arena.queued(), holds(&arena, cell.x, cell.y)), (0, true));
    assert_eq!(arena.apply(), WritesApplied::default(), "nothing left queued");
}

/// Overlapping writes apply in the order queued: the latest wins, and a
/// flip flips what the writes before it left.
#[test]
fn the_latest_write_wins() {
    let corner = CellCartesian { x: 10, y: 10 };
    let mut arena = arena_over(&[corner]);
    stone(&mut arena, WriteOp::Set, corner, Shape::Rect { width: 4, height: 4 });
    stone(&mut arena, WriteOp::Unset, CellCartesian { x: 11, y: 11 }, Shape::Cell);
    stone(&mut arena, WriteOp::Unset, CellCartesian { x: 12, y: 12 }, Shape::Cell);
    stone(&mut arena, WriteOp::Set, CellCartesian { x: 12, y: 12 }, Shape::Cell);
    stone(&mut arena, WriteOp::Flip, CellCartesian { x: 13, y: 10 }, Shape::Rect { width: 2, height: 1 });
    let applied = arena.apply();
    assert!(!holds(&arena, 11, 11) && holds(&arena, 12, 12) && holds(&arena, 10, 13));
    assert!(!holds(&arena, 13, 10) && holds(&arena, 14, 10), "flipped: set to clear, clear to set");
    assert_eq!(applied.changed, 16 + 1 + 1 + 1 + 2);

    stone(&mut arena, WriteOp::Flip, corner, Shape::Disc { radius: 2 });
    stone(&mut arena, WriteOp::Flip, corner, Shape::Disc { radius: 2 });
    let before = arena.bucket(BucketKey { layer_type: STONE, chunk: CellIndex::from(corner).chunk() }).expect("hot").count();
    arena.apply();
    assert_eq!(arena.bucket(BucketKey { layer_type: STONE, chunk: CellIndex::from(corner).chunk() }).expect("hot").count(), before, "flipped twice");
}

/// A rectangle across the corner where four superchunks meet sets its
/// cells in all four, and no others.
#[test]
fn rectangles_cross_chunks_and_superchunks() {
    let edge = WORLD_MIDDLE.top_left().cartesian().x;
    let corner = CellCartesian { x: edge - 3, y: edge - 2 };
    let cells = [corner, CellCartesian { x: edge, y: edge - 2 }, CellCartesian { x: edge - 3, y: edge }, CellCartesian { x: edge, y: edge }];
    let mut arena = arena_over(&cells);
    stone(&mut arena, WriteOp::Set, corner, Shape::Rect { width: 6, height: 5 });
    assert_eq!(arena.apply().changed, 30);
    let superchunks = [(-1, -1), (0, -1), (-1, 0), (0, 0)].map(|(dx, dy)| WORLD_MIDDLE.offset(dx, dy).expect("in the world"));
    assert_eq!(superchunks.map(|superchunk| arena.superchunk_count(STONE, superchunk)), [6, 6, 9, 9], "3 columns each side; 2 rows above, 3 below");
    for (x, y, held) in [(edge - 3, edge - 2, true), (edge + 2, edge + 2, true), (edge - 4, edge, false), (edge + 3, edge, false), (edge, edge - 3, false), (edge, edge + 3, false)] {
        assert_eq!(holds(&arena, x, y), held, "cell ({x}, {y})");
    }
}

/// A disc covers the cells no farther than its radius from its centre,
/// centre to centre; at the world's edge it is cut off.
#[test]
fn discs_cover_their_radius() {
    let centre = CellCartesian { x: 300, y: 300 };
    let mut arena = arena_over(&[centre, CellCartesian { x: 0, y: 0 }]);
    stone(&mut arena, WriteOp::Set, centre, Shape::Disc { radius: 3 });
    assert_eq!(arena.apply().changed, 29, "the cells with dx² + dy² <= 9");
    assert!(holds(&arena, 303, 300) && holds(&arena, 302, 302) && holds(&arena, 297, 300));
    assert!(!holds(&arena, 303, 301) && !holds(&arena, 304, 300));
    stone(&mut arena, WriteOp::Set, CellCartesian { x: 0, y: 0 }, Shape::Disc { radius: 2 });
    stone(&mut arena, WriteOp::Set, CellCartesian { x: 100, y: 100 }, Shape::Disc { radius: 0 });
    assert_eq!(arena.apply().changed, 6 + 1, "a quarter of a disc at the world's corner, and a lone cell");
}

/// Cells of bitmaps that are not hot are left unwritten, and counted.
#[test]
fn cold_bitmaps_are_missed() {
    let mut arena = arena_over(&[CellCartesian { x: 0, y: 0 }]);
    stone(&mut arena, WriteOp::Set, CellCartesian { x: 250, y: 0 }, Shape::Rect { width: 10, height: 2 });
    assert_eq!(arena.apply(), WritesApplied { writes: 1, changed: 12, missed: 8, refused: 0 });
    assert!(arena.holds(STONE, CellCartesian { x: 256, y: 0 }.into()).is_err());
}

/// Writes drawn -- any op over any shape, about the corner where four
/// superchunks meet, one of them not hot -- leave every cell as cells
/// written one by one in the order queued would, and count what
/// changed and what was missed as that does.
#[test]
fn writes_drawn_are_the_cells_written_one_by_one() {
    use std::collections::HashSet;
    let mut random = utilities::rng::Rng::new(utilities::seed::counted());
    let edge = WORLD_MIDDLE.top_left().cartesian().x;
    // The chunks about the corner, each way; those up and left of it left cold.
    let hot: Vec<CellCartesian> = (-2..2).flat_map(|down| (-2..2).map(move |across| (across, down))).filter(|&(across, down)| across >= 0 || down >= 0).map(|(across, down)| CellCartesian { x: edge.wrapping_add_signed(across * 256), y: edge.wrapping_add_signed(down * 256) }).collect();
    let mut arena = arena_over(&hot);
    let is_hot = |x: u32, y: u32| x >= edge || y >= edge;
    let mut held: HashSet<(u32, u32)> = HashSet::new();
    for batch in 0..40 {
        let mut expected = WritesApplied::default();
        for _ in 0..random.between(1, 30) {
            let at = CellCartesian { x: edge - 400 + random.below(800) as u32, y: edge - 400 + random.below(800) as u32 };
            let shape = match random.below(3) {
                0 => Shape::Cell,
                1 => Shape::Rect { width: random.below(40) as u8, height: random.below(40) as u8 },
                _ => Shape::Disc { radius: random.below(20) as u8 },
            };
            let op = [WriteOp::Set, WriteOp::Unset, WriteOp::Flip][random.below(3) as usize];
            let covered: Vec<(u32, u32)> = match shape {
                Shape::Cell => vec![(at.x, at.y)],
                Shape::Rect { width, height } => (0..u32::from(height)).flat_map(|down| (0..u32::from(width)).map(move |across| (at.x + across, at.y + down))).collect(),
                Shape::Disc { radius } => {
                    let radius = i32::from(radius);
                    (-radius..=radius).flat_map(|down| (-radius..=radius).map(move |across| (across, down))).filter(|(across, down)| across * across + down * down <= radius * radius).map(|(across, down)| (at.x.wrapping_add_signed(across), at.y.wrapping_add_signed(down))).collect()
                }
            };
            for cell in covered {
                if !is_hot(cell.0, cell.1) {
                    expected.missed += 1;
                    continue;
                }
                let was = held.contains(&cell);
                let becomes = match op {
                    WriteOp::Set => true,
                    WriteOp::Unset => false,
                    _ => !was,
                };
                expected.changed += u64::from(was != becomes);
                if becomes { held.insert(cell) } else { held.remove(&cell) };
            }
            expected.writes += 1;
            stone(&mut arena, op, at, shape);
        }
        assert_eq!(arena.apply(), expected, "batch {batch}");
        let counted: u32 = [(-1, 0), (0, -1), (0, 0)].iter().map(|&(across, down)| arena.superchunk_count(STONE, WORLD_MIDDLE.offset(across, down).expect("in the world"))).sum();
        assert_eq!(counted as usize, held.len(), "batch {batch}");
    }
    assert!(held.len() > 1_000, "{} cells held", held.len());
    for (x, y) in (edge - 460..edge + 460).flat_map(|y| (edge - 460..edge + 460).map(move |x| (x, y))).filter(|&(x, y)| is_hot(x, y)) {
        assert_eq!(holds(&arena, x, y), held.contains(&(x, y)), "cell ({x}, {y})");
    }
}

/// Grass spreading over the mock superchunk's dirt, as writes: a cell
/// stays dirt or grass, and the counts keep up.
#[test]
fn grass_spreads_over_dirt() {
    let mut codec = LayerCodec::new();
    let mut arena = BitmapArena::new();
    let mut storage = ChunkStorage::new(1 << 12);
    storage.insert(WORLD_MIDDLE, grass_on_dirt(utilities::seed::counted(), 8, &mut codec));
    for chunk in WORLD_MIDDLE.chunks() {
        arena.make_hot_layers(chunk, &[MOCK_DIRT, MOCK_GRASS], &storage, &mut codec);
    }
    let corner = WORLD_MIDDLE.top_left().cartesian();
    let at = CellCartesian { x: corner.x + 256, y: corner.y + 256 };
    arena.queue(MOCK_GRASS, Write { at: at.into(), op: WriteOp::Set, shape: Shape::Disc { radius: 10 } });
    arena.queue(MOCK_DIRT, Write { at: at.into(), op: WriteOp::Unset, shape: Shape::Disc { radius: 10 } });
    let applied = arena.apply();
    assert_eq!(applied.missed, 0);
    assert_eq!(arena.superchunk_count(MOCK_GRASS, WORLD_MIDDLE) + arena.superchunk_count(MOCK_DIRT, WORLD_MIDDLE), 1 << 20, "dirt or grass, never both");
    assert!(arena.superchunk_count(MOCK_GRASS, WORLD_MIDDLE) >= 300, "a disc of radius 10 is over 300 cells");
}
