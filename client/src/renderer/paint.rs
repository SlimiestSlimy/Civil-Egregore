//! Cells into pixels, on a thread of its own, between the host, which
//! only copies, and the window, which only shows
//! (`docs/renderer.md`, "Three threads" and "Many superchunks").

use crate::renderer::ground::{lit_colour, GivenHeights, SuperchunkGround, COARSEST_DETAIL};
use crate::renderer::near::{paint_near, PaintedNear};
use coordinates::{cartesian_from_place, CELLS_IN_CHUNK, SUPERCHUNK_SIDE_CELLS};
use protocol::pipe::frame::{cell_of_bit, cloud_over, rains_over, Cells, ViewportFrame, Water, CHUNK_WORDS, CLOUD_TILE_SHIFT, DEEP_WATER_DEPTH, DEEPEST_CLOUD, OLDEST_TREE_STAGE, STAGE_BITS, WORD_BITS};
use server::host::terrain::TerrainAsker;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Instant;
use utilities::tuning::Tuning;

/// Dirt's colour.
pub const DIRT_BROWN: [u8; 3] = [116, 80, 46];
/// Grass's colour.
pub const GRASS_GREEN: [u8; 3] = [72, 160, 56];
/// A sheep's colour.
pub const SHEEP_WHITE: [u8; 3] = [240, 240, 236];

/// Pixels along a superchunk's side: a cell each.
const SUPERCHUNK_SIDE_PIXELS: usize = SUPERCHUNK_SIDE_CELLS as usize;

/// Cells from a sheep's own its square is drawn out to, each way: none,
/// a sheep a pixel, as it is a cell.
const SHEEP_REACH: usize = 0;

/// One superchunk's pixels.
pub struct PaintedSuperchunk {
    /// Where it is in the world, `(x, y)` in superchunks.
    pub superchunk_cartesian: (u32, u32),
    /// Pixels along its side: its cells', halved `detail` times.
    pub side_pixels: u32,
    /// Its pixels, row by row: red, green, blue, opacity.
    pub rgba_bytes: Vec<u8>,
}

/// A frame, painted.
pub struct PaintedFrame {
    /// The frame, its cells gone into the pixels.
    pub host_frame: ViewportFrame,
    /// What painting took of the painter's thread, in seconds.
    pub paint_seconds: f64,
    /// The superchunks asked for, painted: none seen from near.
    pub superchunks: Vec<PaintedSuperchunk>,
    /// The viewport's cells, if they are seen from near.
    pub painted_near: Option<PaintedNear>,
    /// Each of the frame's superchunks' climates, by where it is in
    /// the world: kept of the cells, for the climate written on its
    /// tiles (`climate_labels`).
    pub climates: Vec<((u32, u32), Vec<u64>)>,
}

/// Superchunks whose fine ground is kept, at most: 8 MiB each.
const FINE_KEPT: usize = 48;
/// Superchunks whose ground is kept at all, at most: past that, those
/// longest unseen go.
const GROUNDS_KEPT: usize = 2048;
/// Frames a ground past [`GROUNDS_KEPT`] may go unseen before it goes.
const UNSEEN_FRAMES: u64 = 256;

