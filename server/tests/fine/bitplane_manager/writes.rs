//! Writes into the bitplanes, batched: queued, then applied in order,
//! the latest winning; each shape covering exactly its cells, across
//! chunks, superchunks and the world's edge.
//!
//! `cargo test`

use crate::bitplane_manager::{WritesApplied, BitmapArena, BucketKey, WriteShape, Write, WriteOp};
use crate::test_types::TestTypes;
use crate::chunks::grass_on_dirt;
use crate::chunk_storage::{ChunkStorage, LayerCodec};
use utilities::coordinates::{CellCartesian, CellIndex, WORLD_MIDDLE};


/// An arena with `types.stone` hot and empty in every chunk holding one of
/// `cells`.
fn arena_over(types: TestTypes, cells: &[CellCartesian]) -> BitmapArena {
    let (mut layer_codec, mut bitmap_arena) = (LayerCodec::new(), BitmapArena::new());
    for cell in cells {
        bitmap_arena.make_hot(BucketKey { layer_type: types.stone, chunk: CellIndex::from(*cell).chunk() }, None, &mut layer_codec);
    }
    bitmap_arena
}

/// `write_op` over `write_shape` in the `types.stone` bitplane.
fn write_stone(types: TestTypes, bitmap_arena: &mut BitmapArena, write_op: WriteOp, anchor_cell: CellCartesian, write_shape: WriteShape) {
    bitmap_arena.queue_write(types.stone, Write { anchor_cell: anchor_cell.into(), write_op, write_shape });
}

/// Whether `types.stone` holds at `(x, y)`.
fn stone_holds_cell(types: TestTypes, bitmap_arena: &BitmapArena, x: u32, y: u32) -> bool {
    bitmap_arena.layer_holds_cell(types.stone, CellCartesian { x, y }.into()).expect("hot")
}

#[test]
fn a_write_is_20_bytes() {
    assert_eq!(size_of::<Write>(), 20);
}

/// Queued writes change nothing until applied; applying empties the
/// queue.
#[test]
fn nothing_changes_until_applied() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let cell = CellCartesian { x: 1000, y: 2000 };
    let mut bitmap_arena = arena_over(types, &[cell]);
    write_stone(types, &mut bitmap_arena, WriteOp::SetCell, cell, WriteShape::Cell);
    assert_eq!((bitmap_arena.queued_write_count(), stone_holds_cell(types, &bitmap_arena, cell.x, cell.y)), (1, false));
    assert_eq!(bitmap_arena.apply(), WritesApplied { writes: 1, cells_changed: 1, missed: 0, writes_refused: 0 });
    assert_eq!((bitmap_arena.queued_write_count(), stone_holds_cell(types, &bitmap_arena, cell.x, cell.y)), (0, true));
    assert_eq!(bitmap_arena.apply(), WritesApplied::default(), "nothing left queued");
}

