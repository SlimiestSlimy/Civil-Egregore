//! The server handle: tiles read, of the ticking chunk or a neighbour
//! hot, or not hot; a set queued only at its end, at its tiles' corners;
//! a bad plane, a value too wide, a cell of another chunk refused by a
//! panic.

use protocol::type_registry::TypeRegistry;
use server::hot_chunk::HotChunk;
use server::queue_applier::apply_queue;
use server::queued_instruction::Instruction;
use server::server_handle::{NotHot, ServerHandle};
use server::server_manager::ServerManager;
use utilities::coordinates::{CellIndex, ChunkIndex, CELLS_IN_CHUNK};
use utilities::rng::RandomSource;

/// A world of `type_registry`'s planes, no sca proc, a drawn chunk hot
/// and empty in it; and a drawn cell of that chunk, its random stream,
/// and a queue: from the run's seed.
fn world_with_a_drawn_chunk(type_registry: TypeRegistry) -> (ServerManager, CellIndex, RandomSource, Vec<Instruction>) {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut random_numbers = RandomSource::new(seed);
    let chunk_index = ChunkIndex(random_numbers.draw_below(1 << 20));
    let cell = CellIndex::from_chunk_place(chunk_index, random_numbers.draw_below(CELLS_IN_CHUNK as u64) as usize);
    let chunk = HotChunk::new(chunk_index, seed);
    let random_stream = chunk.take_random_stream();
    let mut server_manager = ServerManager::new(type_registry, Vec::new());
    server_manager.insert_hot_chunk(chunk);
    (server_manager, cell, random_stream, Vec::new())
}

/// The cell at `cell`'s place in the chunk after `cell`'s.
fn in_the_next_chunk(cell: CellIndex) -> CellIndex {
    CellIndex::from_chunk_place(ChunkIndex(cell.chunk().0 + 1), cell.place())
}

/// A cell of a neighbour, hot, read as it holds; a cell of a chunk not
/// hot, not read.
#[test]
fn hot_tiles_read_and_others_not() {
    let (type_registry, plane_type_ids) = crate::test_registry::registry_of_planes("server_handle_reads", &[(8, 1)]);
    let [rock] = plane_type_ids[..] else { panic!("one plane") };
    let (mut server_manager, cell, mut random_stream, mut queue) = world_with_a_drawn_chunk(type_registry);
    let in_the_neighbour = in_the_next_chunk(cell);
    let mut neighbour = HotChunk::new(in_the_neighbour.chunk(), 0);
    neighbour.write_tile(rock, in_the_neighbour, 200, server_manager.type_registry());
    server_manager.insert_hot_chunk(neighbour);
    let handle = ServerHandle::new(&server_manager, cell.chunk(), &mut random_stream, &mut queue);
    assert_eq!((handle.read_tile(rock, cell), handle.read_tile(rock, in_the_neighbour)), (Ok(0), Ok(200)));
    assert_eq!(handle.read_tile(rock, in_the_next_chunk(in_the_neighbour)), Err(NotHot));
}

/// A set never ended queues nothing; one ended queues its compares,
/// its writes and a skip tag, each at its tile's corner, and applies.
#[test]
fn a_set_is_queued_at_its_end_alone() {
    let (type_registry, plane_type_ids) = crate::test_registry::registry_of_planes("server_handle_sets", &[(1, 1), (4, 2)]);
    let [grass, soil] = plane_type_ids[..] else { panic!("two planes") };
    let (server_manager, cell, mut random_stream, mut queue) = world_with_a_drawn_chunk(type_registry);
    let mut handle = ServerHandle::new(&server_manager, cell.chunk(), &mut random_stream, &mut queue);
    let _never_ended = handle.set().compare(grass, cell, 0).write(grass, cell, 1);
    handle.set().compare(grass, cell, 0).compare(soil, cell, 0).write(grass, cell, 1).write(soil, cell, 9).end();
    let soil_corner = (cell.place() - cell.place() % 4) as u32;
    let expected = [Instruction::compare(grass, cell.place() as u32, 0), Instruction::compare(soil, soil_corner, 0), Instruction::write(grass, cell.place() as u32, 1), Instruction::write(soil, soil_corner, 9), Instruction::skip_tag()];
    assert_eq!(queue, expected);
    // Applied to the chunk as it was, empty.
    let mut chunk = HotChunk::new(cell.chunk(), 0);
    apply_queue(&mut chunk, &queue, server_manager.type_registry());
    assert_eq!((chunk.read_tile(grass, cell), chunk.read_tile(soil, cell)), (1, 9));
}

/// A type ID of no plane -- the mod's own -- panics.
#[test]
#[should_panic(expected = "is no plane's type ID")]
fn a_type_id_of_no_plane_panics() {
    let (type_registry, _) = crate::test_registry::registry_of_planes("server_handle_no_plane", &[(1, 1)]);
    let mod_type_id = type_registry.types()[0].type_id();
    let (server_manager, cell, mut random_stream, mut queue) = world_with_a_drawn_chunk(type_registry);
    let mut handle = ServerHandle::new(&server_manager, cell.chunk(), &mut random_stream, &mut queue);
    let _ = handle.set().compare(mod_type_id, cell, 0);
}

/// A value wider than the plane's numbers panics.
#[test]
#[should_panic(expected = "is wider than the 1 bits")]
fn a_value_too_wide_panics() {
    let (type_registry, plane_type_ids) = crate::test_registry::registry_of_planes("server_handle_too_wide", &[(1, 1)]);
    let (server_manager, cell, mut random_stream, mut queue) = world_with_a_drawn_chunk(type_registry);
    let mut handle = ServerHandle::new(&server_manager, cell.chunk(), &mut random_stream, &mut queue);
    let _ = handle.set().compare(plane_type_ids[0], cell, 2);
}

/// A cell of another chunk in a set panics, for now.
#[test]
#[should_panic(expected = "a cell of another chunk")]
fn a_cell_of_another_chunk_panics() {
    let (type_registry, plane_type_ids) = crate::test_registry::registry_of_planes("server_handle_other_chunk", &[(1, 1)]);
    let (server_manager, cell, mut random_stream, mut queue) = world_with_a_drawn_chunk(type_registry);
    let mut handle = ServerHandle::new(&server_manager, cell.chunk(), &mut random_stream, &mut queue);
    let _ = handle.set().compare(plane_type_ids[0], in_the_next_chunk(cell), 0);
}
