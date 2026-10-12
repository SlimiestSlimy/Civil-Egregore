# The renderer

Civil Egregore on the screen: a Bevy window showing a world run by the
host (`server::host`) on a thread of its own -- dirt brown, grass
green, trees, water, a sheep white -- on ground lit by its height
(below). Bevy draws through `wgpu`, which picks the graphics API as
the window starts -- Vulkan here, and on Linux and Windows wherever
there is a driver for it -- so no code of the renderer names one; the
line Bevy logs as it starts (its adapter and its backend) says which.

`cargo run --release` with no arguments, or `cargo run --release -p
renderer`, opens it on the main menu (`gui`): nothing runs until a
world is made there or one saved is opened. A world made is as the
sliders have it then (`gui/docs/gui.md`): its seed typed, or drawn at
random -- the first from it with land about the origin, where the sheep
start, if one is within 256 -- its size, whether it is forced hot or
its camera loads superchunks, its sheep and how it is generated.

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
asks the host for the hot superchunks of its **viewport** -- whatever
it should render, in superchunks; none in map mode -- (`Host::sync`),
and the host answers with their cells as a tick left them, and which
superchunks of the viewport are hot -- a few superchunks between two
ticks, each few a `Frame` of their own, so a wide viewport takes
little of any one tick. The frames queue here: each one that has come
is painted and laid over the images in turn, and the next ask goes
once the last of an answer has (`Frame::more`). So the window sets how
often the world is drawn; a window that falls behind slows no tick;
one ask at most is ever being answered;
and what is not in the viewport, or is cold, is never sent. "Viewport"
is the world's word here, not Bevy's: a Bevy camera's `Viewport` is a
rectangle of the window.

## Three threads

1. **The host** only copies: each hot superchunk of the viewport as
   its grass's words, as the arena holds them (128 KiB), and the cells
   its sheep stand on. What is rendered costs the ticks next to
   nothing, however much of it there is.
2. **The painter** turns cells into pixels, lit by their height,
   taking no time from the ticks or from the window's frames. The
   shading the sliders set comes to it by a queue of its own, whenever
   they change: the numbers are held by the window (`gui::CurrentTuning`)
   and the painter's own copy, never in a static.
3. **The window** shows the pixels, an image a superchunk.

