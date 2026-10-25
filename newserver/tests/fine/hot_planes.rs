//! A hot superchunk's planes: numbers kept at every width and tile size,
//! counted as they change, kept sparse -- a plane and a chunk only while
//! they hold a cell set or their emptiness is not yet in the cold superchunk --
//! written back, and made again from the cold superchunk they were written into.

use crate::cold_storage::plane_chunk_encoding::encode_plane_chunk;
use crate::cold_storage::{ColdStorage, BitmapCodec};
use crate::hot_superchunks::hot_superchunk::HotSuperchunk;
use crate::hot_superchunks::hot_planes::{COUNT_TILES_IN_CHUNK, COUNT_TILE_CELLS};
use crate::registered_planes::{most_value, TestPlanes};
use protocol::type_registry::PlaneType;
use std::collections::HashMap;
use utilities::coordinates::{CellIndex, SuperchunkIndex, CHUNKS_IN_SUPERCHUNK, WORLD_MIDDLE};
use utilities::rng::RandomSource;

/// Cells in a chunk.
const CHUNK_CELLS: u64 = 1 << 16;

/// What a plane is expected to hold: a number a slot -- a cell's, or a
/// tile's -- by chunk and slot, 0 where none is listed.
type Expected = HashMap<(usize, usize), u32>;

/// A cell of `superchunk` drawn: in one of a few chunks, so some chunks
/// are kept and some are not.
fn drawn_cell(seeded_random: &mut RandomSource, superchunk: SuperchunkIndex) -> CellIndex {
    let chunk = [0, 5, 10, 15][seeded_random.draw_below(4) as usize];
    CellIndex(superchunk.top_left().0 + chunk * CHUNK_CELLS + seeded_random.draw_below(CHUNK_CELLS))
}

/// The slot of `cell` in `plane_type`: its chunk, and its cell's -- or
/// tile's -- place there.
fn slot(plane_type: PlaneType, cell: CellIndex) -> (usize, usize) {
    (cell.chunk().place(), cell.place() >> plane_type.tile_size().shift())
}

/// Sets numbers drawn at cells drawn on every plane of `planes` in
/// `hot_superchunk`, each change checked against what was expected: what
/// is expected of each plane after.
fn set_drawn(hot_superchunk: &mut HotSuperchunk, planes: &TestPlanes, seeded_random: &mut RandomSource, settings: usize) -> HashMap<PlaneType, Expected> {
    let mut expected: HashMap<PlaneType, Expected> = HashMap::new();
    for _ in 0..settings {
        let plane_type = planes.every()[seeded_random.draw_below(planes.every().len() as u64) as usize];
        let cell = drawn_cell(seeded_random, hot_superchunk.superchunk_index());
        // A third of the settings clear a cell: some chunks empty again.
        let value = if seeded_random.draw_below(3) == 0 { 0 } else { 1 + seeded_random.draw_below(u64::from(most_value(plane_type))) as u32 };
        let held = expected.entry(plane_type).or_default().entry(slot(plane_type, cell)).or_default();
        assert_eq!(hot_superchunk.set_value(plane_type, cell, value), *held != value, "a setting changes only what it makes different");
        *held = value;
    }
    expected
}

/// Whether `hot_superchunk` holds what is `expected` of each plane, at
/// every slot set and at cells never set.
fn holds_expected(hot_superchunk: &HotSuperchunk, planes: &TestPlanes, expected: &HashMap<PlaneType, Expected>, seeded_random: &mut RandomSource) {
    for &plane_type in planes.every() {
        let plane_expected = expected.get(&plane_type).cloned().unwrap_or_default();
        for _ in 0..200 {
            let cell = drawn_cell(seeded_random, hot_superchunk.superchunk_index());
            assert_eq!(hot_superchunk.value_at(plane_type, cell), Some(plane_expected.get(&slot(plane_type, cell)).copied().unwrap_or(0)), "the number at a cell drawn");
        }
        for (&(chunk, slot_place), &value) in &plane_expected {
            let cell = CellIndex(hot_superchunk.superchunk_index().top_left().0 + chunk as u64 * CHUNK_CELLS + ((slot_place as u64) << plane_type.tile_size().shift()));
            assert_eq!(hot_superchunk.value_at(plane_type, cell), Some(value), "the number set at a cell");
        }
    }
}

/// Numbers set at every width and tile size are read back, and a cell
/// never set reads 0; each setting says whether it changed anything; a
/// cell of another superchunk is neither read nor written.
#[test]
fn numbers_set_are_read_back_at_every_width_and_tile_size() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (planes, mut seeded_random) = (TestPlanes::registered(seed), RandomSource::new(seed));
    let mut hot_superchunk = HotSuperchunk::new(WORLD_MIDDLE);
    let expected = set_drawn(&mut hot_superchunk, &planes, &mut seeded_random, 3000);
    holds_expected(&hot_superchunk, &planes, &expected, &mut seeded_random);
    let elsewhere = WORLD_MIDDLE.at_cartesian_offset(1, 0).expect("a superchunk beside").top_left();
    assert_eq!(hot_superchunk.value_at(planes.one_bit, elsewhere), None, "a cell of another superchunk is not read");
    assert!(!hot_superchunk.set_value(planes.one_bit, elsewhere, 1), "nor written");
}

