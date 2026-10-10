# The renderer, function by function

The design is in `renderer.md`.

## `paint.rs`

**`Painted`** `{at, cold, side, pixels}`: a superchunk's pixels, four bytes each.
**`Picture`** `{frame, paint_seconds, superchunks, near}`: a frame, painted -- the frame itself, its cells gone into the pixels. **`start(frames, tunings, terrain)`**: the painter's
thread, the ground made again for each world run; where pictures come. **`ground(grounds, terrain, frame, number)`**: the
ground of every hot superchunk of the frame made if missing, the fine
parts of those longest unseen dropped (`FINE_KEPT`, 48), and past
`GROUNDS_KEPT` (2,048) grounds those unseen `UNSEEN_FRAMES` (256)
frames. `chunk_top_left(place)`: how far across and down from its
superchunk's top left a chunk starts, in cells.
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
`COARSEST` (8), `FACTOR_ONE` (128: a colour's factor is a byte, and
this is one); `WIDE`, cells along the side of the heights worked on,
those kept before and after with the superchunk's; `MARGIN` (8) and
`KEPT`, what is kept of them past each side, as far as a wall's band
is looked for; `LIT_ONE` (80), flat ground's light in the seven bits a
cell's is kept in, and `SHADOWED`, the eighth, saying a shadow falls
on it; `FINE_LEVELS` (2), the coarsest of the levels dropped with the
fine parts; `Shade::PART_BITS` (6) and `Shade::WHOLE`, how much of
what is laid over a colour. **`shadow_drop()`**. **`Given`**: heights already worked
out, a superchunk's height words by its top left cell, so `ask`
asks for none twice.
**`Shade`**: what a pixel's colour is drawn through -- what it is
multiplied by, and what is laid over it first, pale, sand or foam and
how much. **`lit(colour, shade)`**: a colour through a shade.

**`Fine`**: heights, shadow lines and light, a cell each, the ocean's
level and the highest land -- **`height(x, y)`**, **`line(x, y)`**,
**`light(x, y)`**, **`under_ocean(x, y)`**, **`surface(x, y)`**: what
a shadow falls on, **`deepest()`**, **`tint(x, y)`**. **`share`**.
**`Ground`** `{levels, fine, used}`: a level of shades a detail;
**`ask(terrain, world, levels, top_left, given)`**: a superchunk's
ground, the heights about it asked of the host, flat at the ocean's
level if the host runs another world by then; `of_heights`: it, of
heights had; **`coarsen()`**: the
fine parts dropped. **`heights(terrain, world, top_left, given)`**: the heights about a superchunk,
those `given` laid over what the host answers of the rest.

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
`CONTOUR_EVERY`, `CONTOURS_APART`; `FIFTH`, 5, contours from one drawn
darker to the next), sand and foam.
**`Shade::of(factor, over, part)`**.

### `ground/light_and_shadow.rs`

**`smoothed(heights)`**, **`shadow_lines(heights)`** (`SHADOW_REACH`),
**`coast_distances(under)`**: cells from each cell to the coast,
**`banded(light, step)`**.

## `near.rs`

`EIGHTHS` (8): a cell's side in eighths, what edges are measured in;
`AROUND`: the eight cells about a cell.

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
closed; `window()`: it, not yet run -- the menus (`gui::Gui`), the host's and the map's links, every
part made at startup, and each frame the menus' worlds told the host,
then -- only over a world -- the view steered, the overlays laid, the
frame shown and the next asked for, the map; then the text.

## `mipmaps.rs`

