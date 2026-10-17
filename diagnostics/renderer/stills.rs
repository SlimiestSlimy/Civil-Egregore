//! Stills: one place of a world at every zoom, painted by the window's
//! own threads -- the map's, the painter's -- with no window: a host
//! asked as the window asks it, the pictures that come laid together
//! as the window lays them, each scaled to what a screen would show.

use crate::frames::detail_at;
use crate::map::{draw_map, WantedMap};
use crate::paint::{self, PaintedFrame};
use coordinates::{SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};
use server::host::frame::{FrameAsk, NearRectangle, Viewport};
use server::host::Host;
use server::WorldStart;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use utilities::tuning::{default_tuning, CAMERA_LOADS, SHEEP_PER_SUPERCHUNK, WORLD_SIDE};

/// Pixels across and down a still: a screen's.
const STILL_SIZE_PIXELS: (u32, u32) = (1024, 768);
/// Cells a pixel of the map's stills.
const MAP_STEPS: [u32; 2] = [64, 16];
/// Pixels a cell of the stills from near.
const NEAR_PIXELS: [u32; 3] = [2, 4, 8];
/// Cells past a near still's edges that its superchunks are asked for.
const NEAR_MARGIN: u32 = 8;
/// Sheep a superchunk of the stills' world.
const SHEEP_A_SUPERCHUNK: f32 = 250.0;
/// How long a picture is waited for: a world's superchunks take a while to generate.
const WAITED_AT_MOST: Duration = Duration::from_secs(1800);
/// How long is waited before the host is asked again for superchunks not yet hot.
const BETWEEN_ASKS: Duration = Duration::from_millis(200);

/// One still.
pub struct StillPicture {
    /// What it is of: the zoom.
    pub zoom_name: String,
    /// Pixels across and down.
    pub size_pixels: (u32, u32),
    /// Its pixels, row by row: red, green, blue, opacity.
    pub rgba_bytes: Vec<u8>,
}

/// The pictures of what `host` answers `frame_ask` with, once every one of
/// the `expected_hot` superchunks of its viewport is hot.
fn pictures_answered(host: &Host, picture_receiver: &Receiver<PaintedFrame>, frame_ask: FrameAsk, expected_hot: u32) -> Result<Vec<PaintedFrame>, String> {
    loop {
        if !host.ask_frame(frame_ask) {
            return Err("the host is gone".to_string());
        }
        let mut answer_pictures = Vec::new();
        loop {
            let painted_picture = picture_receiver.recv_timeout(WAITED_AT_MOST).map_err(|_| "the host never answered".to_string())?;
            let more_coming = painted_picture.host_frame.more_to_come;
            answer_pictures.push(painted_picture);
            if !more_coming {
                break;
            }
        }
        if answer_pictures.last().is_some_and(|last_picture| last_picture.host_frame.hot.len() as u32 >= expected_hot) {
            return Ok(answer_pictures);
        }
        std::thread::sleep(BETWEEN_ASKS);
    }
}

/// The superchunks holding the cells from `first_cell_cartesian` to `last_cell_cartesian`, and how many they are.
fn viewport_of(first_cell_cartesian: (u32, u32), last_cell_cartesian: (u32, u32)) -> (Viewport, u32) {
    let superchunk = |cell: u32| cell / SUPERCHUNK_SIDE_CELLS;
    let viewport = Viewport { top_left_superchunk_cartesian: (superchunk(first_cell_cartesian.0), superchunk(first_cell_cartesian.1)), bottom_right_superchunk_cartesian: (superchunk(last_cell_cartesian.0), superchunk(last_cell_cartesian.1)) };
    (viewport, (viewport.bottom_right_superchunk_cartesian.0 - viewport.top_left_superchunk_cartesian.0 + 1) * (viewport.bottom_right_superchunk_cartesian.1 - viewport.top_left_superchunk_cartesian.1 + 1))
}

