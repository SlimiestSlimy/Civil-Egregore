//! TileSim on the screen: a pasture -- grass, dirt and sheep -- ticking
//! on a thread of its own, and a Bevy window showing it: dirt, grass
//! and sheep, on ground lit by its height -- slopes shaded, cliffs
//! casting shadows, and from near, steps and walls drawn at their edges.
//!
//! The window is the one that asks: each time it has shown a frame, it
//! sends the simulation the superchunks in view, and the simulation
//! answers with their cells as its last tick left them ([`sim`]), which
//! a third thread turns into pixels ([`paint`]). The three share
//! nothing else, so none waits on another.
//!
//! `cargo run --release -p renderer -- [superchunks shown] [sheep a superchunk] [ticks a second, 0 flat out] [ticks to watch for] [1 to force every superchunk shown hot]`
//!
//! Forced hot, the world is loaded to be measured: every superchunk
//! shown hot all the while, whatever its sheep come to.
//!
//! `cargo run --release -p renderer -- lab [superchunks shown]` is the
//! lab instead ([`lab`]): a world of the superchunks shown, every one
//! hot, no sheep, its rules ticking, with sliders for how it is
//! generated.
//!
//! It runs until closed. The ticks to watch for are only shown: how far
//! the run is from what whoever started it wanted seen.
//!
//! | key | what it does |
//! |---|---|
//! | arrows, WASD, or dragging with the left button | move the view |
//! | the wheel, or `Q` and `E` | zoom |
//! | space | pause, and go on |
//! | `T` | tick flat out, or at the game's pace |
//! | `F` | the window over the whole screen, or not |
//! | `[` and `]` | halve and double the pace |
//! | `B` | show the superchunks' boundaries, or not, and near enough each one's Morton index and `(x, y)` |
//! | `C` | the same of the chunks |
//! | `H` | show every cell's height, from near enough to read them |
//! | `U` | the next page of sliders, or none: the near view's shading, and in the lab how the world is generated |

// Every item is documented, private ones included; `cargo clippy`
// checks the private ones.
#![warn(missing_docs, clippy::missing_docs_in_private_items)]

mod ground;
mod lab;
mod map;
mod near;
mod paint;
mod sim;
mod sliders;
mod tuning;

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite::Anchor;
use bevy::window::{MonitorSelection, WindowMode};
use coordinates::{place_from_cartesian, square_side, ChunkIndex, SuperchunkIndex, CELLS_IN_CHUNK, CHUNK_SIDE, SUPERCHUNK_SIDE, SUPERCHUNK_SIDE_CELLS, WORLD_MIDDLE};
use paint::Picture;
use sim::{start, Ask, Mode, Near, Request, Viewport, TARGET_PACE};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Mutex;
use world::diagnostics::frames::BROWN;

/// A superchunk's side on the screen's plane: a cell a unit.
const SPRITE_SIDE: f32 = SUPERCHUNK_SIDE_CELLS as f32;

/// Screen heights the view moves a second, by the keys.
const PAN_SPEED: f32 = 0.8;
/// How much nearer or farther a second, by the keys.
const ZOOM_SPEED: f32 = 2.0;
/// How much nearer a notch of the wheel.
const WHEEL_ZOOM: f32 = 0.85;
/// Seconds from one frame asked for to the next, at least: no oftener
/// than a screen shows them.
const SYNC_EVERY: f32 = 1.0 / 60.0;
/// The farthest the world's cells are drawn from: cells a screen
/// pixel. Some 60 superchunks across a screen, each of which may have
/// to be made. Farther, the map ([`map`]) is what is shown.
const FARTHEST: f32 = 32.0;
/// The farthest the view goes: cells a screen pixel, four superchunks.
const MAP_FARTHEST: f32 = 4096.0;
/// Pixels of the map past each of the view's edges: what a moving view
/// shows before the next map comes.
const MAP_MARGIN: i64 = 64;
/// Screen pixels two boundaries are apart before they are drawn.
const LINES_FROM: f32 = 6.0;
/// Boundaries of a kind, across or down, there are sprites for: as
/// many as a screen holds [`LINES_FROM`] apart.
const LINES: u32 = 700;
/// The coarsest the world is drawn: a pixel `2^6` cells a side, a
/// superchunk 16 pixels.
const COARSEST: u32 = 6;
/// Pixels along the side of an image kept when its superchunk goes out
/// of view, at most: a finer one is dropped, to be asked for again.
const KEPT_SIDE: u32 = 64;

/// Cells a screen pixel at most for the view to be from near: a cell
/// two pixels or more.
const NEAR_SCALE: f32 = 0.5;
/// Pixels along a cell's side from near, at most.
const NEAR_PIXELS: u32 = 8;
/// Cells past the view's edges painted from near: what a moving view
/// shows before the next frame comes.
const NEAR_MARGIN: f32 = 8.0;

/// Superchunks' images kept at most: past that, those out of view go.
const TILES_KEPT: usize = 4096;

