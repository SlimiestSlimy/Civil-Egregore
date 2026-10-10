//! Stills: one place of a world at every zoom, painted by the window's
//! own threads -- the map's, the painter's -- with no window: a host
//! asked as the window asks it, the pictures that come laid together
//! as the window lays them, each scaled to what a screen would show.

use crate::frames::detail_at;
use crate::map::{draw, Wanted};
use crate::paint::{self, Picture};
use coordinates::{SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};
use server::host::frame::{Ask, Near, Viewport};
use server::host::terrain_seen::seed_with_land;
use server::host::Host;
use server::Start;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use utilities::tuning::{defaults, CAMERA_LOADS, SHEEP, WORLD_SIDE};

/// Pixels across and down a still: a screen's.
const SIZE: (u32, u32) = (1024, 768);
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
pub struct Still {
    /// What it is of: the zoom.
    pub name: String,
    /// Pixels across and down.
    pub size: (u32, u32),
    /// Its pixels, row by row: red, green, blue, opacity.
    pub pixels: Vec<u8>,
}

/// The pictures of what `host` answers `ask` with, once every one of
/// the `expected` superchunks of its viewport is hot.
fn answered(host: &Host, pictures: &Receiver<Picture>, ask: Ask, expected: u32) -> Result<Vec<Picture>, String> {
    loop {
        if !host.sync(ask) {
            return Err("the host is gone".to_string());
        }
        let mut answer = Vec::new();
        loop {
            let picture = pictures.recv_timeout(WAITED_AT_MOST).map_err(|_| "the host never answered".to_string())?;
            let more = picture.frame.more;
            answer.push(picture);
            if !more {
                break;
            }
        }
        if answer.last().is_some_and(|last| last.frame.hot.len() as u32 >= expected) {
            return Ok(answer);
        }
        std::thread::sleep(BETWEEN_ASKS);
    }
}

/// The superchunks holding the cells from `first` to `last`, and how many they are.
fn viewport_of(first: (u32, u32), last: (u32, u32)) -> (Viewport, u32) {
    let superchunk = |cell: u32| cell / SUPERCHUNK_SIDE_CELLS;
    let viewport = Viewport { first: (superchunk(first.0), superchunk(first.1)), last: (superchunk(last.0), superchunk(last.1)) };
    (viewport, (viewport.last.0 - viewport.first.0 + 1) * (viewport.last.1 - viewport.first.1 + 1))
}

/// Gathers the stills of the world of `seed` -- none, the counted one
/// moved on to one with land about the world's middle -- centred
/// `offset` cells east and south of its middle, each handed to `keep`
/// as it is made: the map, the cells from `farthest` cells a pixel to
/// one, and the cells from near. Says the seed they are of.
pub fn gather(seed: Option<u64>, offset: (i64, i64), farthest: u32, mut keep: impl FnMut(Still) -> Result<(), String>) -> Result<u64, String> {
    let mut tuning = defaults();
    (tuning[WORLD_SIDE], tuning[CAMERA_LOADS], tuning[SHEEP]) = (0.0, 1.0, SHEEP_A_SUPERCHUNK);
    let start = Start::from_tuning(seed, &tuning);
    let seed = seed.unwrap_or_else(|| seed_with_land(utilities::seed::counted(), &start.generation, WORLD_MIDDLE));
    let start = Start { seed, ..start };
    let middle = WORLD_MIDDLE.top_left().cartesian();
    let centre = ((middle.x + SUPERCHUNK_SIDE_CELLS / 2) as i64 + offset.0, (middle.y + SUPERCHUNK_SIDE_CELLS / 2) as i64 + offset.1);
    // The top left cell of a still `scale` cells a pixel, a pixel `2^detail` cells: on a pixel's edge.
    let first_at = |cells: (u32, u32), tile: u32| (((centre.0 - cells.0 as i64 / 2) as u32) / tile * tile, ((centre.1 - cells.1 as i64 / 2) as u32) / tile * tile);

    for step in MAP_STEPS {
        let first = (centre.0 - (SIZE.0 * step) as i64 / 2, centre.1 - (SIZE.1 * step) as i64 / 2);
        let pixels = draw(&Wanted { first, step, size: SIZE, seed, generation: start.generation, borders: false });
        keep(Still { name: format!("0_map_{step:02}_cells_a_pixel"), size: SIZE, pixels })?;
    }

    let (host, frames) = Host::start();
    let (_tunings, tunings_read) = channel();
    let pictures = paint::start(frames, tunings_read);
    host.pace(None);
    host.make_world(start);

    // From far, and a cell a pixel: a picture a superchunk, laid side by side.
    let mut scale = farthest.max(1).next_power_of_two();
    loop {
        let detail = detail_at(scale as f32);
        let (tile, cells) = (1 << detail, (SIZE.0 * scale, SIZE.1 * scale));
        let first = first_at(cells, tile);
        let (viewport, expected) = viewport_of(first, (first.0 + cells.0 - 1, first.1 + cells.1 - 1));
        let answer = answered(&host, &pictures, Ask { viewport: Some(viewport), detail, skip: 0, most: u32::MAX, near: None }, expected)?;
        // What a screen shows of it: each painted pixel `tile / scale` screen pixels a side.
        let (grown, mut pixels) = (tile / scale, vec![0u8; (SIZE.0 * SIZE.1 * 4) as usize]);
        for painted in answer.iter().flat_map(|picture| &picture.superchunks) {
            let side = painted.side as i64;
            let left_top = |at: u32, first: u32| (at * SUPERCHUNK_SIDE_CELLS) as i64 / tile as i64 - (first / tile) as i64;
            let (left, top) = (left_top(painted.at.0, first.0), left_top(painted.at.1, first.1));
            for y in top.max(0)..(top + side).min((SIZE.1 / grown) as i64) {
                for x in left.max(0)..(left + side).min((SIZE.0 / grown) as i64) {
                    let from = (((y - top) * side + x - left) * 4) as usize;
                    for (down, across) in (0..grown).flat_map(|down| (0..grown).map(move |across| (down, across))) {
                        let to = (((y as u32 * grown + down) * SIZE.0 + x as u32 * grown + across) * 4) as usize;
                        pixels[to..to + 4].copy_from_slice(&painted.pixels[from..from + 4]);
                    }
                }
            }
        }
        keep(Still { name: format!("1_cells_{scale:02}_cells_a_pixel"), size: SIZE, pixels })?;
        if scale == 1 {
            break;
        }
        scale /= 2;
    }

    // From near: the one picture of the viewport's cells.
    for pixels_a_cell in NEAR_PIXELS {
        let size = (SIZE.0 / pixels_a_cell, SIZE.1 / pixels_a_cell);
        let first = first_at(size, 1);
        let (viewport, expected) = viewport_of((first.0 - NEAR_MARGIN, first.1 - NEAR_MARGIN), (first.0 + size.0 + NEAR_MARGIN, first.1 + size.1 + NEAR_MARGIN));
        let near = Near { first, size, pixels_a_cell };
        let answer = answered(&host, &pictures, Ask { viewport: Some(viewport), detail: 0, skip: 0, most: u32::MAX, near: Some(near) }, expected)?;
        let painted = answer.into_iter().find_map(|picture| picture.near).ok_or("no picture from near came")?;
        keep(Still { name: format!("2_near_{pixels_a_cell}_pixels_a_cell"), size: SIZE, pixels: painted.pixels })?;
    }
    Ok(seed)
}
