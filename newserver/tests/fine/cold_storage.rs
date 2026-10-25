//! The cold side: bitmaps through the codec, cold superchunks holding a
//! plane chunk a type a chunk, the cold writeback ring, and the ring
//! flushed into the cold pool.

mod cold_pool_paging;

use crate::cold_storage::{BitmapCodec, ColdPlaneChunkChange, ColdStorage, ColdSuperchunk, ColdWritebackRing, InvalidColdSuperchunk, PlaneChunk};
use crate::registered_planes::TestPlanes;
use utilities::bitmap::{CellWords, WIDTH, WORDS};
use utilities::coordinates::{place_from_cartesian, ChunkIndex, SuperchunkIndex, WORLD_MIDDLE};
use utilities::rng::RandomSource;

/// A bitmap drawn, of one of many shapes: nothing, everything, a few
/// cells, most cells, a rectangle, a disc, noise, runs of whole words.
fn drawn_bitmap(seeded_random: &mut RandomSource) -> Box<CellWords> {
    let mut cells: Box<CellWords> = Box::new([0; WORDS]);
    let set = |cells: &mut CellWords, x: u32, y: u32| {
        let place = place_from_cartesian(x, y);
        cells[place / 64] |= 1 << (place % 64);
    };
    let side = WIDTH as u32;
    match seeded_random.draw_below(8) {
        0 => {}
        1 => cells.fill(u64::MAX),
        2 => (0..1 + seeded_random.draw_below(20)).for_each(|_| set(&mut cells, seeded_random.draw_below(side.into()) as u32, seeded_random.draw_below(side.into()) as u32)),
        3 => {
            cells.fill(u64::MAX);
            for _ in 0..1 + seeded_random.draw_below(20) {
                let place = seeded_random.draw_below((WORDS * 64) as u64) as usize;
                cells[place / 64] &= !(1 << (place % 64));
            }
        }
        4 => {
            let (left, top) = (seeded_random.draw_below(side.into()) as u32, seeded_random.draw_below(side.into()) as u32);
            let (right, bottom) = ((left + seeded_random.draw_below(80) as u32).min(side - 1), (top + seeded_random.draw_below(80) as u32).min(side - 1));
            (top..=bottom).for_each(|y| (left..=right).for_each(|x| set(&mut cells, x, y)));
        }
        5 => {
            let (centre_x, centre_y, radius) = (seeded_random.draw_below(side.into()) as i64, seeded_random.draw_below(side.into()) as i64, seeded_random.draw_below(60) as i64);
            for (x, y) in (0..i64::from(side)).flat_map(|y| (0..i64::from(side)).map(move |x| (x, y))) {
                if (x - centre_x).pow(2) + (y - centre_y).pow(2) <= radius * radius {
                    set(&mut cells, x as u32, y as u32);
                }
            }
        }
        6 => cells.iter_mut().for_each(|word| *word = seeded_random.next_draw()),
        _ => {
            let start = seeded_random.draw_below(WORDS as u64) as usize;
            let end = (start + seeded_random.draw_below(200) as usize).min(WORDS);
            cells[start..end].fill(u64::MAX);
        }
    }
    cells
}

/// `encoded`, decoded by `bitmap_codec`.
fn decoded(bitmap_codec: &mut BitmapCodec, encoded: &[u64]) -> Box<CellWords> {
    let mut cells: Box<CellWords> = Box::new([0; WORDS]);
    bitmap_codec.decode_cells(encoded, &mut cells);
    cells
}

/// Bitmaps of every shape, through one codec in any order, decode to
/// what was encoded whatever words follow the encoding; and nothing set
/// encodes to no words at all, when asked to be left out.
#[test]
fn bitmaps_decode_to_what_was_encoded() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (mut seeded_random, mut bitmap_codec) = (RandomSource::new(seed), BitmapCodec::new());
    for _ in 0..60 {
        let cells = drawn_bitmap(&mut seeded_random);
        let mut encoded = bitmap_codec.encode_cells(&cells).to_vec();
        // Whatever follows: another encoding, say.
        encoded.extend((0..seeded_random.draw_below(40)).map(|_| seeded_random.next_draw()));
        assert_eq!(decoded(&mut bitmap_codec, &encoded), cells, "a bitmap through the codec");
        assert_eq!(bitmap_codec.encode_unless_empty(&cells).is_empty(), cells.iter().all(|&word| word == 0), "left out only when empty");
    }
}

