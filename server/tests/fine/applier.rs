//! The applier: a chunk's queue applied in order, a compare that fails
//! skipping past the next skip tag.

use server::applier::apply;
use server::hot_chunk::HotChunk;
use server::instruction::Instruction;
use server::plane_shapes::PlaneShapes;
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
    let plane_shapes = PlaneShapes::new(vec![(grass, 1, 1), (rock, 8, 4)]);
    let mut chunk = HotChunk::new(chunk_index, seed);
    // Cells drawn in the chunk, side by side: a's right neighbour is b, b's is c.
    let (x, y) = (random_numbers.draw_below(u64::from(CHUNK_SIDE_CELLS) - 3) as u32, random_numbers.draw_below(u64::from(CHUNK_SIDE_CELLS)) as u32);
    let [a, b, c] = [0, 1, 2].map(|across| CellIndex::from_chunk_place(chunk_index, utilities::coordinates::place_from_cartesian(x + across, y)));
    let place = |cell: CellIndex| cell.place() as u32;
    let spread = |from: CellIndex, to: CellIndex| [Instruction::compare(grass, place(from), 1), Instruction::compare(grass, place(to), 0), Instruction::write(grass, place(to), 1), Instruction::skip_tag()];
    let mut queue = vec![Instruction::compare(grass, place(a), 0), Instruction::write(grass, place(a), 1), Instruction::skip_tag()];
    // c has no grass yet: this set fails and skips its write.
    queue.extend(spread(c, b));
    queue.extend([Instruction::compare(grass, place(c), 1), Instruction::write(rock, place(c), 200), Instruction::skip_tag()]);
    // a has grass, written above: spreads to b, then b to c.
    queue.extend(spread(a, b));
    queue.extend(spread(b, c));
    apply(&mut chunk, &queue, &plane_shapes);
    assert_eq!([a, b, c].map(|cell| chunk.read_tile(grass, cell)), [1, 1, 1]);
    assert_eq!((chunk.read_tile(rock, c), chunk.non_zero_tiles(rock)), (0, 0), "skipped: c had no grass when compared");
    // Several compares to one tag: one failing skips the whole set.
    let set = [Instruction::compare(grass, place(a), 1), Instruction::compare(grass, place(b), 1), Instruction::compare(grass, place(c), 0), Instruction::write(rock, place(a), 7), Instruction::skip_tag()];
    apply(&mut chunk, &set, &plane_shapes);
    assert_eq!(chunk.read_tile(rock, a), 0);
    // A compare after a write in a set: the writes before it stay.
    let cleared = [a, b, c].map(|cell| [Instruction::compare(grass, place(cell), 1), Instruction::write(grass, place(cell), 0), Instruction::compare(rock, place(cell), 9), Instruction::write(rock, place(cell), 1), Instruction::skip_tag()]);
    apply(&mut chunk, cleared.as_flattened(), &plane_shapes);
    assert_eq!(([a, b, c].map(|cell| chunk.read_tile(grass, cell)), chunk.non_zero_tiles(grass), chunk.non_zero_tiles(rock)), ([0, 0, 0], 0, 0));
    chunk.drop_empty_planes();
    assert_eq!(chunk.read_tile(grass, a), 0);
}
