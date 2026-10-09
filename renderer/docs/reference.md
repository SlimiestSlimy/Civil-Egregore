# The renderer, function by function

The design is in `renderer.md`.

## `paint.rs`

**`Painted`** `{at, cold, side, pixels}`: a superchunk's pixels, four bytes each.
**`Picture`** `{frame, paint_seconds, superchunks, near}`: a frame, painted -- the frame itself, its cells gone into the pixels. **`start(frames, tunings)`**: the painter's
thread, the ground made again for each world run; where pictures come. **`ground(grounds, frame, number)`**: the
ground of every hot superchunk of the frame made if missing, the fine
parts of those longest unseen dropped (`FINE_KEPT`, 48).
**`paint(cells, ground)`**: dirt, the grass over it, both lit, the
sheep over that, a pixel each (`SHEEP_REACH`, none); **`opaque`**;
`BROWN`, `GREEN`, `WHITE`: dirt's, grass's and a sheep's colours. `WATER`, `FILM`, **`depth_at(cells, word, bit)`**, **`under_water(colour,
depth)`**: a colour seen through water, the less the deeper, none from
`server::host::frame::DEEP` (16). `TREE_YOUNG`, `TREE_OLD`, **`tree_colour(stage)`**, **`stage_at(cells,
word, bit)`**; **`counted(words, detail)`**: the cells set a tile.
**`paint_far(cells, detail, ground)`**: a pixel a tile of cells
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
**`generate(seed, shape, top_left)`**; **`coarsen()`**: the fine parts dropped.
**`heights`**, **`smoothed`**, **`shadow_lines`**: the steps of making
it. **`Sun`**: **`shade(across, down)`**, the light on a slope.
**`banded`**, **`tint`**. **`Level`**: the ground at one detail --
**`halved()`**, **`drawn(detail)`** with cliffs and contours.
**`lit(colour, factor)`**: a colour in a factor's light.

## `near.rs`

**`PaintedNear`** `{near, pixels}`. **`Edge`** `{towards, rise}`: a
neighbour of another height; **`shading(from, span)`**: how dark and
how light it makes a pixel that far in from it. **`paint_near(cells,
grounds, near, tuning)`**: the picture, shaded as `tuning` says. **`Cell`**: **`paint`**, a cell's pixels
-- its edges, the shadow on it, its ground's tone (`TONES`).
**`tree`**: a tree on its cell, a square larger and darker the older.
**`sheep`**: a sheep's shape (`SHEEP`) on its cell.

## `lib.rs`

**`run()`**: the window opened on the main menu, run until it is
closed: the menus (`gui::Gui`), the host's and the map's links, every
part made at startup, and each frame the menus' worlds told the host,
then -- only over a world -- the view steered, the overlays laid, the
frame shown and the next asked for, the map; then the text.

## `link.rs`

**`Link`** `{host, pictures, shading, waiting, since, asked, paused,
pace}`: the host as the window holds it (`server::host::Host`) --
**`start()`**: the host and the painter started; **`forget_asked()`**:
another world to run in place of the one run, no frame of the one
before waited for. **`Seen`** `{frame, painted, paint_seconds,
in_view, detail, near_pixels, map}`: the last frame, and how it was
drawn. **`menus`**: the worlds the menus make (`server::Start::from_tuning`
of the seed and numbers they give), open and save, the host told; the
options told the world run's name. **`keys`**: pause and pace, the
host told. **`shading`**: the sliders' numbers (`gui::CurrentTuning`)
sent to the painter whenever they change.

## `frames.rs`

`SYNC_EVERY` (a sixtieth of a second), `COARSEST` (6), `KEPT_SIDE`
(64), `NEAR_SCALE` (half a cell a screen pixel), `NEAR_PIXELS` (8),
`NEAR_MARGIN` (8 cells), `TILES_KEPT` (4,096).
**`frame_holds(detail)`**: superchunks a frame carries at most.
`DIRT`, one pixel of it; **`picture_of(size, pixels)`**: an image.
**`Laid`**: what of a picture over the images is changed.
**`NearView`**: the picture from near; **`spawn`**: it, hidden.
**`show`**: the frame that came shown -- another world's dropping what
was drawn of the last, the view put over it; each superchunk's image,
and the picture from near. **`near(first, last, scale)`**: the cells
seen from near, if the view is near. **`ask`**: the next frame asked
for -- the superchunks the camera sees, how coarsely, on round them
from the last, and from near the cells in view; fine images out of
view dropped.

## `view.rs`

`SPRITE_SIDE`, `PAN_SPEED`, `ZOOM_SPEED`, `WHEEL_ZOOM`, `FARTHEST` (32
cells a screen pixel), `MAP_FARTHEST` (4,096), `UNLIMITED_SEEN` (3).
**`Sprites`** `{tiles}`: a sprite and its image a superchunk that has
been in view, by where it is in the world; **`origin(axis)`**: the
plane's origin, the origin superchunk's top left;
**`plane(cell, axis)`** and **`cell(plane, axis)`** between the world's
cells and the plane, **`in_view(transform, scale, window)`**.
**`spawn`**: the camera. **`first_view(side, window, transform,
projection)`**: the view over the whole of a world of a side, or the
origin's halo. **`steer`**: the view moved and zoomed, unless the
menus took the keys, the wheel or the pointer. **`fullscreen`**: by
`F11`.

## `overlays.rs`

`LINES_FROM`, `LINES`, `SUPERCHUNK_LINE`, `CHUNK_LINE`.
**`Boundary`**: a line between chunks or superchunks; **`Shown`**: which
of the overlays are shown; **`toggle`**: by `B`, `C` and `H`.
**`boundaries`**: as wide on the screen however near.
**`Label`**: one of `LABELS` (256) texts; **`labels`**: the superchunks
and chunks in view named in their top left corners, from
`LABELLED_FROM` (150) screen pixels across, smaller where there is
less room than `LABEL_WIDTH` (420), a chunk's a line (`LABEL_LINE`)
below. **`HeightLabel`**: one of a grid of `HEIGHT_LABELS` (96 by 54)
texts; **`heights`**: every cell's height written on it, as the world
run is generated, from `HEIGHT_FROM` (20) screen pixels a cell.
**`spawn`**: them all, hidden.

## `hud.rs`

**`Hud`**: the text; **`spawn`**. **`grouped(number)`**: its digits in
threes. `KEYS`. **`hud`**: what the last frame said written, hidden
over the main menu.

## `map.rs`

`MARGIN` (64 pixels), `DEEP_LIGHT`, `BORDER_LIGHT`, `SLOPE_LIGHT`.
**`Wanted`** `{first, step, size, seed, generation, borders}`: a map
asked for; **`Drawn`**: one drawn. **`start()`**: the map's thread.
**`cell(wanted, x, y)`**: the cell in a pixel's middle;
**`draw(wanted)`**: its pixels, rows shared among the machine's threads,
what grows on a cell as `worldgen::Growth` says. **`MapLink`**: the
window's side of the thread -- **`start()`**. **`MapView`**: its
picture; **`spawn`**. **`map_step(scale)`**. **`far`**: from farther
than `FARTHEST`, a map of the world run asked for and laid where it is
of; `P` draws the mesh's lines.