/// Superchunks a frame carries at most, drawn at `detail`: fewer the
/// finer, a fine one being more to paint and to send to the graphics
/// card. The window goes round those in view, so many frames.
const fn frame_holds(detail: u32) -> u32 {
    match detail {
        0 | 1 => 8,
        2 => 16,
        _ => 32,
    }
}

/// The simulation, as the window holds it: where to ask, where the
/// answers come, and what it was last told.
#[derive(Resource)]
struct Link {
    /// Where requests go.
    requests: Sender<Request>,
    /// Where frames come back, painted.
    frames: Mutex<Receiver<Picture>>,
    /// Whether a frame was asked for and has not come yet.
    waiting: bool,
    /// Seconds since a frame was last asked for.
    since: f32,
    /// What was last asked for: the next frame goes on from it, round
    /// the superchunks in view.
    asked: Option<Ask>,
    /// Whether the simulation is paused.
    paused: bool,
    /// Ticks a second it is held to, or flat out.
    pace: Option<u32>,
    /// Ticks the run is to be watched for, if whoever started it said:
    /// shown, never stopped at.
    watch_for: Option<u64>,
}

/// The world as drawn: an image a superchunk that has been in view,
/// made when its first pixels come.
#[derive(Resource)]
struct Sprites {
    /// The superchunk whose top left the plane's origin is, `(x, y)` in
    /// the world: the plane is counted from near where the view starts,
    /// the world being too wide for its numbers.
    origin: [u32; 2],
    /// Superchunks along the side of the square the view starts on.
    side: u32,
    /// Each superchunk drawn, by where it is in the world: its sprite,
    /// its image, and the pixels along the image's side.
    tiles: HashMap<(u32, u32), (Entity, Handle<Image>, u32)>,
}

impl Sprites {
    /// Those showing `superchunks` about the world's origin superchunk at first.
    fn about_origin(superchunks: u32) -> Self {
        let (side, (x, y)) = (square_side(superchunks), WORLD_MIDDLE.cartesian());
        Self { origin: [x - side / 2, y - side / 2], side, tiles: HashMap::new() }
    }

    /// Where on the plane the cell `cell` cells from the world's edge
    /// is, across (`axis` 0) or down (1).
    fn plane(&self, cell: u32, axis: usize) -> f32 {
        cell.wrapping_sub(self.origin[axis] * SUPERCHUNK_SIDE_CELLS) as i32 as f32
    }

    /// The cell at `plane` on the plane, across or down: the world's
    /// first or last if it is past an edge.
    fn cell(&self, plane: f32, axis: usize) -> u32 {
        ((self.origin[axis] * SUPERCHUNK_SIDE_CELLS) as i64 + plane.floor() as i64).clamp(0, u32::MAX as i64) as u32
    }

    /// The cells in view, across and down: the first and the last.
    fn in_view(&self, transform: &Transform, scale: f32, window: &Window) -> [(u32, u32); 2] {
        let half = Vec2::new(window.width(), window.height()) * scale / 2.0;
        let middle = [transform.translation.x, -transform.translation.y];
        [0, 1].map(|axis| (self.cell(middle[axis] - half[axis], axis), self.cell(middle[axis] + half[axis], axis)))
    }
}

/// What the last frame said of the world.
#[derive(Resource, Default)]
struct Seen {
    /// Ticks run.
    tick: u64,
    /// Ticks a second.
    ticks_a_second: f64,
    /// Sheep.
    sheep: usize,
    /// Cells of grass.
    grass: u64,
    /// Trees.
    trees: u64,
    /// Superchunks the frame painted.
    painted: usize,
    /// Superchunks in view.
    in_view: u32,
    /// How coarsely they are drawn: a pixel `2^detail` cells a side.
    detail: u32,
    /// Pixels along a cell's side, seen from near; 0 if not.
    near_pixels: u32,
    /// Cells along a pixel's side of the map; 0 if it is not shown.
    map: u32,
    /// Seconds of the simulation's thread the frame took.
    sync_seconds: f64,
    /// The share of that thread's time frames take.
    sync_share: f64,
    /// Seconds of the painter's thread the frame took.
    paint_seconds: f64,
}

/// The map, as the window holds it: where to ask for one, where it
/// comes back, and what was last asked for.
#[derive(Resource)]
struct MapLink {
    /// Where the maps wanted go.
    requests: Sender<map::Wanted>,
    /// Where they come back, drawn.
    maps: Mutex<Receiver<map::Drawn>>,
    /// The last asked for.
    asked: Option<map::Wanted>,
}

/// The map's picture, over the superchunks' images.
#[derive(Component)]
struct MapView;