/// A cold superchunk holds one plane chunk a type a chunk, sorted by type
/// however they went in: a later change of one replaces the earlier, a
/// change of no words removes it, and it reads back the same from its own
/// words.
#[test]
fn a_cold_superchunk_holds_a_plane_chunk_a_type_a_chunk() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (planes, mut seeded_random, mut bitmap_codec) = (TestPlanes::registered(seed), RandomSource::new(seed), BitmapCodec::new());
    let type_ids = [planes.one_bit.type_id(), planes.other_bit.type_id()];
    let (place, mut held) = (seeded_random.draw_below(16) as usize, [None, None]);
    let mut cold_superchunk = ColdSuperchunk::empty();
    for _ in 0..12 {
        let which = seeded_random.draw_below(2) as usize;
        let cells = drawn_bitmap(&mut seeded_random);
        let encoded = bitmap_codec.encode_unless_empty(&cells).to_vec();
        cold_superchunk = cold_superchunk.rewritten_with(&[ColdPlaneChunkChange { place, type_id: type_ids[which], encoded_plane_chunk: &encoded }]);
        held[which] = (!encoded.is_empty()).then_some(cells);
        let mut held_types: Vec<u64> = (0..2).filter(|&type_held| held[type_held].is_some()).map(|type_held| type_ids[type_held]).collect();
        held_types.sort_unstable();
        assert_eq!(cold_superchunk.type_ids(place).collect::<Vec<_>>(), held_types, "one a type, sorted by type");
        for (type_id, cells_held) in type_ids.iter().zip(&held) {
            assert_eq!(cold_superchunk.plane_chunk(place, *type_id).map(|encoded| decoded(&mut bitmap_codec, encoded)), cells_held.clone(), "the last change of each type");
        }
        let read_back = ColdSuperchunk::from_words(cold_superchunk.words().into()).expect("its own words");
        assert_eq!(read_back.words(), cold_superchunk.words(), "read back from its words");
    }
}

/// Words that are no cold superchunk -- too short, or a plane chunk table
/// pointing outside its chunk -- are refused.
#[test]
fn words_that_are_no_cold_superchunk_are_refused() {
    let refused = |words: Vec<u64>| -> Option<InvalidColdSuperchunk> { ColdSuperchunk::from_words(words.into_boxed_slice()).err() };
    assert!(refused(Vec::new()).is_some(), "no words");
    assert!(refused(vec![u64::MAX; 3]).is_some(), "noise");
    let mut bitmap_codec = BitmapCodec::new();
    let encoded = bitmap_codec.encode_cells(&[u64::MAX; WORDS]).to_vec();
    let good = ColdSuperchunk::empty().rewritten_with(&[ColdPlaneChunkChange { place: 3, type_id: 7, encoded_plane_chunk: &encoded }]);
    let words = good.words();
    assert!(refused(words[..words.len() - 1].to_vec()).is_some(), "cut short");
    assert!(refused(words.to_vec()).is_none(), "its own words are not refused");
}

