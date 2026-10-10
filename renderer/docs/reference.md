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

`SIDE`, `SHADOW_DROP` (24 heights a cell down the diagonal),
`LIGHT_BAND`, `COAST_REACH` (24 cells), `SMOOTHED_OVER` (4), `BEFORE`
(138) and `AFTER` (26) cells kept about a superchunk, `SHADOW`,
`COARSEST` (8). **`shadow_drop()`**.
**`Shade`**: what a pixel's colour is drawn through -- what it is
multiplied by, and what is laid over it first, pale, sand or foam and
how much. **`lit(colour, shade)`**: a colour through a shade.

**`Fine`**: heights, shadow lines and light, a cell each, the ocean's
level and the highest land -- **`height(x, y)`**, **`line(x, y)`**,
**`light(x, y)`**, **`under_ocean(x, y)`**, **`surface(x, y)`**: what
a shadow falls on, **`deepest()`**, **`tint(x, y)`**. **`share`**.
**`Ground`** `{levels, fine, used}`: a level of shades a detail;
**`generate(seed, generation, top_left, given)`**; **`coarsen()`**: the
fine parts dropped. **`heights`**: the heights about a superchunk.

### `ground/relief.rs`

What a height does to a colour, wherever the world is drawn.
**`slope_light(across, down)`**: the light on ground rising so many
heights a cell (`STEEPEST`, `GENTLEST`, `TOWARDS_SUN`, `AWAY_FROM_SUN`,
`ACROSS_SUN`). **`tint(share)`**: what a colour is multiplied by, and
how much of it is `PALE`, that far from the ocean's level to the
highest land (`TINTS`, `TINT_BANDS`). **`water_light(depth,
deepest)`** (`SHALLOWEST`, `DEEPEST`, `SHALLOWS`, `WATER_BANDS`).
`SAND`, `FOAM`, `SAND_MOST`, `FOAM_MOST`; **`tint_on_sand(tint,
sand)`**; **`laid(from, to, part)`**.

### `ground/levels.rs`

**`Level`**: the ground at one detail -- each pixel's factor, its
light untinted, how pale, its height, the share of it under the ocean,
how far its nearest cell is from the coast -- **`halved()`**,
**`drawn(detail)`**: contours (`CONTOUR`, `FIFTH_CONTOUR`,
`CONTOUR_EVERY`, `CONTOURS_APART`), sand and foam.
**`Shade::of(factor, over, part)`**.

### `ground/light_and_shadow.rs`

**`smoothed(heights)`**, **`shadow_lines(heights)`** (`SHADOW_REACH`),
**`coast_distances(under)`**: cells from each cell to the coast,
**`banded(light, step)`**.

## `near.rs`

**`PaintedNear`** `{near, pixels}`. **`Edge`** `{towards, rise}`: a
neighbour of another height; **`shading(from, span)`**: how dark and
how light it makes a pixel that far in from it. **`paint_near(cells,
grounds, near, tuning)`**: the picture, shaded as `tuning` says. **`Cell`**: **`paint`**, a cell's pixels
-- its edges, the shadow on it, its ground's tone (`TONES`), its
height's tint, sand or foam beside the coast, its water's light.
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
pace, generation, first_view}`: the host as the window holds it (`server::host::Host`) --
**`start()`**: the host and the painter started; **`forget_asked()`**:
another world to run in place of the one run, no frame of the one
before waited for. **`Seen`** `{frame, painted, paint_seconds,
viewport_superchunks, detail, near_pixels, map}`: the last frame, and how it was
drawn. **`menus`**: the worlds the menus make (`server::Start::from_tuning`
of the seed and numbers they give), open and save, the host told; the
options told the world run's name. **`keys`**: pause and pace, the
host told. **`shading`**: the sliders' numbers (`gui::CurrentTuning`)
sent to the painter whenever they change. **`generation`**: the host
asked to make the world run again (`Host::reset`) whenever the sliders
change how worlds are generated, the view left where it is.

## `frames.rs`

`SYNC_EVERY` (a sixtieth of a second), `COARSEST` (8), `COARSER_FROM`
(2), `KEPT_SIDE`
(64), `NEAR_SCALE` (half a cell a screen pixel), `NEAR_PIXELS` (8),
`NEAR_MARGIN` (8 cells), `TILES_KEPT` (4,096).
**`frame_holds(detail)`**: superchunks a frame carries at most.
`DIRT`, one pixel of it; **`picture_of(size, pixels)`**: an image.
**`Laid`**: what of a picture over the images is changed.
**`NearView`**: the picture from near; **`spawn`**: it, hidden.
**`show`**: every frame that came shown, in the order they came, the
host asked again once one says no more follow -- another world's dropping what
was drawn of the last, the view put over it; what was drawn of a
superchunk of the viewport gone cold dropped; each superchunk's image,
and the picture from near. **`near(first, last, scale)`**: the cells
seen from near, if the view is near. **`detail_at(scale)`**: how
coarsely the world is drawn, coarser than the screen past
`COARSER_FROM`. **`ask`**: the next frame asked for -- the viewport,
none in map mode, how coarsely, on round its hot superchunks from the
last, and from near its cells; fine images out of the viewport
dropped.

## `view.rs`

`SPRITE_SIDE`, `PAN_SPEED`, `ZOOM_SPEED`, `WHEEL_ZOOM`, `FIRST_FARTHEST`
(32 cells a screen pixel, the farthest a world is first seen from),
`FARTHEST` (4,096), `UNLIMITED_SEEN` (3).
**`Sprites`** `{tiles}`: a sprite and its image a superchunk that has
been in the viewport, by where it is in the world; **`origin(axis)`**: the
plane's origin, the origin superchunk's top left;
**`plane(cell, axis)`** and **`cell(plane, axis)`** between the world's
cells and the plane, **`viewport_cells(transform, scale, window)`**:
the cells the camera shows.
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
and chunks the camera shows named in their top left corners, from
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
what grows on a cell as the server's `terrain_seen::CoverSeen` says. **`MapLink`**: the
window's side of the thread -- **`start()`**, **`map_mode()`**.
**`MapView`**: its picture; **`spawn`**. **`map_step(scale)`**, a cell
at the finest. **`far`**: in map mode, which `M` turns on and off, a
map of the world run asked for and laid where it is of; `P` draws the
mesh's lines.

## `diagnostics/`

**`stills::Still`** `{name, size, pixels}`. **`stills::gather(seed,
offset, farthest, keep)`**: one place of a world at every zoom -- the
map at 64 and 16 cells a pixel, the cells from `farthest` cells a pixel
to one, the cells from near at 2, 4 and 8 pixels a cell -- painted by
the map's and the painter's own code from a host asked as the window
asks it, each 1,024 by 768 as a screen would show it; **`answered`**,
**`viewport_of`**. **`tool::COMMANDS`**: `stills`, each kept as a PNG.

## `transient_data.rs`

`TRANSIENT_DATA`; **`renders()`**: where stills are kept.
