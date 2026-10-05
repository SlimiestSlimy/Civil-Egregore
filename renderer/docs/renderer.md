# The renderer

TileSim on the screen: a pasture ticking on a thread of its own, and a
Bevy window showing it -- dirt brown, grass green, a sheep white -- on
ground lit by its height (below). The world is a generated one
(`server::generate_flocks`), from the workspace's seed
(`utilities::seed`, `TILESIM_SEED` to pick one): the first from it
with land about the origin (`server::seed_with_land`).

`cargo run --release -p renderer -- [superchunks shown] [sheep a superchunk] [ticks a second, 0 flat out] [ticks to watch for] [1 to force hot]`;
64 superchunks shown, 8,000 sheep on each and 256 ticks a second if
not said: a flock that reaches the most the grass feeds, some 16,000 a
superchunk, within 70,000 ticks, and has not eaten it bare before.

The world is a generated one (`server::generate_flocks`, a flock on
every superchunk shown): hot only in the halos about its sheep, so the window shows a square of superchunks
about the world's origin, and those cold are black. Forced hot (the
fifth argument), every superchunk shown is hot all the while, each
with a flock, whatever its sheep come to: the world under a fixed load,
to be measured -- the census then carries the seconds and the pace
held, 0 flat out.

It runs until it is closed: long runs are watched, not waited for. The
ticks to watch for are only shown, beside the ticks run, so whoever
watches knows how far the run is from what was to be seen. As it goes
it keeps a census -- the flock and the grass every 1,000 ticks, in
`transient_data/measurements/census.csv` -- so a run closed at any time
leaves what it came to. And it shows what it costs: the time each
frame takes of the simulation's thread, and of the painter's. It ticks on every thread the machine has, no more than the
superchunks.

## The window asks

Bevy is the window, the drawing and the keys, and nothing else: the
world is not in its entities, and the simulation knows nothing of it.
The two share two queues and no memory.

The window is the one that asks, never the simulation that sends: each
time the window has shown a frame -- 60 times a second at most -- it
sends the simulation the superchunks in view (`Request::Sync`), and the
simulation, between two ticks, answers with their cells as the last
tick left them (a `Frame`). So the window sets how often the world is
drawn; a window that falls behind slows no tick; one frame at most is
ever on its way; and what is not in view is never sent.

## Three threads

1. **The simulation** only copies: each superchunk in view as its
   grass's words, as the arena holds them (128 KiB), and the cells its
   sheep stand on. What is in view costs the ticks next to nothing,
   however much of it there is.
2. **The painter** turns cells into pixels, lit by their height,
   taking no time from the ticks or from the window's frames.
3. **The window** shows the pixels, an image a superchunk.

What is sent is what the cells are, not the writes that changed them: a
window replaying writes would have to hold the world again and apply
every one as the arena does, and one lost would leave it wrong for
good.

## The whole world

The view is not held to the superchunks it starts on: it goes anywhere
in the world, as far out as 32 cells a screen pixel. A superchunk has
an image only once its pixels have come, and a cold one none: it is
black. The plane the images lie on is counted from the corner of the
square the view starts on, not from the world's -- the world is 2^32
cells wide, more than the plane's numbers tell apart. Boundaries,
labels and heights are laid over whatever is in view.

## Many superchunks

A world of 1,024 superchunks -- 32,768 cells a side -- is seen whole,
which a pixel a cell cannot do: that would be 4 GiB of pixels a frame.

- **From far off, a pixel is a tile of cells**, `2^detail` a side, as
  many as a screen pixel covers, up to 64: the tile's colours mixed,
  brown and green by its grass, white by its sheep. A tile is
  a run of bits in Morton order, so its grass is counted from the words
  with no cell looked at.
- **A frame carries only so many superchunks** -- 8 drawn fine, 32
  coarse -- and the window goes round those in view, frame after frame.
  So what a frame costs the simulation is the same however many are in
  view; the whole view is drawn afresh in half a second at most.