/// Gathers the stills of the world of `seed` -- none, one drawn fresh
/// and moved on to one with land about the world's middle -- centred
/// `cells_east_south` cells east and south of its middle, each handed to `keep_still`
/// as it is made: the map, the cells from `farthest_cells_a_pixel` cells a pixel to
/// one, and the cells from near. Says the seed they are of.
pub fn gather_stills(seed: Option<u64>, cells_east_south: (i64, i64), farthest_cells_a_pixel: u32, mut keep_still: impl FnMut(StillPicture) -> Result<(), String>) -> Result<u64, String> {
    let mut tuning = default_tuning();
    (tuning[WORLD_SIDE], tuning[CAMERA_LOADS], tuning[SHEEP_PER_SUPERCHUNK]) = (0.0, 1.0, SHEEP_A_SUPERCHUNK);
    let world_start = WorldStart::from_tuning(seed, &tuning);
    let seed = seed.unwrap_or_else(|| server::seed_with_land(utilities::seed::given_or_fresh_seed(0), &world_start.world_generation));
    let seeded_start = WorldStart { seed, ..world_start };
    let origin_top_left = WORLD_MIDDLE.top_left().cartesian();
    let still_centre = ((origin_top_left.x + SUPERCHUNK_SIDE_CELLS / 2) as i64 + cells_east_south.0, (origin_top_left.y + SUPERCHUNK_SIDE_CELLS / 2) as i64 + cells_east_south.1);
    // The top left cell of a still `scale` cells a pixel, a pixel `2^detail_shift` cells: on a pixel's edge.
    let first_at = |cells: (u32, u32), tile: u32| (((still_centre.0 - cells.0 as i64 / 2) as u32) / tile * tile, ((still_centre.1 - cells.1 as i64 / 2) as u32) / tile * tile);

    let (host, frame_receiver) = Host::start_on_thread();
    let (_tunings, tunings_read) = channel();
    let picture_receiver = paint::start_painter(frame_receiver, tunings_read, host.terrain());
    host.set_pace(None);
    host.make_world(seeded_start);

    // The map is asked of the world run: the number the host gave it is in any frame of it.
    let world = pictures_answered(&host, &picture_receiver, FrameAsk { viewport: None, pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: 0, seen_near: None }, 0)?.last().map_or(0, |world_picture| world_picture.host_frame.world);
    for step in MAP_STEPS {
        let map_first_cell = (still_centre.0 - (STILL_SIZE_PIXELS.0 * step) as i64 / 2, still_centre.1 - (STILL_SIZE_PIXELS.1 * step) as i64 / 2);
        let map_bytes = draw_map(&host.terrain(), &WantedMap { first_cell_cartesian: map_first_cell, step, size_pixels: STILL_SIZE_PIXELS, world, mesh_lines_drawn: false }).ok_or("the host runs another world")?;
        keep_still(StillPicture { zoom_name: format!("0_map_{step:02}_cells_a_pixel"), size_pixels: STILL_SIZE_PIXELS, rgba_bytes: map_bytes })?;
    }

    // From far, and a cell a pixel: a picture a superchunk, laid side by side.
    let mut scale = farthest_cells_a_pixel.max(1).next_power_of_two();
    loop {
        let detail_shift = detail_at(scale as f32);
        let (tile, cells) = (1 << detail_shift, (STILL_SIZE_PIXELS.0 * scale, STILL_SIZE_PIXELS.1 * scale));
        let far_first_cell = first_at(cells, tile);
        let (viewport, far_expected) = viewport_of(far_first_cell, (far_first_cell.0 + cells.0 - 1, far_first_cell.1 + cells.1 - 1));
        let far_answer = pictures_answered(&host, &picture_receiver, FrameAsk { viewport: Some(viewport), pixel_side_log2: detail_shift, superchunks_skipped: 0, most_superchunks: u32::MAX, seen_near: None }, far_expected)?;
        // What a screen shows of it: each painted pixel `tile / scale` screen pixels a side.
        let (screen_pixels_a_pixel, mut far_bytes) = (tile / scale, vec![0u8; (STILL_SIZE_PIXELS.0 * STILL_SIZE_PIXELS.1 * 4) as usize]);
        for painted_superchunk in far_answer.iter().flat_map(|far_picture| &far_picture.superchunks) {
            let side_pixels = painted_superchunk.side_pixels as i64;
            let left_top = |superchunk_coordinate: u32, first_cell_coordinate: u32| (superchunk_coordinate * SUPERCHUNK_SIDE_CELLS) as i64 / tile as i64 - (first_cell_coordinate / tile) as i64;
            let (left_pixel, top_pixel) = (left_top(painted_superchunk.superchunk_cartesian.0, far_first_cell.0), left_top(painted_superchunk.superchunk_cartesian.1, far_first_cell.1));
            for y in top_pixel.max(0)..(top_pixel + side_pixels).min((STILL_SIZE_PIXELS.1 / screen_pixels_a_pixel) as i64) {
                for x in left_pixel.max(0)..(left_pixel + side_pixels).min((STILL_SIZE_PIXELS.0 / screen_pixels_a_pixel) as i64) {
                    let from_byte = (((y - top_pixel) * side_pixels + x - left_pixel) * 4) as usize;
                    for (grown_down, grown_across) in (0..screen_pixels_a_pixel).flat_map(|down_in_pixel| (0..screen_pixels_a_pixel).map(move |across_in_pixel| (down_in_pixel, across_in_pixel))) {
                        let to_byte = (((y as u32 * screen_pixels_a_pixel + grown_down) * STILL_SIZE_PIXELS.0 + x as u32 * screen_pixels_a_pixel + grown_across) * 4) as usize;
                        far_bytes[to_byte..to_byte + 4].copy_from_slice(&painted_superchunk.rgba_bytes[from_byte..from_byte + 4]);
                    }
                }
            }
        }
        keep_still(StillPicture { zoom_name: format!("1_cells_{scale:02}_cells_a_pixel"), size_pixels: STILL_SIZE_PIXELS, rgba_bytes: far_bytes })?;
        if scale == 1 {
            break;
        }
        scale /= 2;
    }

    // From near: the one picture of the viewport's cells.
    for pixels_a_cell in NEAR_PIXELS {
        let near_size_cells = (STILL_SIZE_PIXELS.0 / pixels_a_cell, STILL_SIZE_PIXELS.1 / pixels_a_cell);
        let near_first_cell = first_at(near_size_cells, 1);
        let (viewport, near_expected) = viewport_of((near_first_cell.0 - NEAR_MARGIN, near_first_cell.1 - NEAR_MARGIN), (near_first_cell.0 + near_size_cells.0 + NEAR_MARGIN, near_first_cell.1 + near_size_cells.1 + NEAR_MARGIN));
        let near_asked = NearRectangle { first_cell_cartesian: near_first_cell, size_in_cells: near_size_cells, pixels_a_cell };
        let near_answer = pictures_answered(&host, &picture_receiver, FrameAsk { viewport: Some(viewport), pixel_side_log2: 0, superchunks_skipped: 0, most_superchunks: u32::MAX, seen_near: Some(near_asked) }, near_expected)?;
        let painted_near = near_answer.into_iter().find_map(|near_picture| near_picture.painted_near).ok_or("no picture from near came")?;
        keep_still(StillPicture { zoom_name: format!("2_near_{pixels_a_cell}_pixels_a_cell"), size_pixels: STILL_SIZE_PIXELS, rgba_bytes: painted_near.rgba_bytes })?;
    }
    Ok(seed)
}
