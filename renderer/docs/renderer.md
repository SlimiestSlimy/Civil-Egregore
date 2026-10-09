# The renderer

Civil Egregore on the screen: a Bevy window showing a world run by the
host (`server::host`) on a thread of its own -- dirt brown, grass
green, trees, water, a sheep white -- on ground lit by its height
(below).

`cargo run --release` with no arguments, or `cargo run --release -p
renderer`, opens it on the main menu (`gui`): nothing runs until a
world is made there or one saved is opened. A world made is as the
sliders have it then (`gui/docs/gui.md`): its seed typed, or drawn at
random -- the first from it with land about the origin, where the sheep
start, if one is within 256 -- its size, whether it is forced hot, its
sheep and how it is generated.

A world with a size is a square of superchunks about the origin, with
sheep on every one; hot in the halos about them, or, forced, all of it
hot all the while, whatever its sheep come to: the world under a fixed
load, to be measured. A world with none has sheep on the origin alone.
The view starts over the whole of a world with a size, and over the
origin's halo of one without; the cold superchunks are black.

It runs until it is closed: long runs are watched, not waited for. As a
world runs the host keeps a census -- the flock and the grass every
1,000 ticks, the seconds and the pace held, 0 flat out, in
`server/transient_data/measurements/census.csv` -- so a run closed at
any time leaves what it came to. And the text says what a frame costs:
the time it takes of the host's thread, and of the painter's.

## The window asks

Bevy is the window, the drawing and the keys, and nothing else: the
world is not in its entities, and the host knows nothing of it.
The two share two queues and no memory.

The window is the one that asks, never the host that sends: each
time the window has shown a frame -- 60 times a second at most -- it
sends the host the superchunks in view (`Request::Sync`), and the
host, between two ticks, answers with their cells as the last
tick left them (a `Frame`). So the window sets how often the world is
drawn; a window that falls behind slows no tick; one frame at most is
ever on its way; and what is not in view is never sent.

## Three threads

1. **The host** only copies: each superchunk in view as its
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
  So what a frame costs the host is the same however many are in
  view; the whole view is drawn afresh in half a second at most.
- **A fine image is dropped when its superchunk leaves the view**: it
  is 4 MiB here and as much on the graphics card.

A frame of one superchunk takes very little of the host's
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
| `F11` | the window over the whole screen, or not |
| `U` | the sliders' menu, opened; whatever of them is open, closed |
| Escape | the options, opened or closed: going on, saving the world, opening one, leaving Civil Egregore |
| `[` and `]` | halve and double the pace |
| `B` | show the superchunks' boundaries, or not; and once a superchunk is 150 screen pixels across, its Morton index (as its save file is named) and `(x, y)` in its top left corner |
| `C` | the same of the chunks, their labels a line below |
| `H` | every cell's height written on it, once a cell is 20 screen pixels across |
| `P` | on the map, the mesh's lines drawn over it, or not |

## Menus

The main menu, the sliders at the window's top right and the options
Escape opens are the `gui` crate's (`gui/docs/gui.md`), added to the
window's app. The renderer leaves the pointer, the wheel and the keys
to a menu that took them (`gui::Captured`), and tells the host what
the menus say (`src/link.rs`): a world made (`Request::New`), the
numbers it is made from read off the sliders then; one opened from the
worlds' folder (`Request::Open`), run in place of the one run, hot in
its halos, its ticks its own, the view put over it; the world run saved
(`Request::Save`), on the host's thread between two ticks, under the
name it was opened by or the one typed, how it is generated with it.
What opening or saving came to is said on the first line of the text
at the top left; a world that cannot be read is refused, and the world
run goes on.

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
another world is run.

## Layout

| file | what is in it |
|---|---|
| `src/lib.rs` | `run`: the window, its parts and its systems |
| `src/link.rs` | the host as the window holds it: the menus' worlds made, opened and saved, paused and paced |
| `src/frames.rs` | frames asked for and shown: an image a superchunk, the picture from near |
| `src/view.rs` | the plane, the camera, where it starts for a world, steering it |
| `src/overlays.rs` | boundaries, labels and heights over the world |
| `src/hud.rs` | the text over the world |
| `src/map.rs` | the map: the world from far, drawn from the generator alone |
| `src/paint.rs` | the painter's thread: cells into pixels |
| `src/ground.rs` | the light on the ground: heights as a frame brings them, hillshade, tint, cast shadows, cliffs and contours |
| `src/near.rs` | the cells in view from near as one picture: steps and walls at their edges |
| `docs/` | this, and the reference, function by function |
