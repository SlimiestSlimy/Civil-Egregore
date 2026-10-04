# The renderer, function by function

The design is in `renderer.md`.

## `sim.rs`

`TARGET_PACE` (256 ticks a second), `SEED` (1), `CHUNK_WORDS`, `CENSUS_EVERY`
(1,000 ticks). **`census_path()`**: where the run's census is kept;
**`census`**: its file, started afresh.

**`Viewport`** `{first, last}`: the superchunks in view, a rectangle of
them counted from the world's top left, both corners in it.
**`Ask`** `{viewport, detail, skip, most, near}`: what a frame is to carry --
some of the superchunks in view, and how coarsely they will be drawn.
**`Near`** `{first, size, pixels_a_cell}`: the cells seen from near.
**`Request`**: `Sync(ask)`, `Pause(bool)`, `Pace(ticks a second, or
flat out)`. **`Cells`** `{at, hot, top_left, grass, sheep}`: a superchunk's grass
(**`layer`**), its 16 chunks' words one after another, its sheep's
cells, and where it is in the world.
**`Frame`** `{tick, ticks_a_second, sheep, grass, sync_seconds,
sync_share, detail, near, cells}`: with what answering took of the thread.

**`shown(superchunks)`**: the superchunks shown, a square about the
origin. **`forced(shown, flock)`**: a world with every one of them hot.
**`start(superchunks, flock, forced_hot)`**: the pasture on a thread
of its own, ticking on every thread the machine has; where to send
requests, where frames come back. **`run`**: that thread -- every
request waiting read, each sync answered, a tick, and a sleep to the
next one's time if paced; paused, it waits for a request.
**`copy(world, superchunks, ask)`**: the superchunks asked for
(**`grass`**, **`sheep`**).

## `paint.rs`

**`Painted`** `{at, side, pixels}`: a superchunk's pixels, four bytes each.
**`Picture`**: a frame, painted. **`start(frames)`**: the painter's
thread; where pictures come. **`ground(grounds, frame, number)`**: the
ground of every hot superchunk of the frame made if missing, the fine
parts of those longest unseen dropped (`FINE_KEPT`, 48).
**`paint(cells, ground)`**: dirt, the grass over it, both lit, the
sheep over that, a pixel each (`SHEEP_REACH`, none); **`opaque`**; `COLD`,
black. **`paint_far(cells, detail, ground)`**: a pixel a tile of cells
`2^detail` a side, its grass counted from its run of bits, its colours
**`mixed`** and lit.

## `ground.rs`

`SIDE`, `CELL_METRES` (2), `HEIGHT_METRES` (1), `SUN_ELEVATION` (35),
`SMOOTHED_OVER` (4), `BEFORE` (138) and `AFTER` (10) cells kept about a
superchunk, `SHADOW`, `CLIFF`, `CONTOUR`, `COARSEST` (6).
**`shadow_drop()`**: heights the shadow line drops a cell down the
diagonal. **`contour_every(detail)`**.

**`Fine`**: heights, shadow lines and light, a cell each --
**`height(x, y)`**, **`line(x, y)`**, **`light(x, y)`**.
**`Ground`** `{levels, fine, used}`: a level of factors a detail;
**`generate(seed, top_left)`**; **`coarsen()`**: the fine parts dropped.
**`heights`**, **`smoothed`**, **`shadow_lines`**: the steps of making
it. **`Sun`**: **`shade(across, down)`**, the light on a slope.
**`banded`**, **`tint`**. **`Level`**: the ground at one detail --
**`halved()`**, **`drawn(detail)`** with cliffs and contours.
**`lit(colour, factor)`**: a colour in a factor's light.

## `near.rs`

**`PaintedNear`** `{near, pixels}`. **`Edge`** `{towards, rise}`: a
neighbour of another height; **`shading(from, span)`**: how dark and
how light it makes a pixel that far in from it. **`paint_near(cells,
grounds, near)`**: the picture. **`Cell`**: **`paint`**, a cell's pixels
-- its edges, the shadow on it, its ground's tone (`TONES`).
**`sheep`**: a sheep's shape (`SHEEP`) on its cell.

## `main.rs`

`SPRITE_SIDE`, `PAN_SPEED`, `ZOOM_SPEED`, `WHEEL_ZOOM`, `SYNC_EVERY`
(a sixtieth of a second), `COARSEST` (6), `KEPT_SIDE` (64), `NEAR_SCALE`
(half a cell a screen pixel), `NEAR_PIXELS` (8), `NEAR_MARGIN` (8 cells).
**`frame_holds(detail)`**: superchunks a frame carries at most.

**`Link`**: the requests' sender, the pictures' receiver, whether a frame
is awaited, and the pause and pace last sent. **`Sprites`**: a sprite
and its image a superchunk. **`Seen`**: what the last frame said. **`NearView`**: the picture from
near's sprite. **`Hud`**: the text.

**`grouped(number)`**: its digits in threes. **`setup`**: the camera over the world's middle, the whole of it in
view; an image a superchunk, dirt until the first frame; the text.
**`steer`**: the view moved and zoomed. **`keys`**: pause and pace sent.
**`picture(side, pixels)`**, **`picture_of(size, pixels)`**: an image;
`DIRT`, one pixel of it. **`show`**: the frame that came shown -- each
superchunk's image, and the picture from near. **`ask`**: the next
asked for -- the superchunks the camera sees, how coarsely, on round
them from the last, and from near the cells in view; fine images out
of view dropped. **`hud`**: the text written.
