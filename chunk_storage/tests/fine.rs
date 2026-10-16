//! The fine tier: each test pins one behaviour, at the edges written by hand and at cases drawn from the run's seed -- instant.
//! The tiers: `docs/testing_protocol.md`, at the repository's root.
//!
//! `cargo test --test fine`

mod chunk_storage {
    //! Chunks as stored: a superchunk's
    //! heights, the layer codec, superchunk images, the writeback ring and
    //! the cold pool it feeds.
    //!
    //! `cargo test`

    use bitmap::{Bitmap, CellWords, WORDS};
    use chunk_storage::{ChunkMaps, ChunkStorage, HeightMap, InvalidImage, LayerChange, LayerCodec, LayerType, SuperchunkImage, WritebackRing};
    use coordinates::{CellCartesian, CellIndex, ChunkIndex, SuperchunkIndex, WORLD_MIDDLE};
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
        let mut seeded_random = RandomSource::new(utilities::seed::counted_seed());
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

    /// Heights drawn, a floor and a span a chunk: every cell's comes
    /// back, from the map and from an image made with it, and a chunk
    /// is tall exactly if its ground spans more than a byte.
    #[test]
    fn heights_drawn_come_back_and_a_chunk_is_tall_by_its_span() {
        const CELLS: usize = 1 << 16;
        let mut seeded_random = RandomSource::new(utilities::seed::counted_seed());
        for _ in 0..4 {
            // Spans about a byte, where a chunk turns tall, and far past it.
            let chunks: [(u16, u16); 16] = std::array::from_fn(|_| {
                let drawn_span = [0, 1, 254, 255, 256, 257, seeded_random.draw_below(256) as u16, seeded_random.draw_below(60_000) as u16][seeded_random.draw_below(8) as usize];
                (seeded_random.draw_below(u64::from(u16::MAX - drawn_span) + 1) as u16, drawn_span)
            });
            let noise_multiplier = seeded_random.next_draw() | 1;
            let height_of = |place: usize| {
                let (floor, chunk_span) = chunks[place / CELLS];
                // The floor and the top each on a cell, the rest anywhere between.
                match place % CELLS {
                    0 => floor,
                    1 => floor + chunk_span,
                    cell => floor + ((cell as u64).wrapping_mul(noise_multiplier) >> 20) as u16 % (chunk_span + 1),
                }
            };
            let heights = HeightMap::from_heights(height_of);
            let image = SuperchunkImage::new(&heights);
            for (chunk, &(floor, expected_span)) in chunks.iter().enumerate() {
                assert_eq!((heights.is_tall(chunk), heights.floor(chunk)), (expected_span > 255, floor), "chunk {chunk}: a span of {expected_span} from {floor}");
            }
            for place in (0..16).flat_map(|chunk| [0, 1, CELLS - 1].map(|cell| chunk * CELLS + cell)).chain((0..2_000).map(|_| seeded_random.draw_below(16 * CELLS as u64) as usize)) {
                assert_eq!((heights.get(place), image.height(place)), (height_of(place), height_of(place)), "place {place}");
            }
        }
    }

