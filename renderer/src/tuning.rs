//! What is tuned by eye: a few numbers the window's sliders
//! ([`crate::sliders`]) set -- the near view's shading, which the
//! painter reads each frame, and in the lab ([`crate::lab`]) how the
//! world is generated -- with no lock between them. They are kept from one run to the next
//! ([`path`]), so what was found by eye can be read back and written
//! into the code as the defaults.

use std::fs::{create_dir_all, read_to_string, write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// One number to tune.
pub struct Tuned {
    /// Its name, as shown and as saved.
    pub name: &'static str,
    /// What it is unless set.
    pub default: f32,
    /// The least and the most a slider sets it to.
    pub range: (f32, f32),
    /// The page of sliders it is on.
    pub page: Page,
    /// What it does, said to whoever rests the pointer on its slider.
    pub what: &'static str,
}

/// A page of sliders: the numbers tuned together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// The near view's shading.
    Shading,
    /// How the world is generated: its heights and its grass.
    Generation,
}

/// How much lighter a step's line towards the sun is.
pub const STEP_LIGHT: usize = 0;
/// How much darker a step's line away from the sun is.
pub const STEP_DARK: usize = 1;
/// How much darker the foot of a wall's band is, the wall facing away
/// from the sun.
pub const WALL_SHADE: usize = 2;
/// How much darker it is, the sun on the wall.
pub const WALL_LIT: usize = 3;
/// How much of that the band has lost at its far edge.
pub const WALL_FADE: usize = 4;
/// How much darker a cast shadow is.
pub const SHADOW: usize = 5;
/// How much of the slopes' light and the heights' tint is shown.
pub const RELIEF: usize = 6;
/// How much the ground's pixels differ by lot.
pub const TEXTURE: usize = 7;

/// The height of the ocean's floor: the lowest ground there is.
pub const OCEAN_FLOOR: usize = 8;
/// The height the ocean stands at.
pub const OCEAN_LEVEL: usize = 9;
/// The height the highest land vertex may be at.
pub const HIGHEST_LAND: usize = 10;
/// The share of the vertices that are ocean.
pub const OCEAN_SHARE: usize = 11;
/// The vertices from the ocean within which land is held low.
pub const COAST_BREADTH: usize = 12;
/// How low land beside the ocean is held.
pub const COAST_LOWNESS: usize = 13;
/// How much land and ocean clump.
pub const CLUMPING: usize = 14;
/// The cells along a square of the vertices' grid, as a power of two.
pub const VERTEX_SPACING: usize = 15;
/// The cells the narrowest line's blend is across.
pub const NARROWEST_BLEND: usize = 16;
/// The cells the widest line's blend is across.
pub const WIDEST_BLEND: usize = 17;
/// The least a line's sigmoidness is.
pub const LEAST_SIGMOID: usize = 18;
/// The most a line's sigmoidness is.
pub const MOST_SIGMOID: usize = 19;
/// How far the lines are bent, beside a square's side.
pub const LINE_BENDING: usize = 20;
/// The finer meshes on the land.
pub const FINER_DEPTH: usize = 21;
/// The share of a finer mesh's vertices that raise or sink the land.
pub const FINER_SHARE: usize = 22;
/// The most one of them raises or sinks it.
pub const FINER_HEIGHT: usize = 23;
/// How much of that each mesh finer does, beside the one before.
pub const FINER_FALL: usize = 24;
/// The most a vertex weighs against the meshes finer than its own.
pub const PARENT_WEIGHT: usize = 25;
/// The share of those that raise it.
pub const RAISED_SHARE: usize = 26;
/// The share of the cells that are grass.
pub const GRASS_COVER: usize = 27;
/// The cells across a patch of grass, as a power of two.
pub const GRASS_PATCH: usize = 28;
/// How much finer noise counts beside the patches'.
pub const GRASS_DETAIL: usize = 29;
/// How much each cell's own lot counts: grass scattered, not in patches.
pub const GRASS_SCATTER: usize = 30;
/// The share of the cells that have a tree.
pub const TREE_COVER: usize = 31;
/// The cells across a patch of trees, as a power of two.
pub const TREE_PATCH: usize = 32;
/// How much finer noise counts beside the trees' patches'.
pub const TREE_DETAIL: usize = 33;
/// How much each cell's own lot counts for trees.
pub const TREE_SCATTER: usize = 34;