/// Overlapping writes apply in the order queued: the latest wins, and a
/// flip flips what the writes before it left.
#[test]
fn the_latest_write_wins() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let rectangle_corner = CellCartesian { x: 10, y: 10 };
    let mut bitmap_arena = arena_over(types, &[rectangle_corner]);
    write_stone(types, &mut bitmap_arena, WriteOp::SetCell, rectangle_corner, WriteShape::RectangleOfCells { width: 4, height: 4 });
    write_stone(types, &mut bitmap_arena, WriteOp::ClearCell, CellCartesian { x: 11, y: 11 }, WriteShape::Cell);
    write_stone(types, &mut bitmap_arena, WriteOp::ClearCell, CellCartesian { x: 12, y: 12 }, WriteShape::Cell);
    write_stone(types, &mut bitmap_arena, WriteOp::SetCell, CellCartesian { x: 12, y: 12 }, WriteShape::Cell);
    write_stone(types, &mut bitmap_arena, WriteOp::FlipCell, CellCartesian { x: 13, y: 10 }, WriteShape::RectangleOfCells { width: 2, height: 1 });
    let writes_applied = bitmap_arena.apply();
    assert!(!stone_holds_cell(types, &bitmap_arena, 11, 11) && stone_holds_cell(types, &bitmap_arena, 12, 12) && stone_holds_cell(types, &bitmap_arena, 10, 13));
    assert!(!stone_holds_cell(types, &bitmap_arena, 13, 10) && stone_holds_cell(types, &bitmap_arena, 14, 10), "flipped: set to clear, clear to set");
    assert_eq!(writes_applied.cells_changed, 16 + 1 + 1 + 1 + 2);

    write_stone(types, &mut bitmap_arena, WriteOp::FlipCell, rectangle_corner, WriteShape::DiscOfCells { radius_in_cells: 2 });
    write_stone(types, &mut bitmap_arena, WriteOp::FlipCell, rectangle_corner, WriteShape::DiscOfCells { radius_in_cells: 2 });
    let count_before = bitmap_arena.bucket(BucketKey { layer_type: types.stone, chunk: CellIndex::from(rectangle_corner).chunk() }).expect("hot").count();
    bitmap_arena.apply();
    assert_eq!(bitmap_arena.bucket(BucketKey { layer_type: types.stone, chunk: CellIndex::from(rectangle_corner).chunk() }).expect("hot").count(), count_before, "flipped twice");
}

/// A rectangle across the corner where four superchunks meet sets its
/// cells in all four, and no others.
#[test]
fn rectangles_cross_chunks_and_superchunks() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let superchunk_edge = WORLD_MIDDLE.top_left().cartesian().x;
    let rectangle_corner = CellCartesian { x: superchunk_edge - 3, y: superchunk_edge - 2 };
    let cells = [rectangle_corner, CellCartesian { x: superchunk_edge, y: superchunk_edge - 2 }, CellCartesian { x: superchunk_edge - 3, y: superchunk_edge }, CellCartesian { x: superchunk_edge, y: superchunk_edge }];
    let mut bitmap_arena = arena_over(types, &cells);
    write_stone(types, &mut bitmap_arena, WriteOp::SetCell, rectangle_corner, WriteShape::RectangleOfCells { width: 6, height: 5 });
    assert_eq!(bitmap_arena.apply().cells_changed, 30);
    let superchunks = [(-1, -1), (0, -1), (-1, 0), (0, 0)].map(|(dx, dy)| WORLD_MIDDLE.at_cartesian_offset(dx, dy).expect("in the world"));
    assert_eq!(superchunks.map(|superchunk| bitmap_arena.superchunk_count(types.stone, superchunk)), [6, 6, 9, 9], "3 columns each side; 2 rows above, 3 below");
    for (x, y, expect_held) in [(superchunk_edge - 3, superchunk_edge - 2, true), (superchunk_edge + 2, superchunk_edge + 2, true), (superchunk_edge - 4, superchunk_edge, false), (superchunk_edge + 3, superchunk_edge, false), (superchunk_edge, superchunk_edge - 3, false), (superchunk_edge, superchunk_edge + 3, false)] {
        assert_eq!(stone_holds_cell(types, &bitmap_arena, x, y), expect_held, "cell ({x}, {y})");
    }
}

/// A disc covers the cells no farther than its radius from its centre,
/// centre to centre; at the world's edge it is cut off.
#[test]
fn discs_cover_their_radius() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let disc_centre = CellCartesian { x: 300, y: 300 };
    let mut bitmap_arena = arena_over(types, &[disc_centre, CellCartesian { x: 0, y: 0 }]);
    write_stone(types, &mut bitmap_arena, WriteOp::SetCell, disc_centre, WriteShape::DiscOfCells { radius_in_cells: 3 });
    assert_eq!(bitmap_arena.apply().cells_changed, 29, "the cells with dx² + dy² <= 9");
    assert!(stone_holds_cell(types, &bitmap_arena, 303, 300) && stone_holds_cell(types, &bitmap_arena, 302, 302) && stone_holds_cell(types, &bitmap_arena, 297, 300));
    assert!(!stone_holds_cell(types, &bitmap_arena, 303, 301) && !stone_holds_cell(types, &bitmap_arena, 304, 300));
    write_stone(types, &mut bitmap_arena, WriteOp::SetCell, CellCartesian { x: 0, y: 0 }, WriteShape::DiscOfCells { radius_in_cells: 2 });
    write_stone(types, &mut bitmap_arena, WriteOp::SetCell, CellCartesian { x: 100, y: 100 }, WriteShape::DiscOfCells { radius_in_cells: 0 });
    assert_eq!(bitmap_arena.apply().cells_changed, 6 + 1, "a quarter of a disc at the world's corner, and a lone cell");
}