/// The ring hands back a superchunk's entries in the order written, frees
/// them from its tail, wraps round its end, and refuses an entry with no
/// room.
#[test]
fn the_ring_frees_from_its_tail_and_wraps() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_random = RandomSource::new(seed);
    let superchunks: Vec<SuperchunkIndex> = (0..3).map(|dx| WORLD_MIDDLE.at_cartesian_offset(dx, 0).expect("in the world")).collect();
    let entry = |random: &mut RandomSource| (0..1 + random.draw_below(6)).map(|_| random.next_draw()).collect::<Vec<u64>>();
    let mut ring = ColdWritebackRing::new(64);
    let mut written: Vec<(SuperchunkIndex, Vec<u64>)> = Vec::new();
    // Filled until an entry no longer fits.
    loop {
        let (superchunk, words) = (superchunks[written.len() % 2], entry(&mut seeded_random));
        if !ring.push(ChunkIndex::from_superchunk_place(superchunk, written.len() % 16), written.len() as u64, &words) {
            break;
        }
        written.push((superchunk, words));
    }
    assert!(written.len() >= 2, "a few entries fit");
    for &superchunk in &superchunks[..2] {
        let held: Vec<Vec<u64>> = ring.entries_of(superchunk).iter().map(|ring_entry| ring.encoded_plane_chunk(ring_entry).to_vec()).collect();
        let expected: Vec<Vec<u64>> = written.iter().filter(|(written_superchunk, _)| *written_superchunk == superchunk).map(|(_, words)| words.clone()).collect();
        assert_eq!(held, expected, "a superchunk's entries, in the order written");
    }
    assert_eq!(ring.tail_superchunk(), Some(superchunks[0]), "the oldest entry's superchunk at the tail");
    ring.release_superchunk(superchunks[0]);
    assert_eq!(ring.tail_superchunk(), Some(superchunks[1]), "freed from the tail");
    // Room again, at the ring's start: it wraps.
    let wrapped = vec![seeded_random.next_draw()];
    assert!(ring.push(ChunkIndex::from_superchunk_place(superchunks[2], 0), 99, &wrapped), "room freed is used again");
    assert_eq!(ring.entries_of(superchunks[2]).iter().map(|ring_entry| ring.encoded_plane_chunk(ring_entry).to_vec()).collect::<Vec<_>>(), vec![wrapped]);
    ring.release_superchunk(superchunks[1]);
    ring.release_superchunk(superchunks[2]);
    assert!(ring.is_empty());
}

/// Written back, a plane chunk is in the ring, not yet the cold pool;
/// flushed, its superchunk's cold superchunk holds it and the ring holds
/// nothing of it -- and one written back empty is gone from it. A full
/// ring flushes its tail to make room.
#[test]
fn the_ring_flushes_into_the_cold_pool() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let (planes, mut seeded_random, mut bitmap_codec) = (TestPlanes::registered(seed), RandomSource::new(seed), BitmapCodec::new());
    let mut cold_storage = ColdStorage::new(1 << 12);
    let plane_chunk = PlaneChunk { plane_type: planes.one_bit, chunk: ChunkIndex::from_superchunk_place(WORLD_MIDDLE, 5) };
    let mut cells = drawn_bitmap(&mut seeded_random);
    cells[0] |= 1;
    let mut flushed = Vec::new();
    cold_storage.write_back(plane_chunk, bitmap_codec.encode_unless_empty(&cells), &mut flushed);
    assert!(flushed.is_empty() && cold_storage.plane_chunk(plane_chunk).is_none(), "in the ring, not the cold pool");
    assert!(cold_storage.holds_changes(WORLD_MIDDLE));
    assert!(cold_storage.flush(WORLD_MIDDLE));
    assert_eq!(cold_storage.plane_chunk(plane_chunk).map(|encoded| decoded(&mut bitmap_codec, encoded)), Some(cells), "flushed into its cold superchunk");
    assert!(!cold_storage.holds_changes(WORLD_MIDDLE) && cold_storage.nothing_to_flush(), "nothing left in the ring");
    cold_storage.write_back(plane_chunk, &[], &mut flushed);
    cold_storage.flush(WORLD_MIDDLE);
    assert!(cold_storage.plane_chunk(plane_chunk).is_none(), "written back empty, gone");
    // Written back over and over, more than the ring holds: its tail flushed as it fills, the last kept.
    let mut last = Box::new([0; WORDS]);
    for _ in 0..40 {
        last = drawn_bitmap(&mut seeded_random);
        last[0] |= 1;
        cold_storage.write_back(plane_chunk, bitmap_codec.encode_unless_empty(&last), &mut flushed);
    }
    assert!(!flushed.is_empty(), "a full ring flushes its tail");
    let mut flushed_all = Vec::new();
    cold_storage.flush_all(&mut flushed_all);
    assert_eq!(cold_storage.plane_chunk(plane_chunk).map(|encoded| decoded(&mut bitmap_codec, encoded)), Some(last), "the last written back held");
}