/// The numbers, in the order above; the shading's defaults are what was
/// found by eye with the sliders.
pub const TUNED: [Tuned; 35] = [
    Tuned { name: "step light", default: 0.35, range: (0.0, 1.0), page: Page::Shading, what: "How much lighter the border of a higher cell is where it faces the sun." },
    Tuned { name: "step dark", default: 0.35, range: (0.0, 0.8), page: Page::Shading, what: "How much darker the border of a higher cell is where it faces away from the sun." },
    Tuned { name: "wall shade", default: 0.49, range: (0.0, 1.0), page: Page::Shading, what: "How dark the band at the foot of a wall is, on the side away from the sun." },
    Tuned { name: "wall lit", default: 0.55, range: (0.0, 1.0), page: Page::Shading, what: "How dark the band at the foot of a wall is, on the side the sun is on." },
    Tuned { name: "wall fade", default: 0.8, range: (0.0, 1.5), page: Page::Shading, what: "How much of its darkness a wall's band has lost at its far edge." },
    Tuned { name: "shadow", default: 0.4, range: (0.0, 0.8), page: Page::Shading, what: "How much darker ground is under a cast shadow." },
    Tuned { name: "relief", default: 0.7, range: (0.0, 3.0), page: Page::Shading, what: "How strongly slopes are lit and heights tinted." },
    Tuned { name: "texture", default: 2.0, range: (0.0, 4.0), page: Page::Shading, what: "How much the ground's pixels differ from one another by lot." },
    Tuned { name: "ocean floor level", default: 256.0, range: (0.0, 4096.0), page: Page::Generation, what: "The height of the ocean's floor, and the lowest ground in the world. Every ocean vertex is at this height." },
    Tuned { name: "ocean level", default: 511.0, range: (0.0, 8192.0), page: Page::Generation, what: "The height of the ocean's surface, the same all over the world. Ground under it is under water. Set under the floor, it is held to the floor: no ocean. Water keeps a depth to 255, so a floor more than 255 under this is drawn and kept as 255 deep." },
    Tuned { name: "highest land", default: 711.0, range: (0.0, 8192.0), page: Page::Generation, what: "The height of the highest land there can be. Every land vertex carries a height of its own, drawn by lot, between just over the ocean level and this." },
    Tuned { name: "ocean share", default: 0.5, range: (0.0, 1.0), page: Page::Generation, what: "The share of the vertices that are ocean; the rest are land. 0 is all land, 1 all ocean." },
    Tuned { name: "coast breadth", default: 2.0, range: (0.0, 4.0), page: Page::Generation, what: "How many vertices from the ocean land is held low within. Past it, a land vertex is as likely at any height as another, up to the highest land. 0, and no land is held low." },
    Tuned { name: "coast lowness", default: 2.5, range: (1.0, 16.0), page: Page::Generation, what: "How strongly land right beside the ocean is held low: each coast vertex draws a height of its own, small ones by far the most likely and the higher the less likely -- so coasts differ a little from one another and a few are high, sea cliffs where their lines' blends are narrow. 1 holds nothing low; 16 leaves high coasts very rare. It eases off each vertex further inland. The finer meshes vary the lowest land a quarter as much as the highest, so differences compound inland." },
    Tuned { name: "clumping", default: 0.25, range: (0.0, 1.0), page: Page::Generation, what: "The share of vertices that are land or ocean together with the three others of their block of four, not each by its own lot. 0, and land is scattered vertex by vertex; more, and it gathers in fuller islands with fewer specks." },
    Tuned { name: "vertex spacing (2^)", default: 13.0, range: (6.0, 18.0), page: Page::Generation, what: "How far apart the vertices are, and so about how long a line is: 2 to this power, in cells. 10 is one superchunk, 13 is 8 superchunks, 16 is 64. The ground at a vertex is at the vertex's height, and slopes from it along its lines to its neighbours'." },
    Tuned { name: "narrowest blend", default: 0.25, range: (0.0, 1.0), page: Page::Generation, what: "A line joins two vertices and carries the ground from one's height to the other's. Every line has a blend of its own, drawn by lot between this and the widest blend: the share of the line, about its middle, the change of height is spread over. 1 is the whole line: one slope from vertex to vertex. Near 0 is a cliff at the line's middle with level ground either side." },
    Tuned { name: "widest blend", default: 1.0, range: (0.0, 1.0), page: Page::Generation, what: "The widest blend a line can draw, as a share of its length. Set both to 1 for slopes everywhere and no level ground; both near 0 for plateaus and cliffs." },
    Tuned { name: "least sigmoid", default: 1.0, range: (1.0, 16.0), page: Page::Generation, what: "Every line also has a sigmoidness of its own, drawn by lot between this and the most: the shape of the change across its blend. 1 is an even slope. Higher is more of an S: gentle at both ends of the blend and steep in the middle, to a step." },
    Tuned { name: "most sigmoid", default: 3.0, range: (1.0, 16.0), page: Page::Generation, what: "The most sigmoid a line can draw. Set both the same for one shape everywhere: 1 and 1 for even slopes only, 16 and 16 for steps only." },
    Tuned { name: "line bending", default: 0.3, range: (0.0, 1.0), page: Page::Generation, what: "How far the lines are pushed out of straight by noise, as a share of the vertex spacing. 0 leaves straight lines and triangles; more makes them wind." },
    Tuned { name: "finer mesh depth", default: 9.0, range: (0.0, 10.0), page: Page::Generation, what: "How many finer meshes lie on the land: points spread again, half as far apart each time, inside the triangles of the mesh before, each raising or sinking the ground a little -- small variations at a time. None gets finer than 16 cells between points, so the shortest lines are 10 to 20 cells. Every mesh moves the land, the finest by two heights at least. 0 for none: the land is the broad triangles alone." },
    Tuned { name: "finer mesh share", default: 0.7, range: (0.0, 1.0), page: Page::Generation, what: "The share of a finer mesh's points that raise or sink the land; the rest leave it as it is." },
    Tuned { name: "finer mesh height", default: 120.0, range: (0.0, 2048.0), page: Page::Generation, what: "How many heights a point of the first finer mesh raises or sinks the land at most, each by an amount of its own." },
    Tuned { name: "finer mesh falloff", default: 0.75, range: (0.0, 1.0), page: Page::Generation, what: "How much each finer mesh moves the land beside the one before: 0.5, and each does half as much; near 1, and the finest do as much as the broadest -- rough ground; near 0, and only the first counts." },
    Tuned { name: "parent weight", default: 0.8, range: (0.0, 1.0), page: Page::Generation, what: "The top level has a weight of 1. Every point hands a share of the weight that reached it down to its subdivisions and keeps the rest; a subdivision moves the land by its own offset times the weight that reached it. This is the most a point may keep, each keeping from none to this by lot. Where the parents keep much, the ground keeps their shape -- a plain stays a plain, a ridge a ridge; where they keep little it is broken up in detail. 0, and every level moves the land freely everywhere." },
    Tuned { name: "raised share", default: 0.6, range: (0.0, 1.0), page: Page::Generation, what: "The share of those vertices that raise the land; the rest sink it. Land sunk under the ocean level fills with water." },
    Tuned { name: "grass cover", default: 0.95, range: (0.0, 1.0), page: Page::Generation, what: "The share of dry land that starts as grass; the rest is dirt." },
    Tuned { name: "grass patch size (2^)", default: 8.0, range: (1.0, 10.0), page: Page::Generation, what: "How broad a patch of grass or of dirt is: 2 to this power, in cells." },
    Tuned { name: "grass patch detail", default: 0.6, range: (0.0, 2.0), page: Page::Generation, what: "How ragged the patches' edges are: how much finer noise counts beside the patches'." },
    Tuned { name: "grass scatter", default: 0.05, range: (0.0, 2.0), page: Page::Generation, what: "How much each cell's own lot counts: grass scattered cell by cell, not in patches." },
    Tuned { name: "tree cover", default: 0.06, range: (0.0, 1.0), page: Page::Generation, what: "The share of dry land that starts with a tree." },
    Tuned { name: "tree patch size (2^)", default: 7.0, range: (1.0, 10.0), page: Page::Generation, what: "How broad a wood is: 2 to this power, in cells." },
    Tuned { name: "tree patch detail", default: 0.8, range: (0.0, 2.0), page: Page::Generation, what: "How ragged the woods' edges are." },
    Tuned { name: "tree scatter", default: 0.3, range: (0.0, 2.0), page: Page::Generation, what: "How much each cell's own lot counts: lone trees, not woods." },
];