/// What of the map's picture is changed: its image, where it lies, whether it shows.
type MapParts = (&'static Sprite, &'static mut Transform, &'static mut Visibility);

/// The picture of the cells in view from near, over the superchunks' images.
#[derive(Component)]
struct NearView;

/// A boundary drawn over the world: a line between two rows or two
/// columns of chunks, from one side of the world to the other.
#[derive(Component)]
struct Boundary {
    /// Whether it is one of the superchunks' lines, or of the chunks'.
    of_superchunks: bool,
    /// Whether it runs across, or down.
    across: bool,
    /// Which of those in view it is, counted from the first.
    place: u32,
}

/// Which boundaries are shown.
#[derive(Resource, Default)]
struct Boundaries {
    /// The superchunks'.
    superchunks: bool,
    /// The chunks'.
    chunks: bool,
    /// Not a boundary, but shown as they are: every cell's height.
    heights: bool,
}

/// A superchunks' boundary's colour, and its width in screen pixels.
const SUPERCHUNK_LINE: (Color, f32) = (Color::srgba(1.0, 0.85, 0.2, 0.9), 2.0);
/// A chunks' boundary's.
const CHUNK_LINE: (Color, f32) = (Color::srgba(1.0, 1.0, 1.0, 0.45), 1.0);

/// A label in a superchunk's or a chunk's top left corner: one of a
/// few, given to those in view.
#[derive(Component)]
struct Label;

/// The camera, and no label: both have a place.
type CameraOnly = (With<Camera2d>, Without<Label>, Without<HeightLabel>, Without<Boundary>);

/// Labels there are: what a view can hold of them.
const LABELS: usize = 256;
/// Screen pixels a superchunk or a chunk is across before it is labelled.
const LABELLED_FROM: f32 = 150.0;
/// Screen pixels a label is across at its full size: under that much
/// room it is drawn smaller, to fit.
const LABEL_WIDTH: f32 = 420.0;
/// Screen pixels from a corner to its label, and from one line of
/// labels to the next.
const LABEL_LINE: f32 = 24.0;

/// A cell's height, written on it: one of a grid of them, each given
/// to the cell in view whose `(x, y)` is its own, counted round the
/// grid -- so a cell keeps its text while the view moves.
#[derive(Component)]
struct HeightLabel {
    /// Where it is in the grid.
    slot: (u32, u32),
}

/// Height labels across and down: the most cells in view that are labelled.
const HEIGHT_LABELS: (u32, u32) = (96, 54);
/// Screen pixels a cell is across before its height is written on it.
const HEIGHT_FROM: f32 = 20.0;
/// Screen pixels a height is across at its full size: five digits.
const HEIGHT_WIDTH: f32 = 80.0;

/// The text over the world.
#[derive(Component)]
struct Hud;

/// `number` with its digits in threes: 1,234,567.
fn grouped(number: u64) -> String {
    let digits = number.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (place, digit) in digits.chars().enumerate() {
        if place > 0 && (digits.len() - place).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// The `index`-th argument, or `default`.
fn argument(index: usize, default: usize) -> usize {
    std::env::args().nth(index).and_then(|argument| argument.parse().ok()).unwrap_or(default)
}

fn main() {
    tuning::start();
    let in_lab = std::env::args().nth(1).is_some_and(|first| first == "lab");
    // In the lab the only number is the superchunks shown, after the word.
    let (superchunks, flock) = (argument(1 + in_lab as usize, 64) as u32, argument(2, 8000));
    let pace = Some(argument(3, TARGET_PACE as usize) as u32).filter(|&pace| pace > 0);
    let mode = match (in_lab, argument(5, 0) > 0) {
        (true, _) => Mode::Lab,
        (false, true) => Mode::ForcedHot,
        (false, false) => Mode::Halos,
    };
    if in_lab {
        lab::run();
        sliders::show_generation();
    }
    let (requests, frames) = start(superchunks, flock, mode);
    _ = requests.send(Request::Pace(pace));
    let watch_for = Some(argument(4, 0) as u64).filter(|&ticks| ticks > 0);
    App::new()
        .add_plugins(
            DefaultPlugins
                // A cell a pixel, sharp however near.
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin { primary_window: Some(Window { title: "TileSim".to_string(), ..default() }), ..default() }),
        )
        .insert_resource(Link { requests, frames: Mutex::new(paint::start(frames)), waiting: false, since: SYNC_EVERY, asked: None, paused: false, pace, watch_for })
        .insert_resource(Sprites::about_origin(superchunks))
        .insert_resource({
            let (requests, maps) = map::start();
            MapLink { requests, maps: Mutex::new(maps), asked: None }
        })
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<Seen>()
        .init_resource::<Boundaries>()
        .init_resource::<sliders::Hands>()
        .add_systems(Startup, (setup, sliders::setup))
        .add_systems(Update, (fullscreen, sliders::toggle, sliders::scroll, sliders::slide, sliders::tell, recentre, steer, keys, boundaries, labels, heights, show, ask, far, hud).chain())
        .run();
}

/// Dirt, one pixel of it: a superchunk not drawn yet.
const DIRT: [u8; 4] = [BROWN[0], BROWN[1], BROWN[2], u8::MAX];

/// An image `side` pixels a side, of `pixels`.
fn picture(side: u32, pixels: Vec<u8>) -> Image {
    picture_of((side, side), pixels)
}

/// An image `size` pixels across and down, of `pixels`.
fn picture_of(size: (u32, u32), pixels: Vec<u8>) -> Image {
    let size = Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 };
    Image::new(size, TextureDimension::D2, pixels, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default())
}

/// The camera over the middle of the square the view starts on, the
/// whole of it in view; the picture from near, hidden until there is
/// one; the boundaries and the labels, hidden until asked for; and the
/// text. A superchunk's image is made when its pixels first come.
fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, sprites: Res<Sprites>, window: Single<&Window>) {
    let (scale, at) = first_view(&sprites, &window);
    commands.spawn((Camera2d, Projection::Orthographic(OrthographicProjection { scale, ..OrthographicProjection::default_2d() }), Transform::from_translation(at)));
    commands.spawn((Sprite { image: images.add(picture(1, DIRT.to_vec())), ..default() }, Transform::from_xyz(0.0, 0.0, 1.0), Visibility::Hidden, NearView));
    commands.spawn((Sprite { image: images.add(picture(1, DIRT.to_vec())), ..default() }, Transform::from_xyz(0.0, 0.0, 1.0), Visibility::Hidden, MapView));
    // The chunks' boundaries under the superchunks'.
    for (of_superchunks, (colour, _), height) in [(false, CHUNK_LINE, 2.0), (true, SUPERCHUNK_LINE, 3.0)] {
        for place in 0..LINES {
            for across in [true, false] {
                let sprite = Sprite { color: colour, custom_size: Some(Vec2::ONE), ..default() };
                commands.spawn((sprite, Transform::from_xyz(0.0, 0.0, height), Visibility::Hidden, Boundary { of_superchunks, across, place }));
            }
        }
    }
    for _ in 0..LABELS {
        commands.spawn((Text2d::new(""), Anchor::TOP_LEFT, Transform::from_xyz(0.0, 0.0, 4.0), Visibility::Hidden, Label));
    }
    for slot in 0..HEIGHT_LABELS.0 * HEIGHT_LABELS.1 {
        commands.spawn((Text2d::new(""), Transform::from_xyz(0.0, 0.0, 4.0), Visibility::Hidden, HeightLabel { slot: (slot % HEIGHT_LABELS.0, slot / HEIGHT_LABELS.0) }));
    }
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(13.0), ..default() },
        Node { position_type: PositionType::Absolute, top: Val::Px(8.0), left: Val::Px(8.0), padding: UiRect::all(Val::Px(6.0)), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        Hud,
    ));
}