/// Cells of bitmaps that are not hot are left unwritten, and counted.
#[test]
fn cold_bitmaps_are_missed() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let mut bitmap_arena = arena_over(types, &[CellCartesian { x: 0, y: 0 }]);
    write_stone(types, &mut bitmap_arena, WriteOp::SetCell, CellCartesian { x: 250, y: 0 }, WriteShape::RectangleOfCells { width: 10, height: 2 });
    assert_eq!(bitmap_arena.apply(), WritesApplied { writes: 1, cells_changed: 12, missed: 8, writes_refused: 0 });
    assert!(bitmap_arena.layer_holds_cell(types.stone, CellCartesian { x: 256, y: 0 }.into()).is_err());
}

/// Writes drawn -- any op over any shape, about the corner where four
/// superchunks meet, one of them not hot -- leave every cell as cells
/// written one by one in the order queued would, and count what
/// changed and what was missed as that does.
#[test]
fn writes_drawn_are_the_cells_written_one_by_one() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    use std::collections::HashSet;
    let mut seeded_random = utilities::rng::RandomSource::new(seed);
    let superchunk_edge = WORLD_MIDDLE.top_left().cartesian().x;
    // The chunks about the corner, each way; those up and left of it left cold.
    let hot: Vec<CellCartesian> = (-2..2).flat_map(|chunks_down| (-2..2).map(move |chunks_across| (chunks_across, chunks_down))).filter(|&(offset_across, offset_down)| offset_across >= 0 || offset_down >= 0).map(|(chunk_offset_across, chunk_offset_down)| CellCartesian { x: superchunk_edge.wrapping_add_signed(chunk_offset_across * 256), y: superchunk_edge.wrapping_add_signed(chunk_offset_down * 256) }).collect();
    let mut bitmap_arena = arena_over(types, &hot);
    let is_hot = |x: u32, y: u32| x >= superchunk_edge || y >= superchunk_edge;
    let mut cells_held: HashSet<(u32, u32)> = HashSet::new();
    for batch_number in 0..40 {
        let mut expected_applied = WritesApplied::default();
        for _ in 0..seeded_random.draw_between(1, 30) {
            let anchor_cell = CellCartesian { x: superchunk_edge - 400 + seeded_random.draw_below(800) as u32, y: superchunk_edge - 400 + seeded_random.draw_below(800) as u32 };
            let write_shape = match seeded_random.draw_below(3) {
                0 => WriteShape::Cell,
                1 => WriteShape::RectangleOfCells { width: seeded_random.draw_below(40) as u8, height: seeded_random.draw_below(40) as u8 },
                _ => WriteShape::DiscOfCells { radius_in_cells: seeded_random.draw_below(20) as u8 },
            };
            let write_op = [WriteOp::SetCell, WriteOp::ClearCell, WriteOp::FlipCell][seeded_random.draw_below(3) as usize];
            let covered_cells: Vec<(u32, u32)> = match write_shape {
                WriteShape::Cell => vec![(anchor_cell.x, anchor_cell.y)],
                WriteShape::RectangleOfCells { width, height } => (0..u32::from(height)).flat_map(|rect_down| (0..u32::from(width)).map(move |rect_across| (anchor_cell.x + rect_across, anchor_cell.y + rect_down))).collect(),
                WriteShape::DiscOfCells { radius_in_cells: radius } => {
                    let signed_radius = i32::from(radius);
                    (-signed_radius..=signed_radius).flat_map(|disc_down| (-signed_radius..=signed_radius).map(move |disc_across| (disc_across, disc_down))).filter(|(inside_across, inside_down)| inside_across * inside_across + inside_down * inside_down <= signed_radius * signed_radius).map(|(cell_across, cell_down)| (anchor_cell.x.wrapping_add_signed(cell_across), anchor_cell.y.wrapping_add_signed(cell_down))).collect()
                }
            };
            for cell in covered_cells {
                if !is_hot(cell.0, cell.1) {
                    expected_applied.missed += 1;
                    continue;
                }
                let was_held = cells_held.contains(&cell);
                let becomes_held = match write_op {
                    WriteOp::SetCell => true,
                    WriteOp::ClearCell => false,
                    _ => !was_held,
                };
                expected_applied.cells_changed += u64::from(was_held != becomes_held);
                if becomes_held { cells_held.insert(cell) } else { cells_held.remove(&cell) };
            }
            expected_applied.writes += 1;
            write_stone(types, &mut bitmap_arena, write_op, anchor_cell, write_shape);
        }
        assert_eq!(bitmap_arena.apply(), expected_applied, "batch {batch_number}");
        let superchunks_counted: u32 = [(-1, 0), (0, -1), (0, 0)].iter().map(|&(superchunk_across, superchunk_down)| bitmap_arena.superchunk_count(types.stone, WORLD_MIDDLE.at_cartesian_offset(superchunk_across, superchunk_down).expect("in the world"))).sum();
        assert_eq!(superchunks_counted as usize, cells_held.len(), "batch {batch_number}");
    }
    assert!(cells_held.len() > 1_000, "{} cells held", cells_held.len());
    for (x, y) in (superchunk_edge - 460..superchunk_edge + 460).flat_map(|y| (superchunk_edge - 460..superchunk_edge + 460).map(move |x| (x, y))).filter(|&(x, y)| is_hot(x, y)) {
        assert_eq!(stone_holds_cell(types, &bitmap_arena, x, y), cells_held.contains(&(x, y)), "cell ({x}, {y})");
    }
}