/// Every plane's count of cells reading a number not 0 -- over the
/// superchunk, a chunk and each count tile, a tile's cells all counted --
/// is what its numbers come to after any settings, at every tile size.
#[test]
fn counts_follow_every_change() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (planes, mut seeded_random) = (TestPlanes::registered(seed), RandomSource::new(seed));
    let mut hot_superchunk = HotSuperchunk::new(WORLD_MIDDLE);
    let expected = set_drawn(&mut hot_superchunk, &planes, &mut seeded_random, 3000);
    for &plane_type in planes.every() {
        // Recounted from what was set: each number not 0 covering its tile's cells, a count tile's share of them each.
        let mut recounted = [[0u16; COUNT_TILES_IN_CHUNK]; CHUNKS_IN_SUPERCHUNK];
        let tile_cells = 1usize << plane_type.tile_size().shift();
        for (&(chunk, slot_place), _) in expected.get(&plane_type).into_iter().flatten().filter(|(_, value)| **value != 0) {
            for cell in (slot_place * tile_cells..(slot_place + 1) * tile_cells).step_by(tile_cells.min(COUNT_TILE_CELLS)) {
                recounted[chunk][cell / COUNT_TILE_CELLS] += tile_cells.min(COUNT_TILE_CELLS) as u16;
            }
        }
        let cells_set: u32 = recounted.iter().flatten().map(|&count| u32::from(count)).sum();
        let Some(plane) = hot_superchunk.plane(plane_type) else {
            assert_eq!(cells_set, 0, "a plane with a cell set is kept");
            continue;
        };
        assert_eq!(plane.cells_set(), cells_set, "the superchunk's count");
        for (chunk, chunk_recounted) in recounted.iter().enumerate() {
            assert_eq!(plane.tile_counts(chunk), chunk_recounted, "count tiles of chunk {chunk}");
            assert_eq!(plane.count(chunk), chunk_recounted.iter().map(|&count| u32::from(count)).sum::<u32>(), "the count of chunk {chunk}");
        }
    }
}

/// A superchunk keeps a plane, and a plane a chunk, only once a cell is
/// set there; emptied, they are kept until the emptiness is written back
/// and flushed -- and not let go while a write-back is on its way.
#[test]
fn planes_and_chunks_are_kept_only_while_something_is_set() {
    let planes = TestPlanes::registered(crate::test_seed::TestSeedHandle::open().fine_seed());
    let mut hot_superchunk = HotSuperchunk::new(WORLD_MIDDLE);
    let cell = CellIndex(WORLD_MIDDLE.top_left().0 + 3 * CHUNK_CELLS + 77);
    assert!(!hot_superchunk.set_value(planes.one_bit, cell, 0), "0 set where nothing is changes nothing");
    assert!(hot_superchunk.planes().is_empty(), "and makes no plane");
    assert!(hot_superchunk.set_value(planes.one_bit, cell, 1));
    assert_eq!(hot_superchunk.plane(planes.one_bit).map(|plane| plane.kept()), Some(1 << 3), "the one chunk set kept");
    assert!(hot_superchunk.set_value(planes.one_bit, cell, 0));
    assert!(hot_superchunk.plane(planes.one_bit).is_some(), "emptied, kept until its emptiness is written back");
    let dirty = hot_superchunk.take_dirty();
    assert_eq!(dirty.len(), 1, "the chunk changed");
    assert!(hot_superchunk.take_dirty().is_empty(), "taken once");
    hot_superchunk.flushed_from_ring();
    assert!(hot_superchunk.plane(planes.one_bit).is_some(), "kept while its write-back is on its way");
    assert!(!hot_superchunk.is_written_back());
    hot_superchunk.written_back(dirty.iter().map(|(plane_chunk, _)| *plane_chunk));
    assert!(hot_superchunk.plane(planes.one_bit).is_some(), "kept while in the ring");
    hot_superchunk.flushed_from_ring();
    assert!(hot_superchunk.is_written_back());
    assert!(hot_superchunk.planes().is_empty(), "let go once its cold superchunk holds it empty");
}

/// A superchunk set at every width and tile size, written back into a
/// cold superchunk chunk by chunk, is made again from the cold superchunk holding all it held.
#[test]
fn a_superchunk_comes_back_from_its_cold_superchunk() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (planes, mut seeded_random) = (TestPlanes::registered(seed), RandomSource::new(seed));
    let mut hot_superchunk = HotSuperchunk::new(WORLD_MIDDLE);
    let expected = set_drawn(&mut hot_superchunk, &planes, &mut seeded_random, 3000);
    let (mut cold_storage, mut bitmap_codec, mut flushed) = (ColdStorage::new(1 << 16), BitmapCodec::new(), Vec::new());
    let dirty = hot_superchunk.take_dirty();
    for (plane_chunk, words) in &dirty {
        cold_storage.write_back(*plane_chunk, &encode_plane_chunk(plane_chunk.plane_type, words, &mut bitmap_codec), &mut flushed);
    }
    hot_superchunk.written_back(dirty.iter().map(|(plane_chunk, _)| *plane_chunk));
    cold_storage.flush(WORLD_MIDDLE);
    hot_superchunk.flushed_from_ring();
    let cold_superchunk = cold_storage.cold_superchunk(WORLD_MIDDLE).expect("a cold superchunk written");
    let made_again = HotSuperchunk::from_cold_superchunk(WORLD_MIDDLE, planes.every(), cold_superchunk, &mut bitmap_codec);
    holds_expected(&made_again, &planes, &expected, &mut seeded_random);
    assert!(made_again.is_written_back(), "made from its cold superchunk, nothing to write back");
    let kept = |hot: &HotSuperchunk| hot.planes().iter().map(|plane| (plane.plane_type(), plane.kept(), plane.cells_set())).collect::<Vec<_>>();
    assert_eq!(kept(&made_again), kept(&hot_superchunk), "the same planes and chunks kept, the same counts");
}