/// Makes the ground of every superchunk of `host_frame` -- each hot -- that has none
/// yet, or none fine enough -- each on a thread of its own -- and
/// drops the fine parts of those longest unseen beyond [`FINE_KEPT`].
fn make_grounds(kept_grounds: &mut HashMap<(u32, u32), SuperchunkGround>, terrain: &TerrainAsker, host_frame: &ViewportFrame, frame_number: u64) {
    let fine_wanted = host_frame.seen_near.is_some() || host_frame.pixel_side_log2 < 2;
    let hot = || host_frame.cells.iter().map(|cells| cells.top_left);
    let missing_grounds: Vec<(u32, u32)> = hot().filter(|top_left| kept_grounds.get(top_left).is_none_or(|kept_ground| fine_wanted && kept_ground.fine_ground.is_none())).collect();
    let (world, height_levels) = (host_frame.world, host_frame.drawn_heights);
    // The heights the frame brings: those of the superchunks just turned hot.
    let given_heights: GivenHeights = host_frame.cells.iter().filter(|cells| !cells.heights.is_empty()).map(|cells| (cells.top_left, &cells.heights[..])).collect();
    let given_heights_ref = &given_heights;
    let made_grounds: Vec<SuperchunkGround> = thread::scope(|thread_scope| {
        let making_threads: Vec<_> = missing_grounds.iter().map(|&top_left| thread_scope.spawn(move || SuperchunkGround::ask_of_host(terrain, world, height_levels, top_left, given_heights_ref))).collect();
        making_threads.into_iter().map(|making_thread| making_thread.join().expect("a superchunk's ground")).collect()
    });
    kept_grounds.extend(missing_grounds.into_iter().zip(made_grounds));
    for top_left in hot() {
        kept_grounds.get_mut(&top_left).expect("made above").last_used_frame = frame_number;
    }
    let mut fine_by_use: Vec<(u64, (u32, u32))> = kept_grounds.iter().filter(|(_, fine_ground)| fine_ground.fine_ground.is_some()).map(|(&top_left, used_ground)| (used_ground.last_used_frame, top_left)).collect();
    fine_by_use.sort_unstable();
    for &(last_used_frame, top_left) in fine_by_use.iter().take(fine_by_use.len().saturating_sub(FINE_KEPT)) {
        if last_used_frame < frame_number {
            kept_grounds.get_mut(&top_left).expect("listed above").drop_fine_parts();
        }
    }
    if kept_grounds.len() > GROUNDS_KEPT {
        kept_grounds.retain(|_, retained_ground| retained_ground.last_used_frame + UNSEEN_FRAMES >= frame_number);
    }
}

/// Starts the painter's thread: every frame from `frame_receiver` painted, and
/// sent on, shaded from near as the last of `tunings` says -- as the
/// default settings do until one comes -- the heights about a
/// superchunk asked of `terrain`. It stops once either end is
/// dropped.
pub fn start_painter(frame_receiver: Receiver<ViewportFrame>, tunings: Receiver<Tuning>, terrain: TerrainAsker) -> Receiver<PaintedFrame> {
    let (picture_sender, picture_receiver) = channel();
    thread::Builder::new()
        .name("painter".to_string())
        .spawn(move || {
            let (mut kept_grounds, mut world, mut tuning) = (HashMap::new(), 0, utilities::tuning::default_tuning());
            // Each superchunk's water, sent the once: moved into its cells to paint them, and back.
            let mut kept_waters: HashMap<(u32, u32), Water> = HashMap::new();
            for (frame_number, mut host_frame) in frame_receiver.into_iter().enumerate() {
                let paint_started = Instant::now();
                if let Some(latest_tuning) = tunings.try_iter().last() {
                    tuning = latest_tuning;
                }
                if host_frame.world != world {
                    // Another world: its ground is its own.
                    kept_grounds.clear();
                    kept_waters.clear();
                    world = host_frame.world;
                }
                for cells in &mut host_frame.cells {
                    if let Some(sent_water) = cells.water.take() {
                        kept_waters.insert(cells.top_left, sent_water);
                    }
                    cells.water = kept_waters.remove(&cells.top_left);
                }
                make_grounds(&mut kept_grounds, &terrain, &host_frame, frame_number as u64);
                let superchunks = host_frame
                    .cells
                    .iter()
                    .filter_map(|cells| match (host_frame.seen_near, host_frame.pixel_side_log2) {
                        // From near the viewport's cells are one picture.
                        (Some(_), _) => None,
                        (None, 0) => Some(paint_superchunk(cells, &kept_grounds[&cells.top_left])),
                        (None, detail_shift) => Some(paint_far(cells, detail_shift, &kept_grounds[&cells.top_left])),
                    })
                    .collect();
                let painted_near = host_frame.seen_near.map(|near_asked| paint_near(&host_frame.cells, &kept_grounds, near_asked, &tuning));
                let climates = host_frame.cells.iter_mut().map(|cells| (cells.superchunk_cartesian, std::mem::take(&mut cells.climates))).collect();
                for cells in &mut host_frame.cells {
                    kept_waters.extend(cells.water.take().map(|painted_water| (cells.top_left, painted_water)));
                }
                host_frame.cells = Vec::new();
                let painted_picture = PaintedFrame { host_frame, paint_seconds: paint_started.elapsed().as_secs_f64(), superchunks, painted_near, climates };
                if picture_sender.send(painted_picture).is_err() {
                    return;
                }
            }
        })
        .expect("a thread for the painter");
    picture_receiver
}

