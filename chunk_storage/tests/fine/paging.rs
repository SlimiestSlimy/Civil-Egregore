//! The cold pool paged to disk: images past what it keeps in memory
//! written to a folder of its own and read back the same, and folders
//! left behind removed.

use chunk_storage::transient_data::TRANSIENT_DATA;
use chunk_storage::{disk, ChunkStorage, HeightMap, LayerCodec, LayerType, Stored, SuperchunkImage};
use coordinates::{ChunkIndex, SuperchunkIndex};
use std::path::PathBuf;
use std::sync::Arc;
use utilities::rng::Rng;

/// A folder of its own for the test `name`, emptied.
fn folder(name: &str) -> PathBuf {
    let folder = TRANSIENT_DATA.under("tests").join(name);
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// `count` superchunks drawn, no two the same, each with an image of
/// heights drawn: all the same size.
fn images_drawn(random: &mut Rng, count: usize) -> Vec<(SuperchunkIndex, SuperchunkImage)> {
    let mut superchunks: Vec<u64> = Vec::new();
    while superchunks.len() < count {
        let drawn = random.below(1 << 40);
        if !superchunks.contains(&drawn) {
            superchunks.push(drawn);
        }
    }
    superchunks.into_iter().map(|superchunk| (SuperchunkIndex(superchunk), SuperchunkImage::new(&HeightMap::filled(random.below(200) as _)))).collect()
}

/// What is in `folder`: how many entries.
fn entries(folder: &std::path::Path) -> usize {
    std::fs::read_dir(folder).map_or(0, |entries| entries.count())
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
    let mut random = Rng::new(utilities::seed::counted());
    let (root, save, other) = (folder("paging_root"), folder("paging_save"), folder("paging_other"));
    let images = images_drawn(&mut random, 12);
    let one = std::mem::size_of_val(images[0].1.words()) as u64;
    let mut storage = ChunkStorage::new(1 << 12);
    storage.page_under(root.clone(), 3 * one);
    for (superchunk, image) in &images {
        assert!(!storage.over_memory_kept() || storage.bytes_in_memory() > 3 * one);
        storage.insert(*superchunk, image.clone());
    }
    assert_eq!(storage.bytes_in_memory(), 12 * one);
    // One held by its caller, one by its changes in the ring: drawn.
    let (held, in_ring) = (images[random.below(6) as usize].0, images[6 + random.below(6) as usize].0);
    let (mut codec, mut flushed) = (LayerCodec::new(), Vec::new());
    storage.write_back(ChunkIndex::of(in_ring, random.below(16) as usize), LayerType(1), codec.encode(&[u64::MAX; bitmap::WORDS]), &mut flushed);
    assert!(flushed.is_empty());
    assert_eq!(storage.page_out(|superchunk| superchunk == held).expect("paged out"), 9);
    assert!(!storage.over_memory_kept() && storage.on_disk() == 9 && storage.bytes_in_memory() == 3 * one);
    assert!(!storage.is_on_disk(held) && !storage.is_on_disk(in_ring), "held, each its way");
    assert_eq!(storage.page_out(|_| false).expect("nothing to do"), 0, "no more than it keeps: none goes");
    for (superchunk, image) in &images {
        assert!(storage.holds(*superchunk));
        assert_eq!(storage.shared_image(*superchunk).expect("held").words(), image.words(), "as it was put in");
        assert_eq!(matches!(storage.stored(*superchunk), Some(Stored::OnDisk(_))), storage.is_on_disk(*superchunk));
    }
    // One read back and brought in: in memory, and over what is kept until another goes.
    let back = images.iter().map(|(superchunk, _)| *superchunk).find(|superchunk| storage.is_on_disk(*superchunk)).expect("nine on disk");
    let read = SuperchunkImage::clone(&storage.shared_image(back).expect("held"));
    storage.bring_in(back, read);
    assert!(!storage.is_on_disk(back) && storage.image(back).is_some() && storage.bytes_in_memory() == 4 * one && storage.over_memory_kept());
    // The third that had stayed goes for it. The one in the ring goes only once flushed, its image the larger by a layer.
    assert_eq!(storage.page_out(|superchunk| superchunk == held || superchunk == back).expect("paged out"), 1);
    assert!(!storage.is_on_disk(in_ring) && !storage.over_memory_kept());
    assert!(storage.flush(in_ring) && storage.over_memory_kept());
    let rewritten = SuperchunkImage::clone(&storage.shared_image(in_ring).expect("held"));
    assert_eq!(storage.page_out(|superchunk| superchunk == held || superchunk == back).expect("paged out"), 1);
    assert!(storage.is_on_disk(in_ring));
    assert_eq!(storage.shared_image(in_ring).expect("held").words(), rewritten.words(), "as flushed");
    assert_eq!(entries(&root), 2, "one paging folder, and its lock");

    // Saved: every image in the save, from memory or copied from the paging folder.
    for (superchunk, image) in &images {
        let expected = if *superchunk == in_ring { &rewritten } else { image };
        assert_eq!(storage.save_image(&save, *superchunk).expect("saved"), 8 * expected.words().len() as u64);
        assert_eq!(disk::read_image(&save, *superchunk).expect("read").words(), expected.words());
    }
    // A pool of images left in the save: read from there, saved there as they are, copied elsewhere.
    let (mut loaded, saved_in) = (ChunkStorage::new(1 << 12), Arc::<std::path::Path>::from(save.as_path()));
    for (superchunk, _) in &images {
        loaded.insert_on_disk(*superchunk, &saved_in);
    }
    assert_eq!((loaded.on_disk(), loaded.bytes_in_memory()), (12, 0));
    for (superchunk, _) in &images {
        let words = storage.shared_image(*superchunk).expect("held");
        assert_eq!(loaded.shared_image(*superchunk).expect("held").words(), words.words());
        assert_eq!(loaded.save_image(&save, *superchunk).expect("left"), 8 * words.words().len() as u64);
        assert_eq!(loaded.save_image(&other, *superchunk).expect("copied"), 8 * words.words().len() as u64);
        assert_eq!(disk::read_image(&other, *superchunk).expect("read").words(), words.words());
    }
    drop(storage);
    assert_eq!(entries(&root), 0, "the paging folder gone with its pool");
}

/// A paging folder left behind by a process gone -- its lock held by
/// nobody -- is removed when the next one is made; one whose pool is
/// there is left, and reads back.
#[test]
fn a_paging_folder_left_behind_is_removed_and_one_in_use_is_left() {
    let mut random = Rng::new(utilities::seed::counted());
    let root = folder("paging_left_behind");
    let images = images_drawn(&mut random, 2);
    let paged = |image: &(SuperchunkIndex, SuperchunkImage)| {
        let mut storage = ChunkStorage::new(1 << 12);
        storage.page_under(root.clone(), 0);
        storage.insert(image.0, image.1.clone());
        assert_eq!(storage.page_out(|_| false).expect("paged out"), 1);
        storage
    };
    let first = paged(&images[0]);
    assert_eq!(entries(&root), 2);
    let left = root.join("left_behind");
    std::fs::create_dir_all(left.join("superchunks")).expect("made");
    std::fs::write(left.join("superchunks").join("0.image"), [0; 64]).expect("written");
    std::fs::write(left.with_extension("lock"), []).expect("written");
    let second = paged(&images[1]);
    assert!(!left.exists() && !left.with_extension("lock").exists(), "the one left behind removed");
    assert_eq!(entries(&root), 4, "two pools' folders and locks");
    assert_eq!(first.shared_image(images[0].0).expect("held").words(), images[0].1.words(), "the first pool's, left");
    assert_eq!(second.shared_image(images[1].0).expect("held").words(), images[1].1.words());
    drop((first, second));
    assert_eq!(entries(&root), 0);
}
