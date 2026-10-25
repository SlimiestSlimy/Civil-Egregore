# The client, function by function

> **DEPRECATED** -- out of date, kept until the docs rework at 0.8: not a source of names or designs. Names: `newserver/GLOSSARY.md`.

The designs: `renderer.md` (`src/renderer/`), `gui.md` (`src/gui/`).

# The renderer (`src/renderer/`)

## `paint.rs`

**`PaintedSuperchunk`** `{superchunk_cartesian, side_pixels, rgba_bytes}`: a superchunk's pixels, four bytes each.
**`PaintedFrame`** `{host_frame, paint_seconds, superchunks, painted_near, climates}`: a frame, painted -- the frame itself, its cells gone into the pixels. **`start_painter(frame_receiver, tunings, terrain)`**: the painter's
thread, the ground made again for each world run; where pictures come. **`make_grounds(kept_grounds, terrain, host_frame, frame_number)`**: the
ground of every hot superchunk of the frame made if missing, the fine
parts of those longest unseen dropped (`FINE_KEPT`, 48), and past
`GROUNDS_KEPT` (2,048) grounds those unseen `UNSEEN_FRAMES` (256)
frames. `chunk_top_left(place)`: how far across and down from its
superchunk's top left a chunk starts, in cells.
**`paint_superchunk(cells, superchunk_ground)`**: dirt, the grass over it, both lit, the
sheep over that, a pixel each (`SHEEP_REACH`, none; `SUPERCHUNK_SIDE_PIXELS`, a cell a pixel); **`opaque_rgba`**;
`DIRT_BROWN`, `GRASS_GREEN`, `SHEEP_WHITE`: dirt's, grass's and a sheep's colours. `WATER`, `WATER_FILM_HIDES`, **`depth_at(cells, word, bit_in_word)`**, **`under_water(colour,
depth)`**: a colour seen through water, the less the deeper, none from
`server::host::frame::DEEP_WATER_DEPTH` (16). `CLOUD_SHADOW`, `SEEN_UNDER_DEEPEST_CLOUD`, **`under_cloud(seen_colour, cloud, whole_cloud)`**: a colour in a cloud's shadow; **`each_cloud(cells, on_cloud_tile)`**: every tile with a cloud, its top left cell, its side and its cloud (`renderer.md`, "Clouds"). `RAIN`, `CELLS_A_STREAK_OF_RAIN`, **`streak_of_rain_on(x, y)`**, **`in_rain(seen_colour, rain, whole_rain)`**: rain drawn under a cloud deep enough ("Rain"). `TREE_YOUNG`, `TREE_OLD`, **`tree_colour(stage)`**, **`stage_at(cells,
word, bit)`**; **`counted_in_tiles(words, detail_shift)`**: the cells set a tile.
**`paint_far(cells, detail_shift, superchunk_ground)`**: a pixel a tile of cells
`2^detail_shift` a side, its grass counted from its run of bits, its colours
**`mixed_colour`** and lit.

## `ground.rs`

`SUPERCHUNK_CELLS_A_SIDE`, `SHADOW_DROP` (24 heights a cell down the diagonal),
`LIGHT_BAND`, `COAST_REACH` (24 cells), `SMOOTHED_OVER` (4), `CELLS_KEPT_BEFORE`
(138) and `CELLS_KEPT_AFTER` (26) cells kept about a superchunk, `CAST_SHADOW_FACTORS`,
`COARSEST_DETAIL` (8), `FACTOR_ONE` (128: a colour's factor is a byte, and
this is one); `WORKED_SIDE_CELLS`, cells along the side of the heights worked on,
those kept before and after with the superchunk's; `KEPT_MARGIN_CELLS` (8) and
`KEPT_SIDE_CELLS`, what is kept of them past each side, as far as a wall's band
is looked for; `LIT_ONE` (80), flat ground's light in the seven bits a
cell's is kept in, and `SHADOWED_BIT`, the eighth, saying a shadow falls
on it; `FINE_LEVELS` (2), the coarsest of the levels dropped with the
fine parts; `PixelShade::PART_BITS` (6) and `PixelShade::WHOLE_LAID`, how much of
what is laid over a colour. **`shadow_drop()`**. **`GivenHeights`**: heights already worked
out, a superchunk's height words by its top left cell, so `ask_of_host`
asks for none twice.
**`PixelShade`**: what a pixel's colour is drawn through -- what it is
multiplied by, and what is laid over it first, pale, sand or foam and
how much. **`lit_colour(seen_colour, pixel_shade)`**: a colour through a shade.

**`FineGround`**: heights, shadow lines and light, a cell each, the ocean's
level and the highest land -- **`height(x, y)`**, **`shadow_line(x, y)`**,
**`cell_light(x, y)`**, **`under_ocean(x, y)`**, **`shadow_surface(x, y)`**: what
a shadow falls on, **`deepest_ocean_depth()`**, **`cell_tint(x, y)`**. **`height_share`**.
`offset_of(x, y)`: where a cell is kept, `KEPT_MARGIN_CELLS` cells past each
edge.
**`SuperchunkGround`** `{shades_by_detail, fine_ground, last_used_frame}`: a level of shades a detail;
**`ask_of_host(terrain, world, height_levels, top_left, given_heights)`**: a superchunk's
ground, the heights about it asked of the host, flat at the ocean's
level if the host runs another world by then; `of_heights`: it, of
heights had; **`drop_fine_parts()`**: the
fine parts dropped. **`heights(terrain, world, top_left, given_heights)`**: the heights about a superchunk,
those `given_heights` laid over what the host answers of the rest.

### `ground/relief.rs`

What a height does to a colour, wherever the world is drawn.
**`slope_light(rise_across, rise_down)`**: the light on ground rising so many
heights a cell (`STEEPEST_RISE`, `GENTLEST_RISE`, `TOWARDS_SUN`, `AWAY_FROM_SUN`,
`ACROSS_SUN`). **`height_tint(height_share)`**: what a colour is multiplied by, and
how much of it is `PALE_COLOUR`, that far from the ocean's level to the
highest land (`HEIGHT_TINTS`, `TINT_BANDS`). **`water_light(water_depth,
deepest_depth)`** (`SHALLOWEST_FACTORS`, `DEEPEST_FACTORS`, `SHALLOWS_DEPTH`, `WATER_BANDS`).
`SAND_COLOUR`, `FOAM_COLOUR`, `SAND_MOST`, `FOAM_MOST`; **`tint_on_sand(land_tint,
sand_share)`**; **`laid_over(under_colour, over_colour, part)`**.

### `ground/levels.rs`

**`GroundAtDetail`**: the ground at one detail -- each pixel's factor, its
light untinted, how pale, its height, the share of it under the ocean,
how far its nearest cell is from the coast -- **`twice_as_coarse()`**,
**`drawn_at_detail(detail_shift)`**: contours (`CONTOUR_DARKENING`, `FIFTH_CONTOUR`,
`CONTOUR_EVERY`, `CONTOURS_APART`; `DARKER_CONTOUR_EVERY`, 5, contours from one drawn
darker to the next), sand and foam.
**`PixelShade::from_factor(colour_factor, laid_colour, part)`**.

### `ground/light_and_shadow.rs`

**`box_smoothed(heights)`**, **`shadow_lines(heights)`** (`SHADOW_REACH`),
**`coast_distances(under_ocean)`**: cells from each cell to the coast,
**`banded_light(unbanded_light, step)`**.

## `near.rs`

`EIGHTHS_A_CELL` (8): a cell's side in eighths, what edges are measured in;
`AROUND`: the eight cells about a cell.

**`PaintedNear`** `{near_asked, rgba_bytes}`. **`CellEdge`** `{towards_neighbour, neighbour_rise}`: a
neighbour of another height; **`pixel_shading(from_eighths, span_eighths)`**: how dark and
how light it makes a pixel that far in from it. **`paint_near(cells,
grounds, near, tuning)`**: the picture, shaded as `tuning` says (**`wall_band_eighths(wall_rise, tuning)`**: how far in a wall's band reaches). **`Cell`**: **`paint_cell`**, a cell's pixels
-- its edges, the shadow on it, its ground's tone (`PIXEL_TONES`), its
height's tint, sand or foam beside the coast, its water's light.
**`tree`**: a tree on its cell, a square larger and darker the older.
**`paint_sheep`**: a sheep's shape (`SHEEP_SHAPE`) on its cell.

## `renderer.rs`

**`run_window()`**: the window opened on the main menu, run until it is
closed; `window()`: it, not yet run, on a host of its own; `window_on(host_link)`: on whatever host the link reaches -- the menus (`gui::GuiPlugin`), the host's and the map's links, every
part made at startup, and each frame the menus' worlds told the host,
then -- only over a world -- the view steered, the overlays laid, the
frame shown and the next asked for, the map; then the text.

## `mipmaps.rs`

`renderer.md`, "Mipmaps made on the graphics card".
**`picture_with_mipmaps(side_pixels, rgba_bytes)`**: a superchunk's picture with
room for its mipmaps -- kept as numbers the card may write, drawn
through a view reading them as colours, sharp enlarged and blended
made smaller. **`MipmapsDue`**: the pictures whose pixels changed
this frame. **`Mipmaps`**: the plugin. `begin_frame_mipmaps`: a frame begun with
none due but, the first time, a small picture (`WarmUp`,
`WARM_UP_SIDE` 4) that has the card build what makes mipmaps. On the
card's side: `MipmapsWaiting` `{waiting_pictures, asked_on}`, the pictures not yet
done; `take_due_pictures`: the frame's due taken, every one waiting asked for;
`make_mipmaps`: the mipmaps made before the cameras draw
(`BEFORE_THE_CAMERAS`), a picture done once the card has it and what
makes them is built -- not taken as built before mipmaps were asked on
`ASKED_BEFORE` (2) frames -- and given up after `GIVEN_UP_AFTER` (600).

## `link.rs`

**`HostLink`** `{host, painted_pictures, shading_sender, awaiting_frame, seconds_since_asked, last_ask, world_paused,
held_pace, last_generation, first_view}`: the host as the window holds it (`server::host::Host`) --
**`start_own_host()`**: the host and the painter started; **`of_shown_worlds()`**: the same on a host whose world is whatever ticker's comes, flat out (`around_host`, what both are made by); **`forget_asked()`**:
another world to run in place of the one run, no frame of the one
before waited for. **`LastFrameSeen`** `{last_frame, painted_superchunks, paint_seconds,
viewport_superchunks, detail_shift, near_pixels, map, climates}`: the last frame, and how it was
drawn. **`tell_host_of_menus`**: the worlds the menus make (`server::WorldStart::from_tuning`
of the seed and numbers they give), open and save, the host told; the
options told the world run's name. **`pause_and_pace_by_keys`**: pause and pace, the
host told. **`send_shading`**: the sliders' numbers (`gui::CurrentTuning`)
sent to the painter whenever they change. **`remake_on_generation_change`**: the host
asked to make the world run again (`Host::reset`) whenever the sliders
change how worlds are generated, the view left where it is.

## `frames.rs`

`SYNC_EVERY` (a sixtieth of a second), `COARSEST_DETAIL` (8), `COARSER_FROM`
(2), `KEPT_SIDE`
(64), `NEAR_SCALE` (half a cell a screen pixel), `NEAR_PIXELS` (8),
`NEAR_MARGIN` (8 cells), `TILES_KEPT` (4,096).
**`frame_holds(detail_shift)`**: superchunks a frame carries at most.
`DIRT`, one pixel of it; **`picture_of(size_pixels, rgba_bytes)`**: an image
with no mipmaps -- the picture from near, the map, a still.
**`LaidPictureParts`**: what of a picture over the images is changed.
**`NearView`**: the picture from near; **`spawn_near_view`**: it, hidden.
**`show_frames`**: every frame that came shown, in the order they came, the
host asked again once one says no more follow -- another world's dropping what
was drawn of the last, the view put over it; what was drawn of a
superchunk of the viewport gone cold dropped; each superchunk's image,
and the picture from near; each superchunk's image named as due its
mipmaps. **`near_of_cells(first_cell_cartesian, last_cell_cartesian, scale)`**: the cells
seen from near, if the view is near. **`detail_at(scale)`**: how
coarsely the world is drawn, coarser than the screen past
`COARSER_FROM`. **`ask_next_frame`**: the next frame asked for -- the viewport,
none in map mode, how coarsely, on round its hot superchunks from the
last, and from near its cells; fine images out of the viewport
dropped.

## `view.rs`

`SPRITE_SIDE`, `PAN_SPEED`, `ZOOM_SPEED`, `WHEEL_ZOOM`, `FIRST_FARTHEST`
(32 cells a screen pixel, the farthest a world is first seen from),
`FARTHEST_CELLS_A_PIXEL` (4,096), `UNLIMITED_SEEN` (3).
**`SuperchunkSprites`** `{tiles}`: a sprite and its image a superchunk that has
been in the viewport, by where it is in the world; **`origin(cartesian_axis)`**: the
plane's origin, the origin superchunk's top left;
**`translation_of_cell(cell, cartesian_axis)`** and **`cell_at_translation(translation_on_plane, cartesian_axis)`** between the world's
cells and the plane, **`viewport_cells(camera_transform, scale, window)`**:
the cells the camera shows.
**`spawn_camera`**: the camera. **`first_view(side, window, transform,
projection)`**: the view over the whole of a world of a side, or the
origin's halo. **`steer_view`**: the view moved and zoomed, unless the
menus took the keys, the wheel or the pointer. **`toggle_fullscreen`**: by
`F11`.

## `climate_labels.rs`

Every tile's climate written on it (`renderer.md`, "The climate,
written"). `TILE_SIDE`, `CLIMATE_LABELS` (the grid of labels),
`CLIMATE_FROM`, `CLIMATE_WIDTH`; **`ClimateLabel`**
(`slot`), `CameraOnly`; **`spawn_climate_labels`**: the labels, hidden; `climate_written(climate)`: a
tile's text; **`climates`**: gives each label the tile the camera
shows that is its own, from `LastFrameSeen::climates`.

## `overlays.rs`

`LINES_FROM`, `LINES`, `SUPERCHUNK_LINE`, `CHUNK_LINE`; `HEIGHT_WIDTH`
(80 screen pixels a height written at its full size, five digits);
`CameraOnly`, the camera and none of the overlays' parts.
**`DrawnBoundary`**: a line between chunks or superchunks; **`OverlaysShown`**: which
of the overlays are shown; **`toggle`**: by `B`, `C`, `H` and the K key.
**`lay_boundaries`**: as wide on the screen however near.
**`CornerLabel`**: one of `CORNER_LABEL_COUNT` (256) texts; **`label_corners`**: the superchunks
and chunks the camera shows named in their top left corners, from
`LABELLED_FROM` (150) screen pixels across, smaller where there is
less room than `LABEL_WIDTH` (420), a chunk's a line (`LABEL_LINE`)
below. **`HeightLabel`**: one of a grid of `HEIGHT_LABELS` (96 by 54)
texts; `HeightsAsked`: the heights last asked of the host for them; **`heights`**: every cell's height written on it, as the host
answers the world run is generated, asked again only when the view
shows other cells, from `HEIGHT_FROM` (20) screen pixels a cell.
**`spawn_overlays`**: them all, hidden.

## `hud.rs`

**`HudText`**: the text; **`spawn_hud`**. **`grouped_in_threes(whole_number)`**: its digits in
threes. `KEYS_TEXT`. **`write_hud`**: what the last frame said written, hidden
over the main menu.

## `map.rs`

`MAP_MARGIN_PIXELS` (64 pixels), `BORDER_LIGHT`; what a height does to a colour
is `ground/relief.rs`'s.
**`WantedMap`** `{first_cell_cartesian, step, size_pixels, world, mesh_lines_drawn}`: a map
asked for; **`DrawnMap`**: one drawn. **`start_map_thread(terrain)`**: the map's thread.
**`draw_map(terrain, wanted_map)`**: its pixels, each cell asked of the host
(`server::host::terrain::MapAsk`) and coloured by its height and its
`CellCover`, none if the host runs another world. **`MapLink`**: the
window's side of the thread -- **`start_map_link(terrain)`**, **`map_mode()`**.
**`MapView`**: its picture; **`spawn_map_view`**. **`map_step(scale)`**, a cell
at the finest. **`show_map`**: in map mode, which `M` turns on and off, a
map of the world run asked for and laid where it is of; `P` draws the
mesh's lines.

## `diagnostics/`

**`stills::StillPicture`** `{zoom_name, size_pixels, rgba_bytes}`. **`stills::gather_stills(seed,
cells_east_south, farthest_cells_a_pixel, keep_still)`**: one place of a world at every zoom -- the
map at 64 and 16 cells a pixel, the cells from `farthest_cells_a_pixel` cells a pixel
to one, the cells from near at 2, 4 and 8 pixels a cell -- painted by
the map's and the painter's own code from a host asked as the window
asks it, each 1,024 by 768 as a screen would show it; **`pictures_answered`**,
**`viewport_of`**. Its constants: `STILL_SIZE_PIXELS`, `MAP_STEPS`, the stills'
`NEAR_PIXELS` and `NEAR_MARGIN`, `SHEEP_A_SUPERCHUNK` (250),
`WAITED_AT_MOST` (half an hour: a world's superchunks take a while to
generate) and `BETWEEN_ASKS` (200 ms before the host is asked again
for superchunks not yet hot). **`tool::COMMANDS`**: `stills`, each kept
as a PNG (`stills_tool`); `watch` opens the window on a world an in-bin test ticks (`watch_tool`; `SHEEP_WATCHED`, `PASTURE_SUPERCHUNKS`, `GRASS_THOUSANDTHS`): `show` opens the window that shows tickers' worlds (**`window_on::show_worlds()`**, `show_world`); **`window_on::open_window(world_start)`** tells the host to make the world and shows it in place of the main menu -- `WatchedStart`, `make_watched_world` (`renderer.md`, "Watching a test's world"); the tools' parameters' names `SEED` (in hex; 0 the
counted one, moved on to one with land about the middle),
`CELLS_EAST`, `CELLS_SOUTH`, `CELLS_A_PIXEL_FARTHEST`, `NAMED_AFTER`; and `window_still`
(`window_still_tool`; `CELLS_A_PIXEL`): **`window_still::save_still(seed,
cells_a_pixel, path)`** opens the window on the world of a seed, holds
the view at so many cells a screen pixel, keeps what the graphics card
drew and closes it -- `WantedStill` `{seed, cells_a_pixel, path,
shown_for}`, `take_still` each frame, the still taken `SETTLED_AFTER` (300)
frames after the world is first shown and the window closed
`WRITTEN_AFTER` (90) later.

## `transient_data.rs`

`TRANSIENT_DATA`; **`renders_folder()`**: where stills are kept.


# The menus (`src/gui/`)

What each file holds.

## `gui.rs`

**`GuiPlugin`** `{worlds}`: the menus, a plugin -- `worlds` names the worlds
there are to open; building it takes up the numbers tuned as the
machine kept them (`build`). **`CurrentTuning`**: those numbers, a resource the
sliders set and whatever draws reads (`utilities::tuning::Tuning`).
**`MenusWorked`**: the set their work of a frame is in. **`ShownScreen`**:
`MainMenu`, until a world is made or opened from it, then `World`.
**`MakeWorld`** `{seed, tuning}`, **`OpenWorld(set_world_name)`**, **`Save(set_world_name)`**: the
messages. **`CapturedInput`** `{pointer_buttons, wheel,
keyboard_keys}`: what the menus took this frame. **`offer_sliders`**: the sliders
offered as the screen has it. **`capture_input`**: `CapturedInput`, last.

## `rows.rs`

`ROW_WIDTH_PIXELS`, `ROW_HEIGHT_PIXELS`, `WORDS`: the layout; `WORLDS_LISTED_AT_ONCE`: the worlds listed at a
time; `MOST_ROWS`: the rows there are parts for. **`RowLook`**: `OnlySays`, `GoesBack`,
`ActsOnClick`. **`MenuRow<Does>`** `{row_text, row_look, click_does}` and **`MenuRow::new`**.
**`MenuShade`**, **`Bar`**, **`RowText`**: the dimmed window, a row's bar, a
row's words. **`spawn_menu_parts(bevy_commands, part_marker)`**: them, hidden, each marked
as the menu's. **`row_under_pointer(window, row_count)`**: the row the pointer is on.
**`clicked_row_does(menu_rows, window, mouse_buttons)`**: what the row just clicked does.
**`show_rows(menu_open, menu_rows, shade_visibility, bars, words)`**: the parts as the rows say.
**`WorldListing`** `{worlds, first_shown}`: **`from_lister(world_lister)`**, **`world_rows(back_does,
opens_world)`**, **`scroll_by_wheel(wheel)`**. **`type_into(line, typed_keys, letter_allowed,
most_letters)`**: keys typed into a line; whether Enter was pressed.

## `main_menu.rs`

**`MainMenuClick`**: `DoesNothing`, `SetsUp`, `MakesWorld`, `ListsWorlds`, `GoesBack`,
`OpensWorld(place)`, `LeavesGame`. **`MainMenuPage`**: `FirstPage`, `NewWorld`, `Worlds`.
**`Part`**: marks its parts. **`MainMenu`**: the page, the seed typed,
the worlds listed -- **`with_world_lister(world_lister)`**, **`setting_up()`**: whether
a new world is being set up; **`seed()`**: the seed typed, if one is;
**`shown_rows()`**. **`spawn_main_menu`**: its parts. **`update_main_menu`**: a row clicked done,
unless the pointer is the sliders'; a seed typed; the worlds gone
through by the wheel; Escape back a page; the rows shown -- all while
the window shows it.

## `options.rs`

`MOST_NAME_LETTERS`: the most letters a world's name has. **`OptionsClick`**: `DoesNothing`,
`GoesOn`, `Saves`, `SavesNamed`, `ListsWorlds`, `GoesBack`, `OpensWorld(place)`,
`LeavesGame`. **`OptionsPage`**: `FirstPage`, `Worlds`, `NamingWorld`. **`Part`**: marks
its parts. **`Options`**: whether they are open, the page, the world's
name, the name typed, the worlds listed -- **`with_world_lister(world_lister)`**,
**`is_open()`**, **`current_world_name()`** and **`set_world_name(world_name)`**: the world run's
name, as the window tells it; **`save_named(save)`**: the name typed
saved under, unless none or taken; **`shown_rows()`**. **`spawn_options`**: its
parts. **`update_options`**: opened and closed by Escape over a world, unless a
slider's value is being typed; a row clicked done; a name typed; the
worlds gone through by the wheel; the rows shown.

## `sliders/mod.rs`

`SLIDERS_MARGIN_PIXELS`, `ROW_STEP_PIXELS`, `TRACK_SIZE_PIXELS`, `KNOB_SIZE_PIXELS`, `VALUE_BOX_SIZE_PIXELS`, `GAP`, `PANEL_WIDTH_PIXELS`, `CLOSED_BUTTON_WIDTH_PIXELS`,
`NOTCH_SCROLL_PIXELS`: the layout. **`SlidersShown`**: `ClosedButton`, `GroupMenu` or `OneGroup(group)`.
**`SlidersOffered`**: `NoneOffered`, `EveryGroup`, `RunningGroups`. **`Sliders`**: what
is offered and shown, how far it is scrolled, whether the left button
is held over it, the slider dragged, the value typed, where the pointer
rested -- **`holds_left_button()`**, **`typing_value()`**, **`pointer_over(window)`**,
**`offer_groups(to_offer)`**: a group no longer offered closed;
**`show_from_top(to_show)`**, **`listed_groups()`**, **`shown_group()`**, **`rows_under()`**,
**`shown_foot()`**, **`shown_width()`**, **`row_under(window)`**: the arithmetic
of where things are. **`group_rows(tuning_group)`**: a group's numbers in the
sliders' file's order; **`row_middle(row_number)`**. **`Part`** `{shown_with,
every_group_only}`: anything of the sliders, what it is shown with, and
whether only while every group is offered; **`PartTop`**: how far down it
is. **`toggle`**: the menu by `U`, or all closed. **`scroll_sliders`**: by the
wheel, and every part shown or hidden.

## `sliders/spawn.rs`

`NAME_HEIGHT_PIXELS`: the words' height; `TIP_WIDTH_PIXELS`. **`ValueInBox`**: a value in its box;
**`MovesWithNumber`**: a knob or a track's filled part; **`SliderTip`**: where what a
slider does is said. **`over_menus(stack_above)`**: stacked over the menus.
**`spawn_sliders`**: the button, the menu twice -- every group, and all but those
only of setting a world up -- and each group (**`spawn_group`**).

## `sliders/slide.rs`

**`typed_digit(pressed_key)`**: the digit or point a key types, off `DIGIT_KEYS`, each
digit's key and its key on the number pad. `turned_on(slider_index, value)`:
whether a toggle is on -- nearer its range's far end than its near
one. **`work_slider_clicks`**: the menu
and the groups opened and left, sliders dragged and set back, values
typed -- into `CurrentTuning` -- the numbers kept; **`update_slider_visuals`**: knobs
and values as they are.

## `sliders/tell.rs`

`REST_SECONDS`: seconds the pointer rests before a slider is told. **`tell_what_slider_does`**.
