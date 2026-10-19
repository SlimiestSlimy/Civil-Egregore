//! Chunks as stored: an area as generated, the layer codec,
//! superchunk images, the writeback ring and the cold pool it feeds.

use utilities::bitmap::{Bitmap, CellWords, WORDS};
use crate::test_types::TestTypes;
use crate::chunk_storage::{ChunkStorage, GeneratedArea, InvalidImage, LayerChange, LayerCodec, LayerType, SuperchunkImage, WritebackRing};
use utilities::coordinates::{place_from_cartesian, CellCartesian, ChunkIndex, SuperchunkIndex, CELLS_IN_CHUNK, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};
use utilities::rng::RandomSource;

/// A cell of a chunk, cartesian: across and down from its top left.
const CELL: (u8, u8) = (3, 200);

/// A bitmap's cells, with a rectangle and a circle drawn.
fn rectangle_and_circle() -> CellWords {
    let mut bitmap = Bitmap::new();
    bitmap.set_rect(10, 10, 40, 30);
    bitmap.set_circle(180, 180, 25);
    *bitmap.words()
}

/// A bitmap's cells with only `cell` set.
fn one_cell((x, y): (u8, u8)) -> CellWords {
    let mut bitmap = Bitmap::new();
    bitmap.set(x, y);
    *bitmap.words()
}

/// `words` decoded.
fn decoded_cells(layer_codec: &mut LayerCodec, words: &[u64]) -> CellWords {
    let mut cells = [u64::MAX; WORDS];
    layer_codec.decode_cells(words, &mut cells);
    cells
}

/// A layer comes back from its encoding cell for cell, whatever words
/// follow it, and an empty one takes a word, not a bitmap's worth.
#[test]
fn layers_decode_to_what_was_encoded_whatever_follows() {
    let mut layer_codec = LayerCodec::new();
    let cells = rectangle_and_circle();
    let encoded_layer = layer_codec.encode_cells(&cells).to_vec();
    let encoded_and_followed: Vec<u64> = [&encoded_layer[..], &[u64::MAX; 4], layer_codec.encode_cells(&one_cell(CELL))].concat();
    assert_eq!(decoded_cells(&mut layer_codec, &encoded_and_followed), cells);
    assert_eq!(layer_codec.encode_cells(&[0; WORDS]).len(), 1);
}

/// Bitmaps drawn -- a few cells, most cells, rectangles and discs,
/// words of noise, runs of whole words, nothing and everything --
/// each come back from their encoding cell for cell, whatever
/// words follow, in any order through one codec.
#[test]
fn layers_drawn_decode_to_what_was_encoded() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let mut seeded_random = RandomSource::new(seed);
    let mut layer_codec = LayerCodec::new();
    for case_number in 0..200 {
        let mut bitmap = Bitmap::new();
        let random_byte = |byte_random: &mut RandomSource| byte_random.draw_below(256) as u8;
        match case_number % 8 {
            0 => (0..seeded_random.draw_below(40)).for_each(|_| bitmap.set(random_byte(&mut seeded_random), random_byte(&mut seeded_random))),
            1 => (0..seeded_random.draw_below(40_000)).for_each(|_| bitmap.set(random_byte(&mut seeded_random), random_byte(&mut seeded_random))),
            2 => (0..seeded_random.draw_between(1, 6)).for_each(|_| {
                // Two corners anywhere, past the bitmap's edges too.
                let mut random_corner = || seeded_random.draw_below(400) as i64 - 72;
                bitmap.set_rect(random_corner(), random_corner(), random_corner(), random_corner());
            }),
            3 => (0..seeded_random.draw_between(1, 6)).for_each(|_| bitmap.set_circle(seeded_random.draw_below(256) as i64, seeded_random.draw_below(256) as i64, seeded_random.draw_below(80) as i64)),
            _ => {}
        }
        let mut cells = *bitmap.words();
        match case_number % 8 {
            4 => cells.iter_mut().for_each(|word| *word = seeded_random.next_draw()),
            5 => cells.iter_mut().for_each(|word| *word = [0, u64::MAX, seeded_random.next_draw()][seeded_random.draw_below(3) as usize]),
            6 => cells = [u64::MAX; WORDS],
            // All but a few cells set.
            7 => cells.iter_mut().for_each(|word| *word = !(u64::from(seeded_random.draw_below(50) == 0) << seeded_random.draw_below(64))),
            _ => {}
        }
        let encoded_layer = layer_codec.encode_cells(&cells).to_vec();
        let encoded_and_followed: Vec<u64> = [&encoded_layer[..], &(0..seeded_random.draw_below(5)).map(|_| seeded_random.next_draw()).collect::<Vec<_>>()].concat();
        assert!(decoded_cells(&mut layer_codec, &encoded_and_followed) == cells, "case {case_number}: {} cells set, {} words encoded", cells.iter().map(|word| word.count_ones()).sum::<u32>(), encoded_layer.len());
    }
}