/// How far across and down from its superchunk's top left the chunk at
/// `place` starts, in cells.
fn chunk_top_left(place: usize) -> (usize, usize) {
    let (x, y) = cartesian_from_place(place * CELLS_IN_CHUNK);
    (x as usize, y as usize)
}

/// Water.
pub const WATER: [u8; 3] = [30, 92, 168];
/// How much of what is under it a film of water hides, beside the
/// [`DEEP`] that hide all of it: water one deep hides this and one
/// more, and so on -- three quarters from the first, so the shallowest
/// water is told from wet ground.
const WATER_FILM_HIDES: usize = 44;

/// How deep the water at bit `bit_in_word` of word `word` of `cells`' bitmaps
/// is, to [`DEEP`] at most.
pub fn depth_at(cells: &Cells, word: usize, bit_in_word: u32) -> u32 {
    let Some(water) = &cells.water else {
        return 0;
    };
    if water.deep_cells[word] >> bit_in_word & 1 == 1 {
        return DEEP_WATER_DEPTH;
    }
    (0..water.depth_low_bits.len()).map(|depth_bit| ((water.depth_low_bits[depth_bit][word] >> bit_in_word & 1) as u32) << depth_bit).sum()
}

/// `seen_colour` under water `water_depth` deep: the less of it seen the deeper,
/// none from [`DEEP`].
pub fn under_water(seen_colour: [u8; 3], water_depth: u32) -> [u8; 3] {
    mixed_colour(seen_colour, WATER, WATER_FILM_HIDES + water_depth as usize, WATER_FILM_HIDES + DEEP_WATER_DEPTH as usize)
}

/// A tree at its first stage, and at its last: darker as it ages.
const TREE_YOUNG: [u8; 3] = [62, 128, 44];
/// A tree at its last stage.
const TREE_OLD: [u8; 3] = [14, 62, 30];

/// A tree's colour at `stage`.
pub fn tree_colour(stage: u32) -> [u8; 3] {
    mixed_colour(TREE_YOUNG, TREE_OLD, stage as usize, OLDEST_TREE_STAGE as usize)
}

/// The stage of the tree at bit `bit_in_word` of word `word` of `cells`' bitmaps.
pub fn stage_at(cells: &Cells, word: usize, bit_in_word: u32) -> u32 {
    // The cell's place among the superchunk's, and its stage's four bits there.
    let stage_bit = (word * WORD_BITS + bit_in_word as usize) * STAGE_BITS;
    (cells.stages[stage_bit / WORD_BITS] >> (stage_bit % WORD_BITS)) as u32 & ((1 << STAGE_BITS) - 1)
}

/// What a cloud's shadow darkens towards: dark, and a little blue.
const CLOUD_SHADOW: [u8; 3] = [18, 22, 40];
/// How much of what is under it the deepest cloud's shadow leaves: a
/// half -- the ground is still read under it.
const SEEN_UNDER_DEEPEST_CLOUD: usize = 2;

/// `seen_colour` in the shadow of `cloud` of `whole_cloud`: none at 0, the
/// deepest at `whole_cloud` -- [`DEEPEST_CLOUD`], or as many times it as
/// the clouds added up.
pub fn under_cloud(seen_colour: [u8; 3], cloud: usize, whole_cloud: usize) -> [u8; 3] {
    mixed_colour(seen_colour, CLOUD_SHADOW, cloud, SEEN_UNDER_DEEPEST_CLOUD * whole_cloud)
}

/// What rain is drawn in: a pale blue.
const RAIN: [u8; 3] = [150, 176, 226];
/// Of this many cells under a raining cloud, one has a streak of rain
/// drawn on it.
pub const CELLS_A_STREAK_OF_RAIN: usize = 5;

/// Whether the cell `(x, y)` -- counted from anywhere, the same for
/// every tile -- has a streak of rain drawn on it under a raining
/// cloud: one in [`CELLS_A_STREAK_OF_RAIN`], in slanted lines.
pub fn streak_of_rain_on(x: usize, y: usize) -> bool {
    (2 * x + y).is_multiple_of(CELLS_A_STREAK_OF_RAIN)
}

/// `seen_colour` with `rain` of `whole_rain` of it rain: a streak, whole, half
/// seen through.
pub fn in_rain(seen_colour: [u8; 3], rain: usize, whole_rain: usize) -> [u8; 3] {
    mixed_colour(seen_colour, RAIN, rain, 2 * whole_rain)
}