/// The view the window starts on: cells a screen pixel, and where on
/// the plane its middle is -- the whole of the square about the
/// world's origin in view.
fn first_view(sprites: &Sprites, window: &Window) -> (f32, Vec3) {
    let square_side = sprites.side as f32 * SPRITE_SIDE;
    ((square_side / window.height().min(window.width())).min(FARTHEST), Vec3::new(square_side / 2.0, -square_side / 2.0, 0.0))
}

/// Puts the view back where it started, when the lab's seed is drawn again.
fn recentre(camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>, sprites: Res<Sprites>, window: Single<&Window>) {
    if !lab::view_reset() {
        return;
    }
    let (mut transform, mut projection) = camera.into_inner();
    if let Projection::Orthographic(view) = &mut *projection {
        (view.scale, transform.translation) = first_view(&sprites, &window);
    }
}

/// Moves and zooms the view: the keys, the wheel, and dragging.
fn steer(
    camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    window: Single<&Window>,
    time: Res<Time>,
) {
    let (mut transform, mut projection) = camera.into_inner();
    let Projection::Orthographic(view) = &mut *projection else {
        return;
    };
    let held = |these: [KeyCode; 2]| keys.any_pressed(these) as i32 as f32;
    let nearer = held([KeyCode::KeyE, KeyCode::Equal]) - held([KeyCode::KeyQ, KeyCode::Minus]);
    // The wheel over the sliders scrolls them.
    let wheel = if sliders::over(&window) { 0.0 } else { scroll.delta.y };
    view.scale *= WHEEL_ZOOM.powf(wheel) * ZOOM_SPEED.powf(-nearer * time.delta_secs());
    view.scale = view.scale.clamp(0.02, MAP_FARTHEST);
    let across = held([KeyCode::KeyD, KeyCode::ArrowRight]) - held([KeyCode::KeyA, KeyCode::ArrowLeft]);
    let up = held([KeyCode::KeyW, KeyCode::ArrowUp]) - held([KeyCode::KeyS, KeyCode::ArrowDown]);
    let step = PAN_SPEED * window.height() * view.scale * time.delta_secs();
    transform.translation += Vec3::new(across * step, up * step, 0.0);
    if buttons.pressed(MouseButton::Left) && !sliders::held() {
        // The world follows the pointer.
        transform.translation += Vec3::new(-motion.delta.x, motion.delta.y, 0.0) * view.scale;
    }
}

/// Puts the window over the whole screen, or back, by its key.
fn fullscreen(keys: Res<ButtonInput<KeyCode>>, mut window: Single<&mut Window>) {
    if keys.just_pressed(KeyCode::KeyF) {
        window.mode = match window.mode {
            WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
            _ => WindowMode::Windowed,
        };
    }
}