The host has a second thread, its terrain's, which the painter and the
map ask for the ground no frame brings (`server/docs/server.md`, "Terrain asked
of the host"); and the map has one that waits for it (below).

What is sent is what the cells are, not the writes that changed them: a
window replaying writes would have to hold the world again and apply
every one as the arena does, and one lost would leave it wrong for
good.

## The whole world

The view is not held to the superchunks it starts on: it goes anywhere
in the world, as far out as 4,096 cells a screen pixel. A superchunk
has an image only once its pixels have come, and a cold one none: it
is black, and what was drawn of one that went cold while in the
viewport goes. The plane the images lie on is counted from the corner
of the square the view starts on, not from the world's -- the world is
2^32 cells wide, more than the plane's numbers tell apart. Boundaries,
labels and heights are laid over whatever the camera shows.

## Many superchunks

A world of 1,024 superchunks -- 32,768 cells a side -- is seen whole,
which a pixel a cell cannot do: that would be 4 GiB of pixels a frame.

- **From far off, a pixel is a tile of cells**, `2^detail` a side: as
  many as a screen pixel covers up to 4 cells, and from there coarser
  by a power of two for each further one the view goes out -- 16 cells
  a pixel at 8 a screen pixel, 64 at 16, and a chunk, 256, from 32 on
  (`frames::detail_at`): far out a superchunk is a few pixels, however
  many there are. The tile's colours mixed,
  brown and green by its grass, white by its sheep. A tile is
  a run of bits in Morton order, so its grass is counted from the words
  with no cell looked at.
- **A frame carries only so many superchunks** -- 8 drawn at a cell
  or two a pixel, 16 at four, 32 coarser (`frames::frame_holds`): a
  fine one is more to paint and to send to the graphics card -- and the
  window goes round the viewport's hot ones, frame after frame. From
  near, every hot superchunk of the viewport is in the one picture. So what a frame costs the host is the same however many
  are in the viewport, and cold ones cost nothing.
- **A fine image is dropped when its superchunk leaves the viewport**:
  it is 4 MiB here and as much on the graphics card.

A frame of one superchunk takes very little of the host's
thread, however large the world: the HUD says how much.

### Mipmaps made on the graphics card

A superchunk's picture is rarely shown pixel for pixel: between two
details the screen shows up to two of its pixels in one, and a picture
painted for a nearer view stands, far smaller, until the frame for the
farther one comes. Read one pixel in several, it shimmers as the view
moves. So each picture has **mipmaps** -- itself halved again and
again, down to one pixel -- and the graphics card blends the two
nearest the size it is drawn at.

The graphics card makes them, not the painter (`src/mipmaps.rs`). A
picture is sent with room for its mipmaps (`picture_with_mipmaps`) and
named as due (`MipmapsDue`) each time its pixels change; before the
cameras draw, the card halves it level by level in two compute passes
(Bevy's `mip_generation`, the single-pass downsampler). The painter
paints and sends exactly what it did; a third more memory on the
card. From near nothing changes: a pixel enlarged is still read sharp,
and the picture from near has no mipmaps.

Three things it rests on:

- The card writes the halvings as plain numbers, and cannot write a
  texture whose numbers are read as colours (sRGB). So the picture is
  kept as numbers and drawn through a view that reads them as colours.
  The halvings therefore average the numbers, not the light -- as the
  painter's own mixing of a tile's colours does.
- What makes the mipmaps is built by the card off the frame, and is
  not there on the first frames: a picture waits (`Waiting`) until it
  is, asked again each frame. A small picture is sent when the window
  opens, so it is built while the menu shows.
- A picture's detail is still the painter's (`frames::detail_at`): it
  bounds what the host copies and what is kept, which mipmaps do not.
  Far out, a tile of cells a pixel is still counted from the words.

`Civil_Egregore renderer window_still [seed] [cells a screen pixel]
[name]` opens the window on a world, takes what the card drew as a
PNG, and closes it: the one check of the card's own work that needs no
one at the screen (`src/diagnostics/window_still.rs`).

## Height, from straight above

The view is fully vertical, so height is shown by light and colour
alone, the sun to the top left (`src/ground.rs`, and
`src/ground/relief.rs` for what a height does to a colour, the same
from far, from near and on the map). The land as generated is level
ground and faces many heights a cell steep, with little between
(`AI_SCRATCHPAD how_steep_the_land_is`): what is drawn has to tell a
plateau from the one beside it, and a rise of 1 from one of 64. A superchunk's heights come with the first frame it is
hot in, as its image holds them; the painter asks the host only for
the cells past its edges that no frame brought (138 before, for
the shadows cast onto it, and 26 after, as far as the coast is looked
for; `server/docs/server.md`, "Terrain asked of the host") -- measured, 23 ms a
superchunk's ground where working every height out again took 90 --
once, and keeps them: the fine parts (8 MiB)
for the 48 superchunks last seen (`FINE_KEPT`), the coarse levels for
2,048 (`GROUNDS_KEPT`), past which those unseen for 256 frames go
(`UNSEEN_FRAMES`). Heights never change, so a ground kept is never
made again.

- **Slope light**: slopes facing the sun lighter, those facing away
  darker, those across it a little darker, off the heights smoothed,
  in bands 5% apart -- by how steep, on a scale that halves at each
  doubling of the rise, so gentle ground and the steepest face both
  show.
- **Height tint**: low ground dark and full, higher lighter and
  warmer, the highest pale, in 40 bands from the ocean's level to the
  highest land: two plateaus are told apart by their colour.
- **Cast shadows**: one sweep down the sun's diagonal, a shadow
  line dropping 24 heights a cell -- far more than a sun would have
  it, or steep land would be all shadow -- followed 128 cells
  (`SHADOW_REACH`) and no further -- within what is
  kept before a superchunk, so it is the same on both sides of where
  two meet; each cell keeps
  how high the shadow line stands over it, so from near a shadow's
  edge is found within the cell, with no sweep over pixels.
- **From a cell a pixel outwards**: a contour every 128 heights where
  a cell is a pixel, twice as many each time a pixel is twice as many
  cells, so they lie as far apart on the screen at every detail; every
  fifth darker; and where the ground is so steep they would lie under
  8 pixels apart, only every fifth, then every twenty-fifth.
- **The coast**: sand on the land beside water and foam on the water
  beside it, a pixel wide at every detail, from how far each cell is
  from the coast (24 cells at most); the water's light its depth's,
  light over the shallows and dark over the deep, in 14 bands, and
  the shallowest water already three quarters water's colour. Water
  is taken to be the ocean's: ground under its level. A shadow falls
  on the water, not on the ground under it.
- **From near** (`src/near.rs`), a cell 2, 4 or 8 pixels -- as many as
  the screen shows -- the viewport's cells are one picture, and height is
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

**Corners.** An edge is measured in eighths of a cell, whatever the
pixels a cell. A pixel takes one edge's doing, never two multiplied: the
darkest of the edges that darken it, and only if none does, the
lightest of those that lighten it. A cast shadow darkens an edge's
shade as it does the ground: a step's dark line lies in the shadow the
step casts, and would be lost in it otherwise. A wall met only down
a diagonal fills the corner's square, as far off as its band reaches,
joining the bands either side -- if the cells either side are no
higher and no wall is as near straight up or across; a step met only at a corner draws nothing. So bands turn corners as one
outline, with no doubled patch and no gap.

## Clouds

A cloud casts a shadow on what is under it: the ground, the water, a
tree, a sheep alike. The cloud plane holds a number a tile 8 by 8
cells, 0 for no cloud to 15 for the deepest shadow, and a frame
brings a superchunk's clouds as the arena holds them
(`Cells::clouds`, read by `cloud_over`; none sent where it has none).
The painter goes through the tiles with a cloud (`each_cloud`) and
darkens their pixels towards a dark blue (`under_cloud`), by the
cloud's share of the deepest, which leaves half of what is under it
seen: last, over everything else drawn. From far, a pixel being many
cells, it is darkened by the cloud over its cells added up; from
near, a tile's shadow is over as many pixels as its cells take.

How the clouds are laid out -- four bits, a tile 8 by 8 -- the
renderer does not say: it is the cloud plane's type in the registry
(`server::host::frame::CLOUD_BITS`, `CLOUD_TILE_SHIFT`).

### Rain

Which tiles are raining the frame says (`Cells::raining`, a bit a
tile, worked out by the server from each tile's cloud and climate):
the renderer knows no climate. After a raining tile's shadow, one cell in five of it
(`CELLS_A_STREAK_OF_RAIN`), in slanted lines the same for every tile
(`streak_of_rain_on`), is drawn half way to a pale blue (`in_rain`,
`RAIN`). From far, a pixel is as much paler as it has cells under
rain. The streaks stand still: nothing falls yet.

## Watching a test's world

`Civil_Egregore renderer watch [seed] [sheep] [pasture superchunks]
[grass, thousandths]` opens the window straight on a world a
diagnostic ticks, running, in place of the main menu
(`src/diagnostics/window_on.rs`): the world `server check` ticks from
a seed -- 0, the run's own, the seed the tests are on
(`utilities::seed::counted`) -- or, given a number of superchunks, the
plain `server pasture` ticks, started as that tool starts it
(`server::diagnostics::plain_world::pasture_start`). The host ticks
every rule of it, the weather too, at the window's pace: what a
headless run measures is seen.

## The climate, written

The K key writes on every tile of the climate -- 8 by 8 cells -- what it
holds: its humidity, its heat, and its wind, the way it blows towards
and how strongly, or that it is calm (`src/climate_labels.rs`). The
numbers are the plane's own, as the type registry lays them out. A
frame brings each superchunk's climate with its cells
(`Cells::climates`); the painter hands them on with its picture and
the window keeps the last of each (`Seen::climates`), so what is
written is the climate as the last frame of that superchunk had it,
changing as the weather changes it. As the heights are, the labels
are a grid given round to the tiles the camera shows, and are shown
only once a tile is large enough on the screen to read them.

## Showing a ticker's world

`Civil_Egregore renderer show` opens the window on no world of its
own: it shows the world of whatever test or diagnostic ticks one
while `CIVIL_EGREGORE_RENDER` is set, one after another, stays up
between them, and closes once the last has stopped and none follows
(`diagnostics/window_on.rs`, `show_worlds`;
`../../server/docs/server.md`, "A world shown"). Nobody need start it
by hand: whatever ticks a world with the variable set starts it. The
window is the same as the game's -- it is built on a link
(`window_on(link)`), and the link's host is either one that runs a
world (`Link::start`) or the worlds that come (`Link::of_shown_worlds`),
flat out until the keys pause or pace them.

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
| K | every tile's climate written on it -- humidity, heat, and the way its wind blows towards with its strength -- once a tile is 40 screen pixels across |
| `M` | map mode, or not: the map in place of the cells, at any zoom |
| `P` | on the map, the mesh's lines drawn over it, or not |

## Menus

The main menu, the sliders at the window's top right and the options
Escape opens are the `gui` crate's (`gui/docs/gui.md`), added to the
window's app. The renderer leaves the pointer, the wheel and the keys
to a menu that took them (`gui::Captured`), and tells the host what
the menus say (`src/link.rs`): a world made (`Host::make_world`), the
numbers it is made from read off the sliders then, and handed to the
server to make sense of (`server::Start::from_tuning`); one opened from
the worlds' folder (`Host::open_world`), run in place of the one run, hot in
its halos, its ticks its own, the view put over it; the world run made
again from its start (`Host::reset`) as a slider of how worlds are
generated changes, the view left where it is; the world run saved
(`Host::save_world`), on the host's thread between two ticks, under the
name it was opened by or the one typed, how it is generated with it.
What opening or saving came to is said on the first line of the text
at the top left; a world that cannot be read is refused, and the world
run goes on.

## The map

In **map mode**, which `M` turns on and off at any zoom, the cells are
no longer asked for -- the viewport is none, so the camera loads
nothing either -- and what shows is the map (`src/map.rs`), a pixel no
finer than a cell. A pixel is the cell in its middle as that cell is
generated: its height, the ocean over it darker the
deeper, or grass, dirt or a tree on it, lit by its slope and its
height. Every cell of it is asked of the host (`server/docs/server.md`, "Terrain
asked of the host"), which works it out on its terrain's thread, the
rows shared out among every thread the machine has; the renderer
holds no generator and only colours what it is answered. Nothing is
made hot to draw it and nothing of the simulation is
read, so it is how the world was generated, not how it has changed
since. A thread of its own asks for the last map wanted and waits for
it, so the window never does; a new one is asked for
when the view has moved 32 pixels, zoomed to another power of two, or
another world is run.

## Layout

| file | what is in it |
|---|---|
| `src/lib.rs` | `run`: the window, its parts and its systems |
| `src/link.rs` | the host as the window holds it: the menus' worlds made, opened and saved, paused and paced |
| `src/frames.rs` | frames asked for and shown: an image a superchunk, the picture from near |
| `src/mipmaps.rs` | the superchunks' pictures' mipmaps, made by the graphics card |
| `src/view.rs` | the plane, the camera, where it starts for a world, steering it |
| `src/overlays.rs` | boundaries, labels and heights over the world |
| `src/climate_labels.rs` | every tile's climate written on it |
| `src/hud.rs` | the text over the world |
| `src/map.rs` | the map: the world from far, as the host answers it is generated |
| `src/paint.rs` | the painter's thread: cells into pixels |
| `src/ground.rs` | the light on the ground: heights as a frame brings them, each cell's light and shadow; `ground/relief.rs` what a height does to a colour -- slope light, height tint, water's light, sand and foam; `ground/levels.rs` the ground at each detail, its contours and coast; `ground/light_and_shadow.rs` the smoothing, the shadows and how far the coast is |
| `src/near.rs` | the viewport's cells from near as one picture: steps and walls at their edges |
| `src/diagnostics/` | stills of a world at every zoom, painted with no window: `Civil_Egregore renderer stills`; and a still of the window itself, as the graphics card drew it: `renderer window_still` |
| `src/transient_data.rs` | where the stills are kept: `transient_data/renders/` |
| `docs/` | this, and the reference, function by function |