- **A fine image is dropped when its superchunk leaves the view**: it
  is 4 MiB here and as much on the graphics card.

A frame of one superchunk takes very little of the simulation's
thread, however large the world: the HUD says how much.

## Height, from straight above

The view is fully vertical, so height is shown by light alone, the sun
to the top left and 35 degrees up; a cell is 2 m and a height 1 m
(`src/ground.rs`). A superchunk's heights come with the first frame it is
hot in, as its image holds them; the painter works out from the seed
only the cells past its edges that no frame brought (138 before, for
the shadows cast onto it, and 10 after) -- measured, 23 ms a
superchunk's ground where working every height out again took 90 --
once, on a thread a superchunk, and keeps them: the coarse levels for good, the fine
parts (8 MiB) for the 48 superchunks last seen.

- **Hillshade**: slopes facing the sun lighter, those facing away
  darker, off the heights smoothed, in bands 7% apart.
- **Tint**: high ground a little lighter than low.
- **Cast shadows**: one sweep down the sun's diagonal, a shadow
  followed 128 cells (`SHADOW_REACH`) and no further -- within what is
  kept before a superchunk, so it is the same on both sides of where
  two meet; each cell keeps
  how high the shadow line stands over it, so from near a shadow's
  edge is found within the cell, with no sweep over pixels.
- **From a cell a pixel outwards**: cliffs darkened by the walls in the
  pixel, and a contour every 8, 16 or 32 heights.
- **From near** (`src/near.rs`), a cell 2, 4 or 8 pixels -- as many as
  the screen shows -- the cells in view are one picture, and height is
  drawn at the edges: wherever a cell is higher than the one beside
  it, a thin line along the higher cell's border, light towards the
  sun and dark away; and under a wall a band on the ground at its
  foot, darkest there and fading from it, tuned apart for a
  wall the sun is on and one facing away. The band is longer the
  higher the wall -- 4 eighths of a cell and half an eighth a height
  it rises, 5 at the least -- and no darker: a cliff's runs over the
  cells before it, as far as 8 (`ground::MARGIN`), over ground that
  does not itself drop by a wall. Walls are drawn much
  the stronger: they are what cannot be crossed.

**Corners.** A pixel takes one edge's doing, never two multiplied: the
darkest of the edges that darken it, and only if none does, the
lightest of those that lighten it. A cast shadow darkens an edge's
shade as it does the ground: a step's dark line lies in the shadow the
step casts, and would be lost in it otherwise. A wall met only down
a diagonal fills the corner's square, as far off as its band reaches,
joining the bands either side -- if the cells either side are no
higher and no wall is as near straight up or across; a step met only at a corner draws nothing. So bands turn corners as one
outline, with no doubled patch and no gap.

## Still to come

The words of every superchunk a frame carries are still copied whole,
and painted here: to come, only the chunks changed since they were last
sent, and the words coloured by the graphics card itself, with no
pixels made here.

## Keys

| key | what it does |
|---|---|
| arrows, WASD, or dragging with the left button | move the view |
| the wheel, or `Q` and `E` | zoom |
| space | pause, and go on |
| `T` | tick flat out, or at the game's pace (256 ticks a second) |
| `F` | the window over the whole screen, or not |
| `U` | the sliders' menu, opened; whatever of them is open, closed |
| Escape | the options, opened or closed: going on, saving the world, opening one, leaving TileSim |
| `[` and `]` | halve and double the pace |
| `B` | show the superchunks' boundaries, or not; and once a superchunk is 150 screen pixels across, its Morton index (as its save file is named) and `(x, y)` in its top left corner |
| `C` | the same of the chunks, their labels a line below |
| `H` | every cell's height written on it, once a cell is 20 screen pixels across |

## Menus