/// Counts the changes to how the world is generated: what was made
/// under an earlier count is made again.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// How many times how the world is generated has changed.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// Says that how the world is generated has changed.
pub fn regenerate() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// The numbers as they are now, each a float's bits.
static VALUES: [AtomicU32; TUNED.len()] = [const { AtomicU32::new(0) }; TUNED.len()];

/// The numbers, read together.
pub type Tuning = [f32; TUNED.len()];

/// Where the numbers are kept between runs: a line each, its name and
/// its value. Under the crate's folder, out of git.
pub fn path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("transient_data/tuning.txt")
}

/// Sets every number to its default, then to what was kept, if any.
pub fn start() {
    let kept = read_to_string(path()).unwrap_or_default();
    for (index, tuned) in TUNED.iter().enumerate() {
        let kept = kept.lines().find_map(|line| line.strip_prefix(tuned.name)?.strip_prefix(" = ")?.trim().parse().ok());
        set(index, kept.unwrap_or(tuned.default));
    }
}

/// The numbers now.
pub fn now() -> Tuning {
    std::array::from_fn(|index| f32::from_bits(VALUES[index].load(Ordering::Relaxed)))
}

/// Sets the `index`-th number -- to anything: its range is only what
/// its slider reaches, and a value typed may pass it. The world is to
/// be generated again if it is one of generation's and has changed.
pub fn set(index: usize, value: f32) {
    let bits = if value.is_finite() { value } else { TUNED[index].default }.to_bits();
    if VALUES[index].swap(bits, Ordering::Relaxed) != bits && TUNED[index].page == Page::Generation {
        regenerate();
    }
}

/// Keeps the numbers for the next run. A failure is let pass: they are
/// then only not kept.
pub fn keep() {
    let lines: String = TUNED.iter().zip(now()).map(|(tuned, value)| format!("{} = {value:.3}\n", tuned.name)).collect();
    let path = path();
    _ = path.parent().map(create_dir_all);
    _ = write(path, lines);
}