/// Every tile of `cells` with a cloud over it: where its top left
/// cell is from the superchunk's, `(x, y)`, how many cells it is a
/// side, its cloud, and whether it is raining, as the frame says
/// (`Cells::raining_tiles`) -- the tiles those of the cloud plane, its tile
/// size the type registry's.
pub fn each_cloud(cells: &Cells, mut on_cloud_tile: impl FnMut((usize, usize), usize, u32, bool)) {
    let (tiles_a_chunk, tile_side_cells) = (CELLS_IN_CHUNK >> CLOUD_TILE_SHIFT, 1usize << (CLOUD_TILE_SHIFT / 2));
    for tile in (0..cells.clouds.len() * WORD_BITS / protocol::pipe::frame::CLOUD_BITS).filter(|&tile| cloud_over(&cells.clouds, tile) != 0) {
        let ((chunk_left_column, chunk_top_row), (x, y)) = (chunk_top_left(tile / tiles_a_chunk), cell_of_bit((tile % tiles_a_chunk) << CLOUD_TILE_SHIFT));
        on_cloud_tile((chunk_left_column + x as usize, chunk_top_row + y as usize), tile_side_cells, cloud_over(&cells.clouds, tile), rains_over(&cells.raining_tiles, tile));
    }
}

/// `rgb_colour`, opaque.
const fn opaque_rgba(rgb_colour: [u8; 3]) -> [u8; 4] {
    [rgb_colour[0], rgb_colour[1], rgb_colour[2], u8::MAX]
}

/// A superchunk's cells as pixels: dirt, its grass over it, both in
/// the `superchunk_ground`'s light, and its sheep over that.
fn paint_superchunk(cells: &Cells, superchunk_ground: &SuperchunkGround) -> PaintedSuperchunk {
    let mut rgba_pixels = vec![opaque_rgba(DIRT_BROWN); SUPERCHUNK_SIDE_PIXELS * SUPERCHUNK_SIDE_PIXELS];
    let grass_pixel = opaque_rgba(GRASS_GREEN);
    for (place, chunk) in cells.grass_bitmaps.as_chunks::<CHUNK_WORDS>().0.iter().enumerate() {
        let (chunk_left_column, chunk_top_row) = chunk_top_left(place);
        for (word_index, &word) in chunk.iter().enumerate() {
            let mut grass_bits = word;
            while grass_bits != 0 {
                let (x, y) = cell_of_bit(word_index * WORD_BITS + grass_bits.trailing_zeros() as usize);
                rgba_pixels[(chunk_top_row + y as usize) * SUPERCHUNK_SIDE_PIXELS + chunk_left_column + x as usize] = grass_pixel;
                grass_bits &= grass_bits - 1;
            }
        }
    }
    // Its trees over the grass, each in its stage's colour.
    for (tree_word_index, &word) in cells.trees.iter().enumerate() {
        let (tree_chunk_left, tree_chunk_top) = chunk_top_left(tree_word_index / CHUNK_WORDS);
        let mut tree_bits = word;
        while tree_bits != 0 {
            let tree_bit = tree_bits.trailing_zeros();
            let (x, y) = cell_of_bit(tree_word_index % CHUNK_WORDS * WORD_BITS + tree_bit as usize);
            rgba_pixels[(tree_chunk_top + y as usize) * SUPERCHUNK_SIDE_PIXELS + tree_chunk_left + x as usize] = opaque_rgba(tree_colour(stage_at(cells, tree_word_index, tree_bit)));
            tree_bits &= tree_bits - 1;
        }
    }
    // Its water over the ground: the deeper, the less of the ground seen.
    for (wet_word_index, &word) in cells.wet_bitmaps.iter().enumerate() {
        let (wet_chunk_left, wet_chunk_top) = chunk_top_left(wet_word_index / CHUNK_WORDS);
        let mut wet_bits = word;
        while wet_bits != 0 {
            let wet_bit = wet_bits.trailing_zeros();
            let (x, y) = cell_of_bit(wet_word_index % CHUNK_WORDS * WORD_BITS + wet_bit as usize);
            let wet_pixel = &mut rgba_pixels[(wet_chunk_top + y as usize) * SUPERCHUNK_SIDE_PIXELS + wet_chunk_left + x as usize];
            *wet_pixel = opaque_rgba(under_water([wet_pixel[0], wet_pixel[1], wet_pixel[2]], depth_at(cells, wet_word_index, wet_bit)));
            wet_bits &= wet_bits - 1;
        }
    }
    // All of it in the ground's light: the land's its slope's and its height's, the water's its depth's.
    for (lit_pixel, &light_shade) in rgba_pixels.iter_mut().zip(&superchunk_ground.shades_by_detail[0]) {
        *lit_pixel = opaque_rgba(lit_colour([lit_pixel[0], lit_pixel[1], lit_pixel[2]], light_shade));
    }
    let sheep_pixel = opaque_rgba(SHEEP_WHITE);
    for &(x, y) in &cells.sheep_cells_cartesian {
        let (x, y) = (x as usize, y as usize);
        for y in y.saturating_sub(SHEEP_REACH)..=(y + SHEEP_REACH).min(SUPERCHUNK_SIDE_PIXELS - 1) {
            for x in x.saturating_sub(SHEEP_REACH)..=(x + SHEEP_REACH).min(SUPERCHUNK_SIDE_PIXELS - 1) {
                rgba_pixels[y * SUPERCHUNK_SIDE_PIXELS + x] = sheep_pixel;
            }
        }
    }
    // The clouds' shadows over all of it, the sheep too.
    each_cloud(cells, |(cloud_left, cloud_top), cloud_side, cloud, is_raining| {
        for cloud_pixel in (cloud_top..cloud_top + cloud_side).flat_map(|y| (cloud_left..cloud_left + cloud_side).map(move |x| y * SUPERCHUNK_SIDE_PIXELS + x)) {
            let cloud_shaded = under_cloud([rgba_pixels[cloud_pixel][0], rgba_pixels[cloud_pixel][1], rgba_pixels[cloud_pixel][2]], cloud as usize, DEEPEST_CLOUD as usize);
            // Rain under a cloud deep enough: a streak on some of its cells.
            rgba_pixels[cloud_pixel] = opaque_rgba(if is_raining && streak_of_rain_on(cloud_pixel % SUPERCHUNK_SIDE_PIXELS, cloud_pixel / SUPERCHUNK_SIDE_PIXELS) { in_rain(cloud_shaded, 1, 1) } else { cloud_shaded });
        }
    });
    PaintedSuperchunk { superchunk_cartesian: cells.superchunk_cartesian, side_pixels: SUPERCHUNK_SIDE_PIXELS as u32, rgba_bytes: rgba_pixels.into_flattened() }
}