/// Grass spreading over the mock superchunk's dirt, as writes: a cell
/// stays dirt or grass, and the counts keep up.
#[test]
fn grass_spreads_over_dirt() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let mut layer_codec = LayerCodec::new();
    let mut bitmap_arena = BitmapArena::new();
    let mut cold_storage = ChunkStorage::new(1 << 12);
    cold_storage.insert(WORLD_MIDDLE, grass_on_dirt(types, seed, 8, &mut layer_codec));
    for chunk in WORLD_MIDDLE.chunks() {
        bitmap_arena.make_hot_layers(chunk, &[types.dirt, types.grass], &cold_storage, &mut layer_codec);
    }
    let superchunk_corner = WORLD_MIDDLE.top_left().cartesian();
    let disc_centre = CellCartesian { x: superchunk_corner.x + 256, y: superchunk_corner.y + 256 };
    bitmap_arena.queue_write(types.grass, Write { anchor_cell: disc_centre.into(), write_op: WriteOp::SetCell, write_shape: WriteShape::DiscOfCells { radius_in_cells: 10 } });
    bitmap_arena.queue_write(types.dirt, Write { anchor_cell: disc_centre.into(), write_op: WriteOp::ClearCell, write_shape: WriteShape::DiscOfCells { radius_in_cells: 10 } });
    let writes_applied = bitmap_arena.apply();
    assert_eq!(writes_applied.missed, 0);
    assert_eq!(bitmap_arena.superchunk_count(types.grass, WORLD_MIDDLE) + bitmap_arena.superchunk_count(types.dirt, WORLD_MIDDLE), 1 << 20, "dirt or grass, never both");
    assert!(bitmap_arena.superchunk_count(types.grass, WORLD_MIDDLE) >= 300, "a disc of radius 10 is over 300 cells");
}