/// Pauses and paces the simulation.
fn keys(mut link: ResMut<Link>, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::Space) {
        link.paused = !link.paused;
        _ = link.requests.send(Request::Pause(link.paused));
    }
    let pace = if keys.just_pressed(KeyCode::KeyT) {
        if link.pace.is_some() { None } else { Some(TARGET_PACE) }
    } else if keys.just_pressed(KeyCode::BracketLeft) {
        Some((link.pace.unwrap_or(TARGET_PACE) / 2).max(1))
    } else if keys.just_pressed(KeyCode::BracketRight) {
        Some(link.pace.unwrap_or(TARGET_PACE).saturating_mul(2))
    } else {
        return;
    };
    link.pace = pace;
    _ = link.requests.send(Request::Pace(pace));
}

/// Shows and hides the boundaries by their keys, and lays those shown
/// over the lines in view, as wide on the screen however near the view
/// is -- once the lines are far enough apart on it to be told apart.
fn boundaries(
    mut shown: ResMut<Boundaries>,
    mut lines: Query<(&Boundary, &mut Transform, &mut Visibility)>,
    camera: Single<(&Transform, &Projection), CameraOnly>,
    window: Single<&Window>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    shown.superchunks ^= keys.just_pressed(KeyCode::KeyB);
    shown.chunks ^= keys.just_pressed(KeyCode::KeyC);
    let (camera, projection) = *camera;
    let Projection::Orthographic(view) = projection else {
        return;
    };
    let half = Vec2::new(window.width(), window.height()) * view.scale / 2.0;
    // The view's middle on the plane, across and down.
    let middle = Vec2::new(camera.translation.x, -camera.translation.y);
    for (boundary, mut transform, mut visibility) in &mut lines {
        let (is_shown, (_, pixels), apart) = if boundary.of_superchunks { (shown.superchunks, SUPERCHUNK_LINE, SPRITE_SIDE) } else { (shown.chunks, CHUNK_LINE, CHUNK_SIDE as f32) };
        // A line across is one of those down the view, and the other way round.
        let (along, over) = if boundary.across { (1, 0) } else { (0, 1) };
        let at = (((middle[along] - half[along]) / apart).ceil() + boundary.place as f32) * apart;
        if !is_shown || apart / view.scale < LINES_FROM || at > middle[along] + half[along] {
            *visibility = Visibility::Hidden;
            continue;
        }
        let (width, length) = (pixels * view.scale, 2.0 * half[over]);
        *transform = if boundary.across {
            Transform::from_xyz(middle.x, -at, transform.translation.z).with_scale(Vec3::new(length, width, 1.0))
        } else {
            Transform::from_xyz(at, -middle.y, transform.translation.z).with_scale(Vec3::new(width, length, 1.0))
        };
        *visibility = Visibility::Visible;
    }
}

/// Writes every cell's height on it, by its key, once the cells are
/// large enough on the screen to read it and few enough for the labels
/// there are.
fn heights(
    mut shown: ResMut<Boundaries>,
    sprites: Res<Sprites>,
    keys: Res<ButtonInput<KeyCode>>,
    camera: Single<(&Transform, &Projection), CameraOnly>,
    window: Single<&Window>,
    mut labels: Query<(&HeightLabel, &mut Text2d, &mut Transform, &mut Visibility)>,
) {
    shown.heights ^= keys.just_pressed(KeyCode::KeyH);
    let (transform, projection) = *camera;
    let Projection::Orthographic(view) = projection else {
        return;
    };
    let [(first_x, last_x), (first_y, last_y)] = sprites.in_view(transform, view.scale, &window);
    let (first, last) = ((first_x, first_y), (last_x, last_y));
    let readable = shown.heights && 1.0 / view.scale >= HEIGHT_FROM && last.0 - first.0 < HEIGHT_LABELS.0 && last.1 - first.1 < HEIGHT_LABELS.1;
    let size = view.scale * (1.0 / view.scale / HEIGHT_WIDTH).min(1.0);
    let (seed, shape) = (lab::seed(), lab::generation().shape);
    for (label, mut text, mut transform, mut visibility) in &mut labels {
        // The cell in view that is the label's: the first at or past the view's first whose place round the grid is its slot.
        let round = |first: u32, slot: u32, labels: u32| first + (slot + labels - first % labels) % labels;
        let (x, y) = (round(first.0, label.slot.0, HEIGHT_LABELS.0), round(first.1, label.slot.1, HEIGHT_LABELS.1));
        if !readable || x > last.0 || y > last.1 {
            *visibility = Visibility::Hidden;
            continue;
        }
        let height = terrain::height_shaped(&shape, seed, x, y).to_string();
        if text.0 != height {
            text.0 = height;
        }
        *transform = Transform::from_xyz(sprites.plane(x, 0) + 0.5, -(sprites.plane(y, 1) + 0.5), 4.0).with_scale(Vec3::splat(size));
        *visibility = Visibility::Visible;
    }
}