/// `from_colour` and `to_colour` mixed, `part` of `whole_parts` of it `to_colour`.
fn mixed_colour(from_colour: [u8; 3], to_colour: [u8; 3], part: usize, whole_parts: usize) -> [u8; 3] {
    let part = part.min(whole_parts);
    std::array::from_fn(|colour_channel| ((from_colour[colour_channel] as usize * (whole_parts - part) + to_colour[colour_channel] as usize * part) / whole_parts) as u8)
}

/// The cells set in each tile of cells `2^detail_shift` a side of a
/// superchunk's `words` -- its chunks' bitmaps one after another --
/// row by row: each tile a run of bits in Morton order, counted from
/// the words with no cell looked at.
fn counted_in_tiles(words: &[u64], detail_shift: u32) -> Vec<u32> {
    let (side_tiles, tile_cells) = (SUPERCHUNK_SIDE_PIXELS >> detail_shift, 1usize << (2 * detail_shift));
    let mut counts = vec![0u32; side_tiles * side_tiles];
    for (place, chunk) in words.as_chunks::<CHUNK_WORDS>().0.iter().enumerate() {
        let (chunk_left_column, chunk_top_row) = chunk_top_left(place);
        let (tile_left_column, tile_top_row) = (chunk_left_column >> detail_shift, chunk_top_row >> detail_shift);
        for tile in 0..CELLS_IN_CHUNK / tile_cells {
            let count: u32 = if tile_cells >= WORD_BITS {
                let words = tile_cells / WORD_BITS;
                chunk[tile * words..][..words].iter().map(|word| word.count_ones()).sum()
            } else {
                let first_bit = tile * tile_cells;
                (chunk[first_bit / WORD_BITS] >> (first_bit % WORD_BITS) & ((1 << tile_cells) - 1)).count_ones()
            };
            let (x, y) = cell_of_bit(tile);
            counts[(tile_top_row + y as usize) * side_tiles + tile_left_column + x as usize] = count;
        }
    }
    counts
}