    /// Every cell has its own height, in the map and in an image made with
    /// it, found by its place in its superchunk.
    #[test]
    fn every_cell_has_its_own_height() {
        let top_left = WORLD_MIDDLE.top_left().cartesian();
        let cells: Vec<CellIndex> = WORLD_MIDDLE
            .chunks()
            .flat_map(|chunk| [(0, 0), (255, 0), (0, 255), (255, 255), (CELL.0 as u32, CELL.1 as u32), (128, 77)].map(|(x, y)| {
                let chunk_corner = chunk.top_left().cartesian();
                CellIndex::from(CellCartesian { x: chunk_corner.x + x, y: chunk_corner.y + y })
            }))
            .collect();
        let height_of = |cell: CellIndex| {
            let CellCartesian { x, y } = cell.cartesian();
            ((x - top_left.x) * 3 + (y - top_left.y) * 7) as u8 as u16
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

    /// Heights are 16 bits: a chunk whose ground spans a byte or less
    /// keeps a floor and a byte a cell, wherever its floor is; one that
    /// spans more is tall and keeps a map of its own -- and an image holds
    /// both, its chunks after them.
    #[test]
    fn heights_are_a_floor_a_chunk_or_a_map_where_a_chunk_is_tall() {
        const CELLS: usize = 1 << 16;
        // Chunk 3 a slope of 1,000 from a floor of 40,000; chunk 9 one of 60,000; the rest 700 and a byte.
        let height_of = |place: usize| match place / CELLS {
            3 => 40_000 + (place % CELLS * 1_000 / CELLS) as u16,
            9 => (place % CELLS * 60_000 / CELLS) as u16,
            chunk => 700 + chunk as u16 * 300 + (place % 256) as u16,
        };
        let heights = HeightMap::from_heights(height_of);
        assert_eq!((0..16).filter(|&chunk| heights.is_tall(chunk)).collect::<Vec<_>>(), [3, 9]);
        assert_eq!((heights.floor(0), heights.floor(3), heights.floor(15)), (700, 40_000, 700 + 15 * 300));
        assert_eq!(heights.words().len(), chunk_storage::HEIGHT_WORDS + 2 * chunk_storage::TALL_WORDS);
        let layer = [7u64, 8, 9];
        let image = SuperchunkImage::new(&heights).rewritten_with(&[LayerChange { place: 5, layer_type: LayerType(2), encoded_layer: &layer }]);
        let image_read = SuperchunkImage::from_words(image.words().into()).expect("an image");
        for place in (0..16 * CELLS).step_by(997) {
            assert_eq!(heights.get(place), height_of(place), "cell {place}");
            assert_eq!(image_read.height(place), height_of(place), "cell {place}, in the image");
        }
        assert_eq!(&image_read.layer(5, LayerType(2)).expect("the layer")[..3], &layer);
        assert_eq!(HeightMap::from_words(image_read.height_words()), heights);
        // Other heights put on it, none tall: the image shorter, its layer as it was.
        let flat_image = image_read.with_heights(&HeightMap::all_at_height(512));
        assert_eq!((flat_image.height(12_345), flat_image.words().len() + 2 * chunk_storage::TALL_WORDS), (512, image_read.words().len()));
        assert_eq!(&flat_image.layer(5, LayerType(2)).expect("the layer")[..3], &layer);
        // Water on it: none in most chunks, shallow in one, deeper than a byte tells in another -- a map for each of the two, read back, and the rest as it was.
        let depth_of = |place: usize| match place / CELLS {
            3 => (place % 3000) as u16,
            12 => place.is_multiple_of(7) as u16 * 200,
            _ => 0,
        };
        assert_eq!((image_read.water_depth(21), image_read.water_words().len()), (0, 1), "no water yet");
        let wet_image = SuperchunkImage::from_words(image_read.with_water(&ChunkMaps::from_numbers(depth_of)).words().into()).expect("an image with water");
        assert_eq!(wet_image.water_words().len(), 1 + 3 * chunk_storage::MAP_WORDS, "a wide map and a narrow one");
        for place in (0..16 * CELLS).step_by(997) {
            assert_eq!((wet_image.water_depth(place), wet_image.height(place)), (depth_of(place), height_of(place)), "cell {place}");
        }
        assert_eq!(&wet_image.layer(5, LayerType(2)).expect("the layer")[..3], &layer);
        // Changed, it keeps its water; with none anywhere, none is kept.
        assert_eq!(wet_image.rewritten_with(&[]).with_heights(&heights), wet_image);
        assert_eq!(wet_image.with_water(&ChunkMaps::default()), image_read);
    }

    /// An image's layers come out by type, one a type, however they went
    /// in; a later change replaces an earlier one, and a change of no words
    /// removes the layer. The image read back from its words is the same.
    #[test]
    fn images_hold_one_layer_a_type_in_type_order() {
        let mut layer_codec = LayerCodec::new();
        let (drawn_words, one_cell_words) = (layer_codec.encode_cells(&rectangle_and_circle()).to_vec(), layer_codec.encode_cells(&one_cell(CELL)).to_vec());
        let place = 13;
        let layer_change = |layer_type, words| LayerChange { place, layer_type: LayerType(layer_type), encoded_layer: words };
        let image = SuperchunkImage::new(&HeightMap::all_at_height(9)).rewritten_with(&[
            layer_change(42, &one_cell_words),
            layer_change(7, &drawn_words),
            layer_change(u64::MAX, &one_cell_words),
            layer_change(0, &drawn_words),
            layer_change(42, &drawn_words),
            layer_change(0, &[]),
            layer_change(5, &[]),
        ]);
        assert_eq!(image.layer_types(place).collect::<Vec<_>>(), [LayerType(7), LayerType(42), LayerType(u64::MAX)]);
        for (layer_type, cells) in [(7, rectangle_and_circle()), (42, rectangle_and_circle()), (u64::MAX, one_cell(CELL))] {
            assert_eq!(decoded_cells(&mut layer_codec, image.layer(place, LayerType(layer_type)).expect("held")), cells, "type {layer_type}");
        }
        assert!(image.layer(place, LayerType(0)).is_none());
        assert!((0..16).filter(|&other_place| other_place != place).all(|checked_place| image.layer_types(checked_place).count() == 0));
        assert!((0..1 << 20).step_by(4099).all(|cell| image.height(cell) == 9));
        assert_eq!(image.words()[0] as usize, 16 + chunk_storage::HEIGHT_WORDS + 1, "the first chunk after the chunk table, the heights and the word of no water");

        let read_back = SuperchunkImage::from_words(image.words().into()).expect("an image");
        assert_eq!(read_back, image);
        // Rewriting again keeps every bitmap as it was.
        assert_eq!(image.rewritten_with(&[]), image);
    }

    /// Words that are not an image are refused.
    #[test]
    fn broken_images_are_refused() {
        let mut layer_codec = LayerCodec::new();
        let one_cell_words = layer_codec.encode_cells(&one_cell(CELL)).to_vec();
        let image = SuperchunkImage::new(&HeightMap::default()).rewritten_with(&[LayerChange { place: 15, layer_type: LayerType(1), encoded_layer: &one_cell_words }]);
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
        let mut writeback_ring = WritebackRing::new(40);
        let (superchunk_a, superchunk_b) = (SuperchunkIndex::from_cartesian(1, 0), SuperchunkIndex::from_cartesian(0, 1));
        let chunk = ChunkIndex::from_superchunk_place;
        assert!(writeback_ring.push(chunk(superchunk_a, 1), LayerType(1), &[11; 5]));
        assert!(writeback_ring.push(chunk(superchunk_b, 2), LayerType(2), &[22; 5]));
        assert!(writeback_ring.push(chunk(superchunk_a, 3), LayerType(3), &[33; 5]));
        assert!(!writeback_ring.push(chunk(superchunk_b, 4), LayerType(4), &[44; 20]), "no room");
        assert_eq!(writeback_ring.tail_superchunk(), Some(superchunk_a));
        let entries_of_a = writeback_ring.entries_of(superchunk_a);
        assert_eq!(entries_of_a.iter().map(|ring_entry| (ring_entry.place, ring_entry.layer_type, writeback_ring.encoded_layer(ring_entry)[0])).collect::<Vec<_>>(), [
            (1, LayerType(1), 11),
            (3, LayerType(3), 33)
        ]);
        writeback_ring.release_superchunk(superchunk_a);
        assert_eq!(writeback_ring.tail_superchunk(), Some(superchunk_b), "b's entry is the tail now");
        // Words 24 to 37 go to the next entry; the one after it does not fit
        // before the end, and wraps into the 8 words freed before b's.
        assert!(writeback_ring.push(chunk(superchunk_a, 5), LayerType(5), &[55; 10]));
        assert!(writeback_ring.push(chunk(superchunk_a, 6), LayerType(6), &[66; 2]));
        assert!(!writeback_ring.push(chunk(superchunk_a, 7), LayerType(7), &[77; 1]), "full up to b's entry");
        assert_eq!(writeback_ring.entries_of(superchunk_a).iter().map(|later_entry| writeback_ring.encoded_layer(later_entry)[0]).collect::<Vec<_>>(), [55, 66]);
        writeback_ring.release_superchunk(superchunk_b);
        assert_eq!(writeback_ring.tail_superchunk(), Some(superchunk_a));
        writeback_ring.release_superchunk(superchunk_a);
        assert!(writeback_ring.is_empty() && writeback_ring.tail_superchunk().is_none());
        assert!(writeback_ring.push(chunk(superchunk_b, 0), LayerType(9), &[99; 37]), "an empty ring starts over");
    }

    /// Written back, a bitmap is in the ring and not yet in the cold pool; flushed,
    /// its superchunk's image holds it, heights kept, and the ring is empty.
    /// A layer written back empty is removed.
    #[test]
    fn flushing_writes_the_ring_into_the_cold_pool() {
        let (mut layer_codec, mut cold_storage, mut flushed_superchunks) = (LayerCodec::new(), ChunkStorage::new(1 << 12), Vec::new());
        cold_storage.insert(WORLD_MIDDLE, SuperchunkImage::new(&HeightMap::all_at_height(4)));
        let chunk = ChunkIndex::from_superchunk_place(WORLD_MIDDLE, 6);
        cold_storage.write_back(chunk, LayerType(1), layer_codec.encode_cells(&one_cell(CELL)), &mut flushed_superchunks);
        cold_storage.write_back(chunk, LayerType(2), layer_codec.encode_cells(&rectangle_and_circle()), &mut flushed_superchunks);
        cold_storage.write_back(chunk, LayerType(1), layer_codec.encode_cells(&rectangle_and_circle()), &mut flushed_superchunks);
        assert!(flushed_superchunks.is_empty() && cold_storage.layer(chunk, LayerType(1)).is_none(), "in the ring, not the cold pool");
        assert!(cold_storage.flush(WORLD_MIDDLE) && !cold_storage.flush(WORLD_MIDDLE));
        assert!(cold_storage.nothing_to_flush());
        assert_eq!(decoded_cells(&mut layer_codec, cold_storage.layer(chunk, LayerType(1)).expect("flushed")), rectangle_and_circle(), "the later write");
        assert_eq!(decoded_cells(&mut layer_codec, cold_storage.layer(chunk, LayerType(2)).expect("flushed")), rectangle_and_circle());
        assert_eq!(cold_storage.image(WORLD_MIDDLE).expect("held").height(12345), 4);

        cold_storage.write_back(chunk, LayerType(2), &[], &mut flushed_superchunks);
        cold_storage.flush_all(&mut flushed_superchunks);
        assert_eq!(flushed_superchunks, [WORLD_MIDDLE]);
        assert!(cold_storage.layer(chunk, LayerType(2)).is_none() && cold_storage.layer(chunk, LayerType(1)).is_some());
    }

    /// A full ring flushes the superchunk at its tail to make room, and says
    /// so; a superchunk the cold pool did not hold is made flat. An entry too big
    /// for an empty ring grows it.
    #[test]
    fn a_full_ring_flushes_its_tail() {
        let (mut layer_codec, mut cold_storage, mut flushed_superchunks) = (LayerCodec::new(), ChunkStorage::new(8), Vec::new());
        let words = layer_codec.encode_cells(&rectangle_and_circle()).to_vec();
        assert!(words.len() > 8, "bigger than the ring");
        let (first_superchunk, second_superchunk) = (SuperchunkIndex::from_cartesian(5, 5), SuperchunkIndex::from_cartesian(6, 5));
        let first_chunk = ChunkIndex::from_superchunk_place(first_superchunk, 0);
        cold_storage.write_back(first_chunk, LayerType(1), &words, &mut flushed_superchunks);
        assert!(flushed_superchunks.is_empty(), "grown, nothing flushed");
        let second_chunk = ChunkIndex::from_superchunk_place(second_superchunk, 3);
        cold_storage.write_back(second_chunk, LayerType(1), &words, &mut flushed_superchunks);
        assert_eq!(flushed_superchunks, [first_superchunk]);
        assert_eq!(decoded_cells(&mut layer_codec, cold_storage.layer(first_chunk, LayerType(1)).expect("flushed")), rectangle_and_circle());
        assert_eq!(cold_storage.image(first_superchunk).expect("made").height((1 << 20) - 1), 0);
        assert!(cold_storage.layer(second_chunk, LayerType(1)).is_none(), "still in the ring");
    }
}

#[path = "fine/paging.rs"]
mod paging;