/// Numbers set on wide planes come back from the image made of them:
/// a bit a layer, at the cell's place -- on a plane of tiles, at its
/// tile's -- the last number set on a cell kept, and a layer with no
/// bit set not kept at all.
#[test]
fn numbers_generated_come_back_from_the_image() {
    let seed = crate::test_seed::TestSeedHandle::open().fine_seed();
    let types = TestTypes::seeded(seed);
    let mut seeded_random = RandomSource::new(seed);
    let (cell_plane, tile_plane) = (types.sixteen_bits, types.four_bit_tiles);
    let mut generated_area = GeneratedArea::of_superchunk(WORLD_MIDDLE);
    let area = generated_area.area();
    let drawn_cell = |seeded_random: &mut RandomSource| CellCartesian { x: area.top_left.x + seeded_random.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32, y: area.top_left.y + seeded_random.draw_below(u64::from(SUPERCHUNK_SIDE_CELLS)) as u32 };
    let mut numbers_set = std::collections::HashMap::new();
    for _ in 0..2_000 {
        let cell = drawn_cell(&mut seeded_random);
        // Set twice, the second number the one kept.
        generated_area.set_value(cell_plane, cell, seeded_random.draw_below(1 << 16) as u32);
        let number = seeded_random.draw_below(1 << 15) as u32;
        generated_area.set_value(cell_plane, cell, number);
        numbers_set.insert(cell, number);
    }
    let (tile_cell, tile_number) = (drawn_cell(&mut seeded_random), 0b1010);
    generated_area.set_value(tile_plane, tile_cell, tile_number);
    let image = generated_area.image(&mut LayerCodec::new());
    let mut layer_codec = LayerCodec::new();
    // Each plane under its one ID, its bits in order: decoded whole, its number read at a place.
    let mut number_at = |layer_type: LayerType, place: usize| {
        image.layer(place / CELLS_IN_CHUNK, layer_type.type_id()).map_or(0, |words| {
            let mut bucket = vec![0; crate::chunk_storage::wide::bucket_words(layer_type)];
            crate::chunk_storage::wide::decode_bucket(layer_type, words, &mut layer_codec, &mut bucket);
            protocol::pipe::frame::value_at(&bucket, layer_type.width_bits(), place % CELLS_IN_CHUNK)
        })
    };
    for (cell, number) in numbers_set {
        let place = place_from_cartesian(cell.x - area.top_left.x, cell.y - area.top_left.y);
        assert_eq!(number_at(cell_plane.layer_type(), place), number, "{cell:?}");
    }
    let place = place_from_cartesian(tile_cell.x - area.top_left.x, tile_cell.y - area.top_left.y);
    let tile_place = place / CELLS_IN_CHUNK * CELLS_IN_CHUNK + ((place % CELLS_IN_CHUNK) >> tile_plane.layer_type().tile_shift());
    assert_eq!(number_at(tile_plane.layer_type(), tile_place), tile_number);
}

