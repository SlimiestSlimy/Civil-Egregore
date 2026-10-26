//! The queue applier: a chunk's queue applied in order, a compare that fails
//! skipping past the next skip tag.

use server::hot_chunk::HotChunk;
use server::plane_shapes::{PlaneShape, PlaneShapes};
use server::queue_applier::apply_queue;
use server::queued_instruction::Instruction;
use utilities::coordinates::{ChunkIndex, CellIndex, CHUNK_SIDE_CELLS};
use utilities::rng::RandomSource;

/// Grass spreading, set by set: a set that holds writes; one that fails
/// skips its writes, the set after it run all the same; a compare holds
/// against what a write before it made; the last tile not 0 cleared
/// empties the plane, let go at the tick's end.
#[test]
fn sets_apply_in_order_and_skip_on_a_failed_compare() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random_numbers = RandomSource::new(seed);
    let chunk_index = ChunkIndex(random_numbers.draw_below(1 << 20));
    let (grass, rock) = (random_numbers.next_draw(), random_numbers.next_draw());
    let plane_shapes = PlaneShapes::new(vec![(grass, PlaneShape { width_bits: 1, tile_scale: 0 }), (rock, PlaneShape { width_bits: 8, tile_scale: 2 })]);
    let mut chunk = HotChunk::new(chunk_index, seed);
    // Cells drawn in the chunk, side by side: a's right neighbour is b, b's is c.
    let (x, y) = (random_numbers.draw_below(u64::from(CHUNK_SIDE_CELLS) - 3) as u32, random_numbers.draw_below(u64::from(CHUNK_SIDE_CELLS)) as u32);
    let [a, b, c] = [0, 1, 2].map(|across| CellIndex::from_chunk_place(chunk_index, utilities::coordinates::place_from_cartesian(x + across, y)));
    let tile_index = |cell: CellIndex| cell.place() as u32;
    let spread = |from: CellIndex, to: CellIndex| [Instruction::compare(grass, tile_index(from), 1), Instruction::compare(grass, tile_index(to), 0), Instruction::write(grass, tile_index(to), 1), Instruction::skip_tag()];
    let mut queue = vec![Instruction::compare(grass, tile_index(a), 0), Instruction::write(grass, tile_index(a), 1), Instruction::skip_tag()];
    // c has no grass yet: this set fails and skips its write.
    queue.extend(spread(c, b));
    queue.extend([Instruction::compare(grass, tile_index(c), 1), Instruction::write(rock, tile_index(c), 200), Instruction::skip_tag()]);
    // a has grass, written above: spreads to b, then b to c.
    queue.extend(spread(a, b));
    queue.extend(spread(b, c));
    apply_queue(&mut chunk, &queue, &plane_shapes);
    assert_eq!([a, b, c].map(|cell| chunk.read_tile(grass, cell)), [1, 1, 1]);
    assert_eq!((chunk.read_tile(rock, c), chunk.non_zero_tiles(rock)), (0, 0), "skipped: c had no grass when compared");
    // Several compares to one tag: one failing skips the whole set.
    let set = [Instruction::compare(grass, tile_index(a), 1), Instruction::compare(grass, tile_index(b), 1), Instruction::compare(grass, tile_index(c), 0), Instruction::write(rock, tile_index(a), 7), Instruction::skip_tag()];
    apply_queue(&mut chunk, &set, &plane_shapes);
    assert_eq!(chunk.read_tile(rock, a), 0);
    // A compare after a write in a set: the writes before it stay.
    let cleared = [a, b, c].map(|cell| [Instruction::compare(grass, tile_index(cell), 1), Instruction::write(grass, tile_index(cell), 0), Instruction::compare(rock, tile_index(cell), 9), Instruction::write(rock, tile_index(cell), 1), Instruction::skip_tag()]);
    apply_queue(&mut chunk, cleared.as_flattened(), &plane_shapes);
    assert_eq!(([a, b, c].map(|cell| chunk.read_tile(grass, cell)), chunk.non_zero_tiles(grass), chunk.non_zero_tiles(rock)), ([0, 0, 0], 0, 0));
    chunk.drop_empty_planes();
    assert_eq!(chunk.read_tile(grass, a), 0);
}