/// Labels the superchunks and chunks in view whose boundaries are
/// shown, once they are large enough on the screen: each one's Morton
/// index and its `(x, y)`, in its top left corner -- a chunk's a line
/// below, clear of its superchunk's.
fn labels(
    shown: Res<Boundaries>,
    sprites: Res<Sprites>,
    camera: Single<(&Transform, &Projection), CameraOnly>,
    window: Single<&Window>,
    mut labels: Query<(&mut Text2d, &mut Transform, &mut Visibility), With<Label>>,
) {
    let (transform, projection) = *camera;
    let Projection::Orthographic(view) = projection else {
        return;
    };
    // The chunks in view, counted from the world's top left: every label is at a chunk's corner.
    let [(first_x, last_x), (first_y, last_y)] = sprites.in_view(transform, view.scale, &window).map(|(first, last)| (first / CHUNK_SIDE as u32, last / CHUNK_SIDE as u32));
    let (first, last) = ((first_x, first_y), (last_x, last_y));
    // How large a label of something `cells` across is drawn, of its full size: none if there is no room to read it.
    let fitted = |cells: usize| Some(cells as f32 / view.scale).filter(|&room| room >= LABELLED_FROM).map(|room| (room / LABEL_WIDTH).min(1.0));
    let (superchunk_size, chunk_size) = (fitted(SPRITE_SIDE as usize).filter(|_| shown.superchunks), fitted(CHUNK_SIDE).filter(|_| shown.chunks));
    let mut wanted = Vec::new();
    for y in first.1..=last.1 {
        for x in first.0..=last.0 {
            let (within_x, within_y) = (x % SUPERCHUNK_SIDE as u32, y % SUPERCHUNK_SIDE as u32);
            let superchunk = SuperchunkIndex::from_cartesian(x / SUPERCHUNK_SIDE as u32, y / SUPERCHUNK_SIDE as u32);
            let (superchunk_x, superchunk_y) = superchunk.cartesian();
            let corner = Vec2::new(sprites.plane(x * CHUNK_SIDE as u32, 0), -sprites.plane(y * CHUNK_SIDE as u32, 1));
            if let Some(size) = superchunk_size.filter(|_| (within_x, within_y) == (0, 0)) {
                wanted.push((corner, 0.0, size, format!("superchunk {:011x} ({superchunk_x}, {superchunk_y})", superchunk.0)));
            }
            if let Some(size) = chunk_size {
                let place = place_from_cartesian(within_x * CHUNK_SIDE as u32, within_y * CHUNK_SIDE as u32) / CELLS_IN_CHUNK;
                let (chunk_x, chunk_y) = (x, y);
                wanted.push((corner, superchunk_size.unwrap_or(0.0), size, format!("chunk {:012x} ({chunk_x}, {chunk_y})", ChunkIndex::of(superchunk, place).0)));
            }
        }
    }
    let mut wanted = wanted.into_iter();
    for (mut text, mut transform, mut visibility) in &mut labels {
        let Some((corner, lines_above, size, label)) = wanted.next() else {
            *visibility = Visibility::Hidden;
            continue;
        };
        if text.0 != label {
            text.0 = label;
        }
        // As large on the screen however near the view is, and a little in from the corner.
        let inset = Vec2::new(LABEL_LINE / 4.0, -LABEL_LINE / 4.0 - lines_above * LABEL_LINE) * view.scale;
        *transform = Transform::from_translation((corner + inset).extend(4.0)).with_scale(Vec3::splat(view.scale * size));
        *visibility = Visibility::Visible;
    }
}

/// Shows the frame the simulation answered with, if it has: each
/// superchunk's image, and the picture from near -- hidden once the
/// view is no longer near.
fn show(
    mut commands: Commands,
    mut link: ResMut<Link>,
    mut sprites: ResMut<Sprites>,
    mut images: ResMut<Assets<Image>>,
    mut seen: ResMut<Seen>,
    near_view: Single<(&Sprite, &mut Transform, &mut Visibility), With<NearView>>,
) {
    let (near_sprite, mut near_transform, mut near_visibility) = near_view.into_inner();
    if seen.near_pixels == 0 {
        *near_visibility = Visibility::Hidden;
    }
    let frame = link.frames.lock().expect("the frames' receiver").try_iter().last();
    if let Some(frame) = frame {
        link.waiting = false;
        *seen = Seen {
            tick: frame.tick,
            ticks_a_second: frame.ticks_a_second,
            sheep: frame.sheep,
            grass: frame.grass,
            trees: frame.trees,
            painted: frame.superchunks.len(),
            in_view: seen.in_view,
            detail: seen.detail,
            near_pixels: seen.near_pixels,
            map: seen.map,
            sync_seconds: frame.sync_seconds,
            sync_share: frame.sync_share,
            paint_seconds: frame.paint_seconds,
        };
        for painted in frame.superchunks {
            if painted.cold {
                // Nothing to draw: what was drawn of it goes.
                if let Some((sprite, ..)) = sprites.tiles.remove(&painted.at) {
                    commands.entity(sprite).despawn();
                }
                continue;
            }
            let (at, side) = (painted.at, painted.side);
            if let Some((_, image, drawn)) = sprites.tiles.get_mut(&at) {
                if let Some(mut image) = images.get_mut(&*image) {
                    *image = picture(side, painted.pixels);
                    *drawn = side;
                }
                continue;
            }
            // Its first pixels: a sprite a superchunk's side on the plane whatever its image's; the world's y grows downwards, the plane's upwards.
            let image = images.add(picture(side, painted.pixels));
            let sprite = Sprite { image: image.clone(), custom_size: Some(Vec2::splat(SPRITE_SIDE)), ..default() };
            let middle = [0, 1].map(|axis| sprites.plane([at.0, at.1][axis] * SUPERCHUNK_SIDE_CELLS, axis) + SPRITE_SIDE / 2.0);
            let sprite = commands.spawn((sprite, Transform::from_xyz(middle[0], -middle[1], 0.0))).id();
            sprites.tiles.insert(at, (sprite, image, side));
        }
        if let Some(painted) = frame.near {
            let (first, size) = (painted.near.first, painted.near.size);
            if let Some(mut image) = images.get_mut(&near_sprite.image) {
                *image = picture_of((size.0 * painted.near.pixels_a_cell, size.1 * painted.near.pixels_a_cell), painted.pixels);
            }
            // Over the cells it is of, a cell a unit whatever its pixels.
            *near_transform = Transform::from_xyz(sprites.plane(first.0, 0) + size.0 as f32 / 2.0, -(sprites.plane(first.1, 1) + size.1 as f32 / 2.0), 1.0).with_scale(Vec3::new(1.0 / painted.near.pixels_a_cell as f32, 1.0 / painted.near.pixels_a_cell as f32, 1.0));
            if seen.near_pixels > 0 {
                *near_visibility = Visibility::Visible;
            }
        }
    }
}