/// An image's layers come out by type, one a type, however they went
/// in; a later change replaces an earlier one, and a change of no words
/// removes the layer. The image read back from its words is the same.
#[test]
fn images_hold_one_layer_a_type_in_type_order() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let mut layer_codec = LayerCodec::new();
    let (drawn_words, one_cell_words) = (layer_codec.encode_cells(&rectangle_and_circle()).to_vec(), layer_codec.encode_cells(&one_cell(CELL)).to_vec());
    let place = 13;
    let id_of = |place_among: usize| types.layers[place_among].type_id();
    let layer_change = |type_id, words| LayerChange { place, type_id, encoded_layer: words };
    let image = SuperchunkImage::empty().rewritten_with(&[
        layer_change(id_of(3), &one_cell_words),
        layer_change(id_of(2), &drawn_words),
        layer_change(id_of(4), &one_cell_words),
        layer_change(id_of(0), &drawn_words),
        layer_change(id_of(3), &drawn_words),
        layer_change(id_of(0), &[]),
        layer_change(id_of(1), &[]),
    ]);
    let mut held_in_type_order = [id_of(2), id_of(3), id_of(4)];
    held_in_type_order.sort_unstable();
    assert_eq!(image.type_ids(place).collect::<Vec<_>>(), held_in_type_order);
    for (type_id, cells) in [(id_of(2), rectangle_and_circle()), (id_of(3), rectangle_and_circle()), (id_of(4), one_cell(CELL))] {
        assert_eq!(decoded_cells(&mut layer_codec, image.layer(place, type_id).expect("held")), cells, "type {type_id}");
    }
    assert!(image.layer(place, id_of(0)).is_none());
    assert!((0..16).filter(|&other_place| other_place != place).all(|checked_place| image.type_ids(checked_place).count() == 0));
    assert_eq!(image.words()[0] as usize, 16, "the first chunk after the chunk table");

    let read_back = SuperchunkImage::from_words(image.words().into()).expect("an image");
    assert_eq!(read_back, image);
    // Rewriting again keeps every bitmap as it was.
    assert_eq!(image.rewritten_with(&[]), image);
}

/// Words that are not an image are refused.
#[test]
fn broken_images_are_refused() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let mut layer_codec = LayerCodec::new();
    let one_cell_words = layer_codec.encode_cells(&one_cell(CELL)).to_vec();
    let image = SuperchunkImage::empty().rewritten_with(&[LayerChange { place: 15, type_id: types.layers[0].type_id(), encoded_layer: &one_cell_words }]);
    let words = image.words();
    assert!(SuperchunkImage::from_words(words[..10].into()).is_err(), "shorter than its chunk table");
    let mut bad_chunk_offset = words.to_vec();
    bad_chunk_offset[3] += 1;
    assert!(SuperchunkImage::from_words(bad_chunk_offset.into()).is_err());
    let last_chunk = words[15] as usize;
    let mut layer_outside = words.to_vec();
    layer_outside[last_chunk + 2] = (words.len() - last_chunk) as u64;
    assert_eq!(SuperchunkImage::from_words(layer_outside.into()), Err(InvalidImage("an encoded layer outside its chunk, or two at one offset")));
}

/// The ring hands back each superchunk's entries in the order written,
/// frees them from its tail, wraps round its end, and refuses an entry
/// with no room for it.
#[test]
fn the_ring_frees_from_its_tail_and_wraps() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let mut writeback_ring = WritebackRing::new(40);
    let (superchunk_a, superchunk_b) = (SuperchunkIndex::from_cartesian(1, 0), SuperchunkIndex::from_cartesian(0, 1));
    let chunk = ChunkIndex::from_superchunk_place;
    let id_of = |place_among: usize| types.layers[place_among].type_id();
    assert!(writeback_ring.push(chunk(superchunk_a, 1), id_of(0), &[11; 5]));
    assert!(writeback_ring.push(chunk(superchunk_b, 2), id_of(1), &[22; 5]));
    assert!(writeback_ring.push(chunk(superchunk_a, 3), id_of(2), &[33; 5]));
    assert!(!writeback_ring.push(chunk(superchunk_b, 4), id_of(3), &[44; 20]), "no room");
    assert_eq!(writeback_ring.tail_superchunk(), Some(superchunk_a));
    let entries_of_a = writeback_ring.entries_of(superchunk_a);
    assert_eq!(entries_of_a.iter().map(|ring_entry| (ring_entry.place, ring_entry.type_id, writeback_ring.encoded_layer(ring_entry)[0])).collect::<Vec<_>>(), [
        (1, id_of(0), 11),
        (3, id_of(2), 33)
    ]);
    writeback_ring.release_superchunk(superchunk_a);
    assert_eq!(writeback_ring.tail_superchunk(), Some(superchunk_b), "b's entry is the tail now");
    // Words 24 to 37 go to the next entry; the one after it does not fit
    // before the end, and wraps into the 8 words freed before b's.
    assert!(writeback_ring.push(chunk(superchunk_a, 5), id_of(4), &[55; 10]));
    assert!(writeback_ring.push(chunk(superchunk_a, 6), id_of(5), &[66; 2]));
    assert!(!writeback_ring.push(chunk(superchunk_a, 7), id_of(6), &[77; 1]), "full up to b's entry");
    assert_eq!(writeback_ring.entries_of(superchunk_a).iter().map(|later_entry| writeback_ring.encoded_layer(later_entry)[0]).collect::<Vec<_>>(), [55, 66]);
    writeback_ring.release_superchunk(superchunk_b);
    assert_eq!(writeback_ring.tail_superchunk(), Some(superchunk_a));
    writeback_ring.release_superchunk(superchunk_a);
    assert!(writeback_ring.is_empty() && writeback_ring.tail_superchunk().is_none());
    assert!(writeback_ring.push(chunk(superchunk_b, 0), id_of(8), &[99; 37]), "an empty ring starts over");
}

