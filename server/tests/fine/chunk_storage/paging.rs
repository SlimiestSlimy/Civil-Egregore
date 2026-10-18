//! The cold pool paged to disk: images past what it keeps in memory
//! written to a folder of its own and read back the same, and folders
//! left behind removed.

use crate::chunk_storage::{disk, ChunkStorage, ImageStored, LayerChange, LayerCodec, LayerType, SuperchunkImage};
use utilities::coordinates::{ChunkIndex, SuperchunkIndex};
use std::path::PathBuf;
use std::sync::Arc;
use utilities::rng::RandomSource;

/// A folder of its own for the test `test_name`, in the crate's
/// `transient_data/`, emptied.
fn folder(test_name: &str) -> PathBuf {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("transient_data").join("tests").join(test_name);
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// `count` superchunks drawn, no two the same, each with an image of
/// a cell of a layer drawn: all the same size.
fn images_drawn(seeded_random: &mut RandomSource, count: usize) -> Vec<(SuperchunkIndex, SuperchunkImage)> {
    let mut superchunks: Vec<u64> = Vec::new();
    while superchunks.len() < count {
        let drawn_superchunk = seeded_random.draw_below(1 << 40);
        if !superchunks.contains(&drawn_superchunk) {
            superchunks.push(drawn_superchunk);
        }
    }
    let mut layer_codec = LayerCodec::new();
    let mut one_cell = [0; utilities::bitmap::WORDS];
    one_cell[0] = 1;
    let encoded_layer = layer_codec.encode_cells(&one_cell).to_vec();
    superchunks.into_iter().map(|superchunk| (SuperchunkIndex(superchunk), SuperchunkImage::empty().rewritten_with(&[LayerChange { place: 0, layer_type: LayerType(2 + seeded_random.draw_below(200)), encoded_layer: &encoded_layer }]))).collect()
}

/// What is in `folder`: how many entries.
fn count_entries(folder: &std::path::Path) -> usize {
    std::fs::read_dir(folder).map_or(0, |folder_entries| folder_entries.count())
}

/// A pool keeping three images of twelve pages the rest out, none of
/// a superchunk held or with changes in the ring; every image reads
/// back as it was put in, from memory or disk; one brought in is in
/// memory again; the one in the ring goes once flushed; a save holds
/// every image, copied file to file where it was on disk; and a pool
/// of images left in a save reads them from there, saves them there
/// by leaving them, and elsewhere by copying. Its paging folder is
/// gone with it.
#[test]
fn images_past_what_is_kept_go_to_disk_and_read_back_the_same() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_random = RandomSource::new(seed);
    let (paging_root, save, other_save) = (folder("paging_root"), folder("paging_save"), folder("paging_other"));
    let images = images_drawn(&mut seeded_random, 12);
    let image_bytes = std::mem::size_of_val(images[0].1.words()) as u64;
    let mut cold_storage = ChunkStorage::new(1 << 12);
    cold_storage.page_under(paging_root.clone(), 3 * image_bytes);
    for (superchunk, image) in &images {
        assert!(!cold_storage.over_memory_kept() || cold_storage.bytes_in_memory() > 3 * image_bytes);
        cold_storage.insert(*superchunk, image.clone());
    }
    assert_eq!(cold_storage.bytes_in_memory(), 12 * image_bytes);
    // One held by its caller, one by its changes in the ring: drawn.
    let (held_superchunk, in_ring) = (images[seeded_random.draw_below(6) as usize].0, images[6 + seeded_random.draw_below(6) as usize].0);
    let (mut layer_codec, mut flushed_superchunks) = (LayerCodec::new(), Vec::new());
    cold_storage.write_back(ChunkIndex::from_superchunk_place(in_ring, seeded_random.draw_below(16) as usize), LayerType(1), layer_codec.encode_cells(&[u64::MAX; utilities::bitmap::WORDS]), &mut flushed_superchunks);
    assert!(flushed_superchunks.is_empty());
    assert_eq!(cold_storage.page_out(|superchunk| superchunk == held_superchunk).expect("paged out"), 9);
    assert!(!cold_storage.over_memory_kept() && cold_storage.on_disk() == 9 && cold_storage.bytes_in_memory() == 3 * image_bytes);
    assert!(!cold_storage.is_on_disk(held_superchunk) && !cold_storage.is_on_disk(in_ring), "held, each its way");
    assert_eq!(cold_storage.page_out(|_| false).expect("nothing to do"), 0, "no more than it keeps: none goes");
    for (superchunk, image) in &images {
        assert!(cold_storage.holds_image(*superchunk));
        assert_eq!(cold_storage.shared_image(*superchunk).expect("held").words(), image.words(), "as it was put in");
        assert_eq!(matches!(cold_storage.stored_image(*superchunk), Some(ImageStored::OnDisk(_))), cold_storage.is_on_disk(*superchunk));
    }
    // One read back and brought in: in memory, and over what is kept until another goes.
    let brought_back = images.iter().map(|(superchunk, _)| *superchunk).find(|superchunk| cold_storage.is_on_disk(*superchunk)).expect("nine on disk");
    let image_read = SuperchunkImage::clone(&cold_storage.shared_image(brought_back).expect("held"));
    cold_storage.bring_in(brought_back, image_read);
    assert!(!cold_storage.is_on_disk(brought_back) && cold_storage.image(brought_back).is_some() && cold_storage.bytes_in_memory() == 4 * image_bytes && cold_storage.over_memory_kept());
    // The third that had stayed goes for it. The one in the ring goes only once flushed, its image the larger by a layer.
    assert_eq!(cold_storage.page_out(|superchunk| superchunk == held_superchunk || superchunk == brought_back).expect("paged out"), 1);
    assert!(!cold_storage.is_on_disk(in_ring) && !cold_storage.over_memory_kept());
    assert!(cold_storage.flush(in_ring) && cold_storage.over_memory_kept());
    let rewritten_image = SuperchunkImage::clone(&cold_storage.shared_image(in_ring).expect("held"));
    assert_eq!(cold_storage.page_out(|superchunk| superchunk == held_superchunk || superchunk == brought_back).expect("paged out"), 1);
    assert!(cold_storage.is_on_disk(in_ring));
    assert_eq!(cold_storage.shared_image(in_ring).expect("held").words(), rewritten_image.words(), "as flushed");
    assert_eq!(count_entries(&paging_root), 2, "one paging folder, and its lock");

    // Saved: every image in the save, from memory or copied from the paging folder.
    for (superchunk, image) in &images {
        let expected_image = if *superchunk == in_ring { &rewritten_image } else { image };
        assert_eq!(cold_storage.save_image(&save, *superchunk).expect("saved"), 8 * expected_image.words().len() as u64);
        assert_eq!(disk::read_image(&save, *superchunk).expect("read").words(), expected_image.words());
    }
    // A pool of images left in the save: read from there, saved there as they are, copied elsewhere.
    let (mut loaded_storage, saved_in) = (ChunkStorage::new(1 << 12), Arc::<std::path::Path>::from(save.as_path()));
    for (superchunk, _) in &images {
        loaded_storage.insert_on_disk(*superchunk, &saved_in);
    }
    assert_eq!((loaded_storage.on_disk(), loaded_storage.bytes_in_memory()), (12, 0));
    for (superchunk, _) in &images {
        let words = cold_storage.shared_image(*superchunk).expect("held");
        assert_eq!(loaded_storage.shared_image(*superchunk).expect("held").words(), words.words());
        assert_eq!(loaded_storage.save_image(&save, *superchunk).expect("left"), 8 * words.words().len() as u64);
        assert_eq!(loaded_storage.save_image(&other_save, *superchunk).expect("copied"), 8 * words.words().len() as u64);
        assert_eq!(disk::read_image(&other_save, *superchunk).expect("read").words(), words.words());
    }
    drop(cold_storage);
    assert_eq!(count_entries(&paging_root), 0, "the paging folder gone with its pool");
}