/// A superchunk's cells as pixels from far off, a pixel a tile of
/// cells `2^detail_shift` a side: dirt and grass mixed by the grass in the
/// tile, its trees' colour mixed in by the trees in it, and white mixed in by the
/// sheep on it, each as many cells as it is drawn from near; the
/// ground in the `superchunk_ground`'s light.
fn paint_far(cells: &Cells, detail_shift: u32, superchunk_ground: &SuperchunkGround) -> PaintedSuperchunk {
    let (side_pixels, tile_cells) = (SUPERCHUNK_SIDE_PIXELS >> detail_shift, 1usize << (2 * detail_shift));
    let (grass_counts, trees) = (counted_in_tiles(&cells.grass_bitmaps, detail_shift), counted_in_tiles(&cells.trees, detail_shift));
    let (wet_counts, deep_counts) = (counted_in_tiles(&cells.wet_bitmaps, detail_shift), counted_in_tiles(cells.water.as_ref().map_or(&[][..], |water| &water.deep_cells), detail_shift));
    let mut sheep_counts = vec![0u16; side_pixels * side_pixels];
    for &(x, y) in &cells.sheep_cells_cartesian {
        let sheep_pixel = (y as usize >> detail_shift) * side_pixels + (x as usize >> detail_shift);
        sheep_counts[sheep_pixel] = sheep_counts[sheep_pixel].saturating_add(1);
    }
    // The cloud over each pixel's cells, added up: a tile's cloud once for every cell of it under the pixel.
    let mut clouds = vec![0usize; if cells.clouds.is_empty() { 0 } else { side_pixels * side_pixels }];
    // And the cells of each pixel that are under a raining cloud.
    let mut rain = clouds.clone();
    each_cloud(cells, |(cloud_left, cloud_top), tile_side, cloud, is_raining| {
        let (pixels_a_side, cells_each) = ((tile_side >> detail_shift).max(1), tile_side.min(1 << detail_shift));
        for cloud_pixel in ((cloud_top >> detail_shift)..(cloud_top >> detail_shift) + pixels_a_side).flat_map(|y| ((cloud_left >> detail_shift)..(cloud_left >> detail_shift) + pixels_a_side).map(move |x| y * side_pixels + x)) {
            clouds[cloud_pixel] += cloud as usize * cells_each * cells_each;
            if is_raining {
                rain[cloud_pixel] += cells_each * cells_each;
            }
        }
    });
    let sheep_cells = (2 * SHEEP_REACH + 1) * (2 * SHEEP_REACH + 1);
    let mut rgba_bytes = Vec::with_capacity(side_pixels * side_pixels * 4);
    for (pixel_number, &light_shade) in superchunk_ground.shades_by_detail[(detail_shift as usize).min(COARSEST_DETAIL)].iter().enumerate() {
        let (grass_cells, trees, sheep_on_pixel) = (grass_counts[pixel_number], trees[pixel_number], sheep_counts[pixel_number]);
        let dry_colour = mixed_colour(mixed_colour(DIRT_BROWN, GRASS_GREEN, grass_cells as usize, tile_cells), tree_colour(OLDEST_TREE_STAGE / 2), trees as usize, tile_cells);
        // The water over the ground, by the share of the tile under it, shallow water a quarter seen through; and all of it in the ground's light.
        let wet_lit_colour = lit_colour(mixed_colour(dry_colour, WATER, (3 * wet_counts[pixel_number] as usize + deep_counts[pixel_number] as usize) / 4, tile_cells), light_shade);
        let sheep_mixed_colour = mixed_colour(wet_lit_colour, SHEEP_WHITE, sheep_on_pixel as usize * sheep_cells, tile_cells);
        // Under the clouds' shadow, and the rain's streaks as thin as they are among the cells.
        rgba_bytes.extend_from_slice(&opaque_rgba(clouds.get(pixel_number).map_or(sheep_mixed_colour, |&cloud| in_rain(under_cloud(sheep_mixed_colour, cloud, DEEPEST_CLOUD as usize * tile_cells), rain[pixel_number], CELLS_A_STREAK_OF_RAIN * tile_cells))));
    }
    PaintedSuperchunk { superchunk_cartesian: cells.superchunk_cartesian, side_pixels: side_pixels as u32, rgba_bytes }
}
