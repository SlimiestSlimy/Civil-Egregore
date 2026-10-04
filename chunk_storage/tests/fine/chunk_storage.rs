//! Chunks as stored: a superchunk's
//! heights, the layer codec, superchunk images, the writeback ring and
//! the cold pool it feeds.
//!
//! `cargo test`

use bitmap::{Bitmap, CellWords, WORDS};
use chunk_storage::{ChunkStorage, HeightMap, InvalidImage, LayerChange, LayerCodec, LayerType, SuperchunkImage, WritebackRing};
use coordinates::{CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, WORLD_MIDDLE};

/// A cell of a chunk, cartesian: across and down from its top left.
const CELL: (u8, u8) = (3, 200);

/// A bitmap's cells, with a rectangle and a circle drawn.
fn drawn() -> CellWords {
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
fn decoded(codec: &mut LayerCodec, words: &[u64]) -> CellWords {
    let mut cells = [u64::MAX; WORDS];
    codec.decode(words, &mut cells);
    cells
}

/// A layer comes back from its encoding cell for cell, whatever words
/// follow it, and an empty one takes a word, not a bitmap's worth.
#[test]
fn layers_decode_to_what_was_encoded_whatever_follows() {
    let mut codec = LayerCodec::new();
    let cells = drawn();
    let encoded = codec.encode(&cells).to_vec();
    let followed: Vec<u64> = [&encoded[..], &[u64::MAX; 4], codec.encode(&one_cell(CELL))].concat();
    assert_eq!(decoded(&mut codec, &followed), cells);
    assert_eq!(codec.encode(&[0; WORDS]).len(), 1);
}

/// Every cell has its own height, in the map and in an image made with
/// it, found by its place in its superchunk.
#[test]
fn every_cell_has_its_own_height() {
    let top_left = WORLD_MIDDLE.top_left().cartesian();
    let cells: Vec<CellIndex> = WORLD_MIDDLE
        .chunks()
        .flat_map(|chunk| [(0, 0), (255, 0), (0, 255), (255, 255), (CELL.0 as u32, CELL.1 as u32), (128, 77)].map(|(x, y)| {
            let corner = chunk.top_left().cartesian();
            CellIndex::from(CellCartesian { x: corner.x + x, y: corner.y + y })
        }))
        .collect();
    let height_of = |cell: CellIndex| {
        let CellCartesian { x, y } = cell.cartesian();
        ((x - top_left.x) * 3 + (y - top_left.y) * 7) as u8
    };
    let mut heights = HeightMap::default();
    for &cell in &cells {
        heights.set(cell.place_in_superchunk(), height_of(cell));
    }
    let image = SuperchunkImage::new(&heights);
    for &cell in &cells {
        assert_eq!(heights.get(cell.place_in_superchunk()), height_of(cell), "{:?}", cell.cartesian());
        assert_eq!(image.height(cell.place_in_superchunk()), height_of(cell), "{:?}", cell.cartesian());
    }
    assert_eq!(heights.get(1), 0, "a cell never set");
}

/// An image's layers come out by type, one a type, however they went
/// in; a later change replaces an earlier one, and a change of no words
/// removes the layer. The image read back from its words is the same.
#[test]
fn images_hold_one_layer_a_type_in_type_order() {
    let mut codec = LayerCodec::new();
    let (drawn_words, one) = (codec.encode(&drawn()).to_vec(), codec.encode(&one_cell(CELL)).to_vec());
    let place = 13;
    let change = |layer_type, words| LayerChange { place, layer_type: LayerType(layer_type), encoded: words };
    let image = SuperchunkImage::new(&HeightMap::filled(9)).rewritten(&[
        change(42, &one),
        change(7, &drawn_words),
        change(u64::MAX, &one),
        change(0, &drawn_words),
        change(42, &drawn_words),
        change(0, &[]),
        change(5, &[]),
    ]);
    assert_eq!(image.layer_types(place).collect::<Vec<_>>(), [LayerType(7), LayerType(42), LayerType(u64::MAX)]);
    for (layer_type, cells) in [(7, drawn()), (42, drawn()), (u64::MAX, one_cell(CELL))] {
        assert_eq!(decoded(&mut codec, image.layer(place, LayerType(layer_type)).expect("held")), cells, "type {layer_type}");
    }
    assert!(image.layer(place, LayerType(0)).is_none());
    assert!((0..16).filter(|&other| other != place).all(|other| image.layer_types(other).count() == 0));
    assert!((0..1 << 20).step_by(4099).all(|cell| image.height(cell) == 9));
    assert_eq!(image.words()[0] as usize, 16 + chunk_storage::HEIGHT_WORDS, "the first chunk after the chunk table and heights");

    let read_back = SuperchunkImage::from_words(image.words().into()).expect("an image");
    assert_eq!(read_back, image);
    // Rewriting again keeps every bitmap as it was.
    assert_eq!(image.rewritten(&[]), image);
}

/// Words that are not an image are refused.
#[test]
fn broken_images_are_refused() {
    let mut codec = LayerCodec::new();
    let one = codec.encode(&one_cell(CELL)).to_vec();
    let image = SuperchunkImage::new(&HeightMap::default()).rewritten(&[LayerChange { place: 15, layer_type: LayerType(1), encoded: &one }]);
    let words = image.words();
    assert!(SuperchunkImage::from_words(words[..100].into()).is_err());
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
    let mut ring = WritebackRing::new(40);
    let (a, b) = (SuperchunkIndex::from_cartesian(1, 0), SuperchunkIndex::from_cartesian(0, 1));
    let chunk = ChunkIndex::of;
    assert!(ring.push(chunk(a, 1), LayerType(1), &[11; 5]));
    assert!(ring.push(chunk(b, 2), LayerType(2), &[22; 5]));
    assert!(ring.push(chunk(a, 3), LayerType(3), &[33; 5]));
    assert!(!ring.push(chunk(b, 4), LayerType(4), &[44; 20]), "no room");
    assert_eq!(ring.tail_superchunk(), Some(a));
    let entries = ring.entries_of(a);
    assert_eq!(entries.iter().map(|entry| (entry.place, entry.layer_type, ring.encoded(entry)[0])).collect::<Vec<_>>(), [
        (1, LayerType(1), 11),
        (3, LayerType(3), 33)
    ]);
    ring.release(a);
    assert_eq!(ring.tail_superchunk(), Some(b), "b's entry is the tail now");
    // Words 24 to 37 go to the next entry; the one after it does not fit
    // before the end, and wraps into the 8 words freed before b's.
    assert!(ring.push(chunk(a, 5), LayerType(5), &[55; 10]));
    assert!(ring.push(chunk(a, 6), LayerType(6), &[66; 2]));
    assert!(!ring.push(chunk(a, 7), LayerType(7), &[77; 1]), "full up to b's entry");
    assert_eq!(ring.entries_of(a).iter().map(|entry| ring.encoded(entry)[0]).collect::<Vec<_>>(), [55, 66]);
    ring.release(b);
    assert_eq!(ring.tail_superchunk(), Some(a));
    ring.release(a);
    assert!(ring.is_empty() && ring.tail_superchunk().is_none());
    assert!(ring.push(chunk(b, 0), LayerType(9), &[99; 37]), "an empty ring starts over");
}

/// Written back, a bitmap is in the ring and not yet in the cold pool; flushed,
/// its superchunk's image holds it, heights kept, and the ring is empty.
/// A layer written back empty is removed.
#[test]
fn flushing_writes_the_ring_into_the_cold_pool() {
    let (mut codec, mut storage, mut flushed) = (LayerCodec::new(), ChunkStorage::new(1 << 12), Vec::new());
    storage.insert(WORLD_MIDDLE, SuperchunkImage::new(&HeightMap::filled(4)));
    let chunk = ChunkIndex::of(WORLD_MIDDLE, 6);
    storage.write_back(chunk, LayerType(1), codec.encode(&one_cell(CELL)), &mut flushed);
    storage.write_back(chunk, LayerType(2), codec.encode(&drawn()), &mut flushed);
    storage.write_back(chunk, LayerType(1), codec.encode(&drawn()), &mut flushed);
    assert!(flushed.is_empty() && storage.layer(chunk, LayerType(1)).is_none(), "in the ring, not the cold pool");
    assert!(storage.flush(WORLD_MIDDLE) && !storage.flush(WORLD_MIDDLE));
    assert!(storage.nothing_to_flush());
    assert_eq!(decoded(&mut codec, storage.layer(chunk, LayerType(1)).expect("flushed")), drawn(), "the later write");
    assert_eq!(decoded(&mut codec, storage.layer(chunk, LayerType(2)).expect("flushed")), drawn());
    assert_eq!(storage.image(WORLD_MIDDLE).expect("held").height(12345), 4);

    storage.write_back(chunk, LayerType(2), &[], &mut flushed);
    storage.flush_all(&mut flushed);
    assert_eq!(flushed, [WORLD_MIDDLE]);
    assert!(storage.layer(chunk, LayerType(2)).is_none() && storage.layer(chunk, LayerType(1)).is_some());
}

/// A full ring flushes the superchunk at its tail to make room, and says
/// so; a superchunk the cold pool did not hold is made flat. An entry too big
/// for an empty ring grows it.
#[test]
fn a_full_ring_flushes_its_tail() {
    let (mut codec, mut storage, mut flushed) = (LayerCodec::new(), ChunkStorage::new(8), Vec::new());
    let words = codec.encode(&drawn()).to_vec();
    assert!(words.len() > 8, "bigger than the ring");
    let (one, other) = (SuperchunkIndex::from_cartesian(5, 5), SuperchunkIndex::from_cartesian(6, 5));
    let first = ChunkIndex::of(one, 0);
    storage.write_back(first, LayerType(1), &words, &mut flushed);
    assert!(flushed.is_empty(), "grown, nothing flushed");
    let second = ChunkIndex::of(other, 3);
    storage.write_back(second, LayerType(1), &words, &mut flushed);
    assert_eq!(flushed, [one]);
    assert_eq!(decoded(&mut codec, storage.layer(first, LayerType(1)).expect("flushed")), drawn());
    assert_eq!(storage.image(one).expect("made").height((1 << 20) - 1), 0);
    assert!(storage.layer(second, LayerType(1)).is_none(), "still in the ring");
}






