//! The cold pool paged to disk, only ever inside a world: cold
//! superchunks past the bytes kept go to the world's paging folder and
//! read back the same, and the paging folder is emptied when paging
//! starts and gone when the pool is.

use crate::cold_storage::{world_folder_files, BitmapCodec, ColdPlaneChunkChange, ColdStorage, ColdSuperchunk, PlaneChunk};
use crate::registered_planes::TestPlanes;
use utilities::bitmap::WORDS;
use utilities::coordinates::{ChunkIndex, SuperchunkIndex, WORLD_MIDDLE};
use utilities::rng::RandomSource;
use std::path::{Path, PathBuf};

/// An empty folder for the test `test_name`, in the workspace's
/// `transient_data/newserver_tests/`.
fn test_folder(test_name: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("transient_data").join("newserver_tests").join(test_name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("a test's folder");
    folder
}

/// `folder` made a world's: its world file written.
fn make_world(folder: &Path, planes: TestPlanes) {
    let world_info = world_folder_files::WorldInfo { seed: 1, tick: 0, plane_types: vec![planes.one_bit, planes.other_bit], side_in_superchunks: None, forced_hot: false, camera_forces_superchunks_hot: false };
    world_folder_files::write_world(folder, &world_info).expect("a world file");
}

/// `count` superchunks in a row from the world's middle, each a cold
/// superchunk of one cell set in one chunk of one of two planes, drawn:
/// all the same size.
fn drawn_cold_superchunks(planes: TestPlanes, seeded_random: &mut RandomSource, count: i32) -> Vec<(SuperchunkIndex, ColdSuperchunk)> {
    let mut one_cell = [0; WORDS];
    one_cell[0] = 1;
    let encoded_plane_chunk = BitmapCodec::new().encode_cells(&one_cell).to_vec();
    (0..count).map(|dx| {
        let type_id = [planes.one_bit, planes.other_bit][seeded_random.draw_below(2) as usize].type_id();
        let change = ColdPlaneChunkChange { place: seeded_random.draw_below(16) as usize, type_id, encoded_plane_chunk: &encoded_plane_chunk };
        (WORLD_MIDDLE.at_cartesian_offset(dx, 0).expect("in the world"), ColdSuperchunk::empty().rewritten_with(&[change]))
    }).collect()
}

/// Kept to three cold superchunks of eight, the pool pages out all but
/// one held and one with changes in the ring; each reads back as it went
/// in, from memory or the world's `paging/`; one brought in is in memory
/// again; a save holds them all; and the paging folder goes with the
/// pool, the world's file left.
#[test]
fn cold_superchunks_past_the_bytes_kept_go_to_disk_and_read_back_the_same() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (planes, mut seeded_random) = (TestPlanes::registered(seed), RandomSource::new(seed));
    let (world_folder, save_folder) = (test_folder("paging_world"), test_folder("paging_save"));
    make_world(&world_folder, planes);
    let cold_superchunks = drawn_cold_superchunks(planes, &mut seeded_random, 8);
    let bytes_each = std::mem::size_of_val(cold_superchunks[0].1.words()) as u64;
    let mut cold_storage = ColdStorage::new(1 << 12);
    cold_storage.page_in_world(&world_folder, 3 * bytes_each).expect("a world to page in");
    for (superchunk, cold_superchunk) in &cold_superchunks {
        cold_storage.insert(*superchunk, cold_superchunk.clone());
    }
    assert!(cold_storage.over_memory_kept());
    let held = cold_superchunks[seeded_random.draw_below(4) as usize].0;
    let in_ring = cold_superchunks[4 + seeded_random.draw_below(4) as usize].0;
    let mut flushed = Vec::new();
    cold_storage.write_back(PlaneChunk { plane_type: planes.one_bit, chunk: ChunkIndex::from_superchunk_place(in_ring, 0) }, BitmapCodec::new().encode_cells(&[u64::MAX; WORDS]), &mut flushed);
    assert_eq!(cold_storage.page_out(|superchunk| superchunk == held).expect("paged out"), 5);
    assert_eq!((cold_storage.on_disk(), cold_storage.bytes_in_memory()), (5, 3 * bytes_each));
    assert!(!cold_storage.is_on_disk(held) && !cold_storage.is_on_disk(in_ring), "the held one and the one in the ring stay");
    assert_eq!(cold_storage.page_out(|_| false).expect("nothing to do"), 0, "none goes when none is over");
    for (superchunk, cold_superchunk) in &cold_superchunks {
        assert_eq!(cold_storage.shared_cold_superchunk(*superchunk).expect("in the pool").words(), cold_superchunk.words(), "as it went in");
    }
    // Brought in: in memory again, over the bytes kept.
    let brought_in = cold_superchunks.iter().map(|(superchunk, _)| *superchunk).find(|superchunk| cold_storage.is_on_disk(*superchunk)).expect("five on disk");
    let read_back = ColdSuperchunk::clone(&cold_storage.shared_cold_superchunk(brought_in).expect("in the pool"));
    cold_storage.bring_in(brought_in, read_back);
    assert!(!cold_storage.is_on_disk(brought_in) && cold_storage.over_memory_kept());
    // Saved: every one, from memory or copied from the paging folder.
    for (superchunk, cold_superchunk) in cold_superchunks.iter().filter(|(superchunk, _)| *superchunk != in_ring) {
        cold_storage.save_cold_superchunk(&save_folder, *superchunk).expect("saved");
        assert_eq!(world_folder_files::read_cold_superchunk(&save_folder, *superchunk).expect("read").words(), cold_superchunk.words());
    }
    assert!(world_folder.join("paging").is_dir());
    drop(cold_storage);
    assert!(!world_folder.join("paging").exists(), "the paging folder gone with the pool");
    assert!(world_folder.join(world_folder_files::WORLD_FILE).is_file(), "the world left");
}

/// Paging is refused in a folder holding no world; in a world, what an
/// earlier run left in its paging folder is cleared when paging starts.
#[test]
fn paging_needs_a_world_and_starts_with_an_empty_paging_folder() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let planes = TestPlanes::registered(seed);
    let folder = test_folder("paging_needs_a_world");
    let mut cold_storage = ColdStorage::new(1 << 12);
    assert!(cold_storage.page_in_world(&folder, 0).is_err(), "no world, no paging");
    assert!(!folder.join("paging").exists() && !cold_storage.over_memory_kept());
    make_world(&folder, planes);
    let left_behind = folder.join("paging").join("superchunks").join("left_behind");
    std::fs::create_dir_all(left_behind.parent().expect("a folder")).expect("made");
    std::fs::write(&left_behind, [0; 64]).expect("written");
    cold_storage.page_in_world(&folder, 0).expect("a world to page in");
    assert!(folder.join("paging").is_dir() && !left_behind.exists(), "what was left behind cleared");
}