The sliders at the window's top right and the options Escape opens are
the `gui` crate's (`gui/docs/gui.md`), added to the window's app. The
renderer reads the numbers the sliders tune, leaves the pointer and
the wheel to a menu they are over, and opens the world chosen in the
options: the simulation's thread loads it from the worlds' folder
(`sim::Request::Open`) and runs it in place of the one run, hot in its
halos, its ticks its own, the view back where it started. A world that
cannot be read is refused, the reason on the first line of the text at
the top left, and the world run goes on. Saving
(`sim::Request::Save`) is on the simulation's thread too, between two
ticks: the world written to its folder of the worlds' folder, under
the name it was opened by or the one typed, how it is generated with
it -- so a world tuned in the lab is saved as tuned. What either came
to is said on that first line. A world opened from the lab ends the
lab -- generation's sliders stay listed and do nothing.

## The map

From farther than 32 cells a screen pixel -- to 4,096, four superchunks
a pixel -- the cells are no longer asked for: what shows is the map
(`src/map.rs`). A pixel is the cell in its middle as that cell is
generated: its height from the seed, the ocean over it darker the
deeper, or grass, dirt or a tree on it, lit by its slope and its
height. Nothing is made hot to draw it and nothing of the simulation is
read, so it is how the world was generated, not how it has changed
since. A thread of its own draws the last map asked for, its rows
shared out among every thread the machine has; a new one is asked for
when the view has moved 32 pixels, zoomed to another power of two, or
the seed or generation has changed.

## The lab

`cargo run --release -p renderer -- lab [superchunks shown]`, 64 (8 by
8) if not said, is the renderer run to tune by eye how the world is
made and how it changes (`src/lab.rs`). The world starts as the
superchunks shown, every one hot, and reaches wherever it is looked at:
a superchunk that comes into view is generated, made hot and kept so,
with no bound but memory -- until a slider of
generation moves or the seed is drawn, when it starts over; the seed
drawn, the view too goes back to where it started. No sheep,
its rules -- grass and trees -- ticking as in any run, paused and paced
by the same keys. Generation's sliders are shown from the start:

- **The ocean** (`worldgen::Shape`): its floor's level, the lowest
  ground; its own level; and its share of the vertices.
- **The land**: the highest it may be; the coast's breadth, the
  vertices from the ocean land is held low within, and how low; how
  much land and ocean clump; and how far apart the
  vertices are.
- **The lines**: the narrowest and widest blend, the least and most
  sigmoidness, and how far they are bent. `P` draws the lines over the
  map.
- **The finer meshes**: how many, the share of their vertices that
  raise or sink the land, by how much, how much less each finer mesh
  does, how unevenly the weight is shared out among subdivisions, and
  the share that raise the land.
- **The grass**, and **the trees** (`worldgen::patches`): each in patches,
  not scattered cell by cell. A cell's number is smooth noise as broad
  as a patch, finer noise on it (detail), and a lot drawn for the cell
  alone (scatter); it has the thing under a threshold found, by looking
  at 16,384 cells, so that the share asked for (cover) do.
- **The button** draws a new seed off the clock; the seed is shown in
  the text.

Whenever a slider of generation moves or the seed is drawn, the world
is made afresh from nothing but the sliders, and its ticks start again
from 0. A slider says what it does when the pointer rests on it, and
the panel scrolls. The sliders start at the numbers last settled on
(`gui::tuning`); `server::Generation::DEFAULT` and
`worldgen::Shape::DEFAULT`, which a world made outside the lab uses,
are not those yet.

## Layout

| folder | what is in it |
|---|---|
| `src/sim.rs` | the simulation's thread: requests read between ticks, the cells in view copied when asked |
| `src/paint.rs` | the painter's thread: cells into pixels |
| `src/ground.rs` | the light on the ground: heights as a frame brings them, hillshade, tint, cast shadows, cliffs and contours |
| `src/map.rs` | the map: the world from far, drawn from the generator alone |
| `src/lab.rs` | the lab: the seed and how the world is generated, as the sliders have it, or the opened world's |
| `src/near.rs` | the cells in view from near as one picture: steps and walls at their edges |
| `src/main.rs` | the window: the camera, an image a superchunk, the keys, the text |
| `docs/` | this, and the reference, function by function |
