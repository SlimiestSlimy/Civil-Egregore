# The renderer

TileSim on the screen: a pasture ticking on a thread of its own, and a
Bevy window showing it -- dirt brown, grass green, a sheep white -- on
ground lit by its height (below). The world is a generated one
(`world::generate_flocks`, seed 1).

`cargo run --release -p renderer -- [superchunks shown] [sheep a superchunk] [ticks a second, 0 flat out] [ticks to watch for] [1 to force hot]`;
64 superchunks shown, 8,000 sheep on each and 256 ticks a second if
not said: a flock that reaches the most the grass feeds, some 16,000 a
superchunk, within 70,000 ticks, and has not eaten it bare before.

The world is a generated one (`world::generate_flocks`, a flock on
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

Measured, 1,024 superchunks with 1,000 sheep each, flat out on 12
threads: 150 ticks a second, 2.3 GiB held, 45 seconds to make the mock
world; a frame of one superchunk 111 us of the simulation's thread.

## Height, from straight above

The view is fully vertical, so height is shown by light alone, the sun
to the top left and 35 degrees up; a cell is 2 m and a height 1 m
(`src/ground.rs`). The painter works a superchunk's heights out itself,
from the seed -- the simulation is asked for none -- once, on a thread
a superchunk, and keeps them: the coarse levels for good, the fine
parts (8 MiB) for the 48 superchunks last seen.

- **Hillshade**: slopes facing the sun lighter, those facing away
  darker, off the heights smoothed, in bands 7% apart.
- **Tint**: high ground a little lighter than low.
- **Cast shadows**: one sweep down the sun's diagonal; each cell keeps
  how high the shadow line stands over it, so from near a shadow's
  edge is found within the cell, with no sweep over pixels.
- **From a cell a pixel outwards**: cliffs darkened by the walls in the
  pixel, and a contour every 8, 16 or 32 heights.
- **From near** (`src/near.rs`), a cell 2, 4 or 8 pixels -- as many as
  the screen shows -- the cells in view are one picture, and height is
  drawn at the edges: wherever a cell is higher than the one beside
  it, a thin line along the higher cell's border, light towards the
  sun and dark away; and under a wall a band 5 to 7 eighths of a cell on its lower
  cell, darkest at its foot and fading from it, tuned apart for a
  wall the sun is on and one facing away. Walls are drawn much
  the stronger: they are what cannot be crossed.

**Corners.** A pixel takes one edge's doing, never two multiplied: the
darkest of the edges that darken it, and only if none does, the
lightest of those that lighten it. A cast shadow darkens an edge's
shade as it does the ground: a step's dark line lies in the shadow the
step casts, and would be lost in it otherwise. A cell higher only at a
corner fills that corner's square if it is a wall there, joining the
bands either side; a step met only at a corner draws nothing. So bands turn corners as one
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
| `U` | the next page of sliders, or none |
| `[` and `]` | halve and double the pace |
| `B` | show the superchunks' boundaries, or not; and once a superchunk is 150 screen pixels across, its Morton index (as its save file is named) and `(x, y)` in its top left corner |
| `C` | the same of the chunks, their labels a line below |
| `H` | every cell's height written on it, once a cell is 20 screen pixels across |

## Sliders

At the window's top right, a page of sliders at a time, each a number
of `src/tuning.rs`; `U` goes to the next page, and to none. The left
button drags a knob, the right sets the number back to its default.
Beside each is a box with its value: a click on it and the value is
typed -- digits and a point, Enter to set it, Escape to leave it. A
value typed may pass the slider's range, which is only what the knob
reaches; the knob then stays at its end. The
numbers are kept whenever one is settled, in
`transient_data/tuning.txt`, and taken up again the next run -- what is
found by eye is then written into the code as the defaults.

- **Shading**: the near view's -- how light and dark the border lines
  are, a wall's band facing away from the sun and towards it and how
  it fades, the cast shadows, how much relief and how much texture.
  The painter reads them each frame. As tuned: the border lines 35%
  lighter and darker, a wall's band 49% darker at its foot facing away
  from the sun and 55% with the sun on it, fading by 80% across it,
  cast shadows 40% darker, 70% of the relief, and twice the texture.
- **Generation**, in the lab only: below.

## The lab

`cargo run --release -p renderer -- lab [superchunks shown]`, 64 (8 by
8) if not said, is the renderer run to tune by eye how the world is
made and how it changes (`src/lab.rs`). The world is of the superchunks
shown and no more: every one hot from the start and kept so, no sheep,
its rules -- grass and trees -- ticking as in any run, paused and paced
by the same keys. Generation's sliders are shown from the start:

- **The heights**: how much of a height each of the four octaves makes
  up beside the others -- hills 512 cells across, ridges 128, bumps 32,
  roughness 8 -- and the height span they come to together, 255 at
  most (`terrain::Shape`).
- **The grass**, and **the trees** (`world::patches`): each in patches,
  not scattered cell by cell. A cell's number is smooth noise as broad
  as a patch, finer noise on it (detail), and a lot drawn for the cell
  alone (scatter); it has the thing under a threshold found, by looking
  at 16,384 cells, so that the share asked for (cover) do.
- **The button** draws a new seed off the clock; the seed is shown in
  the text.

Whenever a slider of generation moves or the seed is drawn, the world
is made afresh from nothing but the sliders, and its ticks start again
from 0. What is settled here is written into `world::Generation::DEFAULT`
and `terrain::Shape::DEFAULT`, which every world is made with.

## Layout

| folder | what is in it |
|---|---|
| `src/sim.rs` | the simulation's thread: requests read between ticks, the cells in view copied when asked |
| `src/paint.rs` | the painter's thread: cells into pixels |
| `src/ground.rs` | the light on the ground: heights from the seed, hillshade, tint, cast shadows, cliffs and contours |
| `src/tuning.rs` | the numbers the near view's shading is tuned by, kept between runs |
| `src/sliders.rs` | the sliders that set them, their value boxes and the button |
| `src/lab.rs` | the lab: the seed and how the world is generated, as the sliders have it |
| `src/near.rs` | the cells in view from near as one picture: steps and walls at their edges |
| `src/main.rs` | the window: the camera, an image a superchunk, the keys, the text |
| `docs/` | this, and the reference, function by function |