/// Written back, a bitmap is in the ring and not yet in the cold pool; flushed,
/// its superchunk's image holds it, and the ring is empty.
/// A layer written back empty is removed.
#[test]
fn flushing_writes_the_ring_into_the_cold_pool() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (mut layer_codec, mut cold_storage, mut flushed_superchunks) = (LayerCodec::new(), ChunkStorage::new(1 << 12), Vec::new());
    cold_storage.insert(WORLD_MIDDLE, SuperchunkImage::empty());
    let chunk = ChunkIndex::from_superchunk_place(WORLD_MIDDLE, 6);
    cold_storage.write_back(chunk, types.layers[0], layer_codec.encode_cells(&one_cell(CELL)), &mut flushed_superchunks);
    cold_storage.write_back(chunk, types.layers[1], layer_codec.encode_cells(&rectangle_and_circle()), &mut flushed_superchunks);
    cold_storage.write_back(chunk, types.layers[0], layer_codec.encode_cells(&rectangle_and_circle()), &mut flushed_superchunks);
    assert!(flushed_superchunks.is_empty() && cold_storage.layer(chunk, types.layers[0]).is_none(), "in the ring, not the cold pool");
    assert!(cold_storage.flush(WORLD_MIDDLE) && !cold_storage.flush(WORLD_MIDDLE));
    assert!(cold_storage.nothing_to_flush());
    assert_eq!(decoded_cells(&mut layer_codec, cold_storage.layer(chunk, types.layers[0]).expect("flushed")), rectangle_and_circle(), "the later write");
    assert_eq!(decoded_cells(&mut layer_codec, cold_storage.layer(chunk, types.layers[1]).expect("flushed")), rectangle_and_circle());

    cold_storage.write_back(chunk, types.layers[1], &[], &mut flushed_superchunks);
    cold_storage.flush_all(&mut flushed_superchunks);
    assert_eq!(flushed_superchunks, [WORLD_MIDDLE]);
    assert!(cold_storage.layer(chunk, types.layers[1]).is_none() && cold_storage.layer(chunk, types.layers[0]).is_some());
}

/// A full ring flushes the superchunk at its tail to make room, and says
/// so; a superchunk the cold pool did not hold is made empty. An entry too big
/// for an empty ring grows it.
#[test]
fn a_full_ring_flushes_its_tail() {
    let types = TestTypes::seeded(crate::test_seed::TestSeedHandle::open().fine_seed());
    let (mut layer_codec, mut cold_storage, mut flushed_superchunks) = (LayerCodec::new(), ChunkStorage::new(8), Vec::new());
    let words = layer_codec.encode_cells(&rectangle_and_circle()).to_vec();
    assert!(words.len() > 8, "bigger than the ring");
    let (first_superchunk, second_superchunk) = (SuperchunkIndex::from_cartesian(5, 5), SuperchunkIndex::from_cartesian(6, 5));
    let first_chunk = ChunkIndex::from_superchunk_place(first_superchunk, 0);
    cold_storage.write_back(first_chunk, types.layers[0], &words, &mut flushed_superchunks);
    assert!(flushed_superchunks.is_empty(), "grown, nothing flushed");
    let second_chunk = ChunkIndex::from_superchunk_place(second_superchunk, 3);
    cold_storage.write_back(second_chunk, types.layers[0], &words, &mut flushed_superchunks);
    assert_eq!(flushed_superchunks, [first_superchunk]);
    assert_eq!(decoded_cells(&mut layer_codec, cold_storage.layer(first_chunk, types.layers[0]).expect("flushed")), rectangle_and_circle());
    assert!(cold_storage.image(first_superchunk).expect("made").layer(3, types.layers[0].type_id()).is_none(), "made empty");
    assert!(cold_storage.layer(second_chunk, types.layers[0]).is_none(), "still in the ring");
}

mod paging;