`renderer.md`, "Mipmaps made on the graphics card".
**`picture_with_mipmaps(side, pixels)`**: a superchunk's picture with
room for its mipmaps -- kept as numbers the card may write, drawn
through a view reading them as colours, sharp enlarged and blended
made smaller. **`MipmapsDue`**: the pictures whose pixels changed
this frame. **`Mipmaps`**: the plugin. `begin`: a frame begun with
none due but, the first time, a small picture (`WarmUp`,
`WARM_UP_SIDE` 4) that has the card build what makes mipmaps. On the
card's side: `Waiting` `{pictures, asked_on}`, the pictures not yet
done; `take`: the frame's due taken, every one waiting asked for;
`make`: the mipmaps made before the cameras draw
(`BEFORE_THE_CAMERAS`), a picture done once the card has it and what
makes them is built -- not taken as built before mipmaps were asked on
`ASKED_BEFORE` (2) frames -- and given up after `GIVEN_UP_AFTER` (600).

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
`DIRT`, one pixel of it; **`picture_of(size, pixels)`**: an image
with no mipmaps -- the picture from near, the map, a still.
**`Laid`**: what of a picture over the images is changed.
**`NearView`**: the picture from near; **`spawn`**: it, hidden.
**`show`**: every frame that came shown, in the order they came, the
host asked again once one says no more follow -- another world's dropping what
was drawn of the last, the view put over it; what was drawn of a
superchunk of the viewport gone cold dropped; each superchunk's image,
and the picture from near; each superchunk's image named as due its
mipmaps. **`near(first, last, scale)`**: the cells
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

`LINES_FROM`, `LINES`, `SUPERCHUNK_LINE`, `CHUNK_LINE`; `HEIGHT_WIDTH`
(80 screen pixels a height written at its full size, five digits);
`CameraOnly`, the camera and none of the overlays' parts.
**`Boundary`**: a line between chunks or superchunks; **`Shown`**: which
of the overlays are shown; **`toggle`**: by `B`, `C` and `H`.
**`boundaries`**: as wide on the screen however near.
**`Label`**: one of `LABELS` (256) texts; **`labels`**: the superchunks
and chunks the camera shows named in their top left corners, from
`LABELLED_FROM` (150) screen pixels across, smaller where there is
less room than `LABEL_WIDTH` (420), a chunk's a line (`LABEL_LINE`)
below. **`HeightLabel`**: one of a grid of `HEIGHT_LABELS` (96 by 54)
texts; `HeightsAsked`: the heights last asked of the host for them; **`heights`**: every cell's height written on it, as the host
answers the world run is generated, asked again only when the view
shows other cells, from `HEIGHT_FROM` (20) screen pixels a cell.
**`spawn`**: them all, hidden.

## `hud.rs`

**`Hud`**: the text; **`spawn`**. **`grouped(number)`**: its digits in
threes. `KEYS`. **`hud`**: what the last frame said written, hidden
over the main menu.

## `map.rs`

`MARGIN` (64 pixels), `BORDER_LIGHT`; what a height does to a colour
is `ground/relief.rs`'s.
**`Wanted`** `{first, step, size, world, borders}`: a map
asked for; **`Drawn`**: one drawn. **`start(terrain)`**: the map's thread.
**`draw(terrain, wanted)`**: its pixels, each cell asked of the host
(`server::host::terrain::MapAsk`) and coloured by its height and its
`Cover`, none if the host runs another world. **`MapLink`**: the
window's side of the thread -- **`start(terrain)`**, **`map_mode()`**.
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
**`viewport_of`**. Its constants: `SIZE`, `MAP_STEPS`, the stills'
`NEAR_PIXELS` and `NEAR_MARGIN`, `SHEEP_A_SUPERCHUNK` (250),
`WAITED_AT_MOST` (half an hour: a world's superchunks take a while to
generate) and `BETWEEN_ASKS` (200 ms before the host is asked again
for superchunks not yet hot). **`tool::COMMANDS`**: `stills`, each kept
as a PNG (`stills_tool`); its parameters' names `SEED` (in hex; 0 the
counted one, moved on to one with land about the middle),
`CELLS_EAST`, `CELLS_SOUTH`, `FARTHEST`, `NAMED`; and `window_still`
(`window_still_tool`; `CELLS_A_PIXEL`): **`window_still::keep(seed,
cells_a_pixel, path)`** opens the window on the world of a seed, holds
the view at so many cells a screen pixel, keeps what the graphics card
drew and closes it -- `Wanted` `{seed, cells_a_pixel, path,
shown_for}`, `take` each frame, the still taken `SETTLED_AFTER` (300)
frames after the world is first shown and the window closed
`WRITTEN_AFTER` (90) later.

## `transient_data.rs`

`TRANSIENT_DATA`; **`renders()`**: where stills are kept.