/// A paging folder left behind by a process gone -- its lock held by
/// nobody -- is removed when the next one is made; one whose pool is
/// there is left, and reads back.
#[test]
fn a_paging_folder_left_behind_is_removed_and_one_in_use_is_left() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_random = RandomSource::new(seed);
    let paging_root = folder("paging_left_behind");
    let images = images_drawn(&mut seeded_random, 2);
    let paged = |image: &(SuperchunkIndex, SuperchunkImage)| {
        let mut cold_storage = ChunkStorage::new(1 << 12);
        cold_storage.page_under(paging_root.clone(), 0);
        cold_storage.insert(image.0, image.1.clone());
        assert_eq!(cold_storage.page_out(|_| false).expect("paged out"), 1);
        cold_storage
    };
    let first_storage = paged(&images[0]);
    assert_eq!(count_entries(&paging_root), 2);
    let left_behind = paging_root.join("left_behind");
    std::fs::create_dir_all(left_behind.join("superchunks")).expect("made");
    std::fs::write(left_behind.join("superchunks").join("0.image"), [0; 64]).expect("written");
    std::fs::write(left_behind.with_extension("lock"), []).expect("written");
    let second_storage = paged(&images[1]);
    assert!(!left_behind.exists() && !left_behind.with_extension("lock").exists(), "the one left behind removed");
    assert_eq!(count_entries(&paging_root), 4, "two pools' folders and locks");
    assert_eq!(first_storage.shared_image(images[0].0).expect("held").words(), images[0].1.words(), "the first pool's, left");
    assert_eq!(second_storage.shared_image(images[1].0).expect("held").words(), images[1].1.words());
    drop((first_storage, second_storage));
    assert_eq!(count_entries(&paging_root), 0);
}