/// Asks the simulation for the next frame: the superchunks now in view.
fn ask(
    mut commands: Commands,
    mut link: ResMut<Link>,
    mut sprites: ResMut<Sprites>,
    mut seen: ResMut<Seen>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
    window: Single<&Window>,
    time: Res<Time>,
) {
    link.since += time.delta_secs();
    if link.waiting || link.since < SYNC_EVERY {
        return;
    }
    let (transform, projection) = *camera;
    let Projection::Orthographic(view) = projection else {
        return;
    };
    let [(left, right), (top, bottom)] = sprites.in_view(transform, view.scale, &window);
    let superchunk = |cell: u32| cell / SUPERCHUNK_SIDE_CELLS;
    let viewport = Viewport { first: (superchunk(left), superchunk(top)), last: (superchunk(right), superchunk(bottom)) };
    // A pixel of the screen is `scale` cells: drawn no finer than that.
    let detail = (view.scale.max(1.0).log2().floor() as u32).min(COARSEST);
    let in_view = (viewport.last.0 - viewport.first.0 + 1) * (viewport.last.1 - viewport.first.1 + 1);
    // From near, the cells in view and a margin about them, as many pixels a cell as the screen shows.
    let margin = NEAR_MARGIN as u32;
    let near = (view.scale <= NEAR_SCALE).then(|| {
        let first = (left.saturating_sub(margin), top.saturating_sub(margin));
        let size = (right.saturating_add(margin) - first.0 + 1, bottom.saturating_add(margin) - first.1 + 1);
        Near { first, size, pixels_a_cell: (1 << (1.0 / view.scale).log2().floor() as u32).min(NEAR_PIXELS) }
    });
    let near = near.filter(|near| near.size.0 > 0 && near.size.1 > 0);
    // From near every superchunk in view is in the one picture; from as far as the map, none is asked for.
    let most = match near {
        Some(_) => in_view,
        None if view.scale > FARTHEST => 0,
        None => frame_holds(detail),
    };
    // On round the superchunks in view from the last frame's, or from the first if the view changed.
    let skip = match link.asked {
        Some(last) if near.is_none() && last.near.is_none() && last.viewport == viewport && last.detail == detail && last.skip + last.most < in_view => last.skip + last.most,
        _ => 0,
    };
    let ask = Ask { viewport, detail, skip, most, near };
    (seen.in_view, seen.detail, seen.near_pixels) = (in_view, detail, near.map_or(0, |near| near.pixels_a_cell));
    seen.map = if view.scale > FARTHEST { map_step(view.scale) } else { 0 };
    // Fine images of superchunks gone out of view are dropped, each 4 MiB here and as much on the graphics card; and every one out of view, once there are very many.
    let many = sprites.tiles.len() > TILES_KEPT;
    sprites.tiles.retain(|&(x, y), (sprite, _, side)| {
        let in_view = (viewport.first.0..=viewport.last.0).contains(&x) && (viewport.first.1..=viewport.last.1).contains(&y);
        let kept = in_view || (*side <= KEPT_SIDE && !many);
        if !kept {
            commands.entity(*sprite).despawn();
        }
        kept
    });
    link.waiting = link.requests.send(Request::Sync(ask)).is_ok();
    link.asked = Some(ask);
    link.since = 0.0;
}

/// Cells along a pixel's side of the map, the view `scale` cells a
/// screen pixel: a power of two, no more than the screen's pixels'.
fn map_step(scale: f32) -> u32 {
    1 << scale.log2().floor() as u32
}

/// Shows the map from farther than the cells are drawn from: asks for
/// one of what is in view when the view, the seed or how the world is
/// generated has changed, and lays the last drawn where it is of.
fn far(
    mut link: ResMut<MapLink>,
    sprites: Res<Sprites>,
    mut images: ResMut<Assets<Image>>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
    window: Single<&Window>,
    map_view: Single<MapParts, (With<MapView>, Without<Camera2d>)>,
) {
    let (sprite, mut transform, mut visibility) = map_view.into_inner();
    let (camera, projection) = *camera;
    let Projection::Orthographic(view) = projection else {
        return;
    };
    if view.scale <= FARTHEST {
        visibility.set_if_neq(Visibility::Hidden);
        link.asked = None;
        return;
    }
    // The view's pixels and a margin, from a corner that is a whole number of pixels: the same cells whatever way it is moved.
    let step = map_step(view.scale);
    let half = Vec2::new(window.width(), window.height()) * view.scale / 2.0;
    let corner = |axis: usize, middle: f32, half: f32| ((sprites.origin[axis] * SUPERCHUNK_SIDE_CELLS) as i64 + (middle - half).floor() as i64).div_euclid(step as i64) - MAP_MARGIN;
    let first = (corner(0, camera.translation.x, half.x), corner(1, -camera.translation.y, half.y));
    let pixels = |half: f32| (2.0 * half / step as f32).ceil() as u32 + 2 * MAP_MARGIN as u32 + 1;
    let wanted = map::Wanted { first: (first.0 * step as i64, first.1 * step as i64), step, size: (pixels(half.x), pixels(half.y)), seed: lab::seed(), generation: lab::generation() };
    // Not for every pixel the view moves: only once it is half the margin from what was asked for, or anything else differs.
    let near_enough = |asked: &map::Wanted| {
        let moved = |asked: i64, wanted: i64| (asked - wanted).abs() / step as i64 <= MAP_MARGIN / 2;
        (asked.step, asked.size, asked.seed, asked.generation) == (wanted.step, wanted.size, wanted.seed, wanted.generation) && moved(asked.first.0, wanted.first.0) && moved(asked.first.1, wanted.first.1)
    };
    if !link.asked.as_ref().is_some_and(near_enough) && link.requests.send(wanted).is_ok() {
        link.asked = Some(wanted);
    }
    let drawn = link.maps.lock().expect("the maps' receiver").try_iter().last();
    if let Some(drawn) = drawn {
        let (wanted, size) = (drawn.wanted, drawn.wanted.size);
        if let Some(mut image) = images.get_mut(&sprite.image) {
            *image = picture_of(size, drawn.pixels);
        }
        // Over the cells it is of: a pixel `step` units.
        let plane = |axis: usize, first: i64, pixels: u32| (first - (sprites.origin[axis] * SUPERCHUNK_SIDE_CELLS) as i64) as f32 + (pixels * wanted.step) as f32 / 2.0;
        *transform = Transform::from_xyz(plane(0, wanted.first.0, size.0), -plane(1, wanted.first.1, size.1), 1.5).with_scale(Vec3::new(wanted.step as f32, wanted.step as f32, 1.0));
        visibility.set_if_neq(Visibility::Visible);
    }
}

/// Writes what the last frame said over the world.
fn hud(mut text: Single<&mut Text, With<Hud>>, seen: Res<Seen>, link: Res<Link>) {
    let pace = match (link.paused, link.pace) {
        (true, _) => "paused".to_string(),
        (false, Some(pace)) => format!("held to {pace} ticks a second"),
        (false, None) => "flat out".to_string(),
    };
    let watched = match link.watch_for {
        Some(ticks) if seen.tick >= ticks => format!("   watched for {} ticks, as asked: close when you like", grouped(ticks)),
        Some(ticks) => format!("   of {} to watch for ({:.0}%)", grouped(ticks), 100.0 * seen.tick as f64 / ticks as f64),
        None => String::new(),
    };
    let drawn = match seen.near_pixels {
        0 if seen.map > 0 => format!("the map, a pixel {} cells a side", seen.map),
        0 => format!("a pixel {} cell(s) a side", 1u32 << seen.detail),
        pixels => format!("a cell {pixels} pixels a side"),
    };
    text.0 = format!(
        "seed {:016x}   tick {}{watched}\n{} ticks a second ({pace})\n{} sheep   {} cells of grass   {} trees\n{} superchunk(s) in view, {drawn}\na frame, {} of them: {:.0} us of the simulation ({:.2}% of its time), {:.1} ms painting\nmove: arrows, WASD, drag   zoom: wheel, Q E   space: pause\nT: flat out   [ ]: pace   F: fullscreen   B: superchunks   C: chunks   H: heights   U: sliders",
        lab::seed(),
        grouped(seen.tick),
        grouped(seen.ticks_a_second as u64),
        grouped(seen.sheep as u64),
        grouped(seen.grass),
        grouped(seen.trees),
        seen.in_view,
        seen.painted,
        seen.sync_seconds * 1e6,
        seen.sync_share * 100.0,
        seen.paint_seconds * 1e3
    );
}
