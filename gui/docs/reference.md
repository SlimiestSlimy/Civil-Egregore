# gui: reference

What each file holds. The design: `gui.md`.

## `lib.rs`

**`GuiPlugin`** `{worlds}`: the menus, a plugin -- `worlds` names the worlds
there are to open; building it takes up the numbers tuned as the
machine kept them (`build`). **`CurrentTuning`**: those numbers, a resource the
sliders set and whatever draws reads (`utilities::tuning::Tuning`).
**`MenusWorked`**: the set their work of a frame is in. **`ShownScreen`**:
`MainMenu`, until a world is made or opened from it, then `World`.
**`MakeWorld`** `{seed, tuning}`, **`OpenWorld(set_world_name)`**, **`Save(set_world_name)`**: the
messages. **`CapturedInput`** `{pointer, wheel,
keys}`: what the menus took this frame. **`offer`**: the sliders
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
**`WorldListing`** `{worlds, first_shown}`: **`from_lister(world_lister)`**, **`rows(back,
opens)`**, **`scroll(wheel)`**. **`type_into(line, typed, allowed,
most)`**: keys typed into a line; whether Enter was pressed.

## `main_menu.rs`

**`MainMenuClick`**: `DoesNothing`, `SetsUp`, `MakesWorld`, `ListsWorlds`, `GoesBack`,
`OpensWorld(place)`, `LeavesGame`. **`MainMenuPage`**: `FirstPage`, `NewWorld`, `Worlds`.
**`Part`**: marks its parts. **`MainMenu`**: the page, the seed typed,
the worlds listed -- **`listing(world_lister)`**, **`setting_up()`**: whether
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
name, the name typed, the worlds listed -- **`listing(world_lister)`**,
**`is_open()`**, **`named()`** and **`set_world_name(world_name)`**: the world run's
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
rested -- **`held()`**, **`typing_value()`**, **`pointer_over(window)`**,
**`offer_groups(to_offer)`**: a group no longer offered closed;
**`show_from_top(to_show)`**, **`listed_groups()`**, **`shown_group()`**, **`rows_under()`**,
**`shown_foot()`**, **`shown_width()`**, **`row_under(window)`**: the arithmetic
of where things are. **`group_rows(tuning_group)`**: a group's numbers in the
sliders' file's order; **`row_middle(row_number)`**. **`Part`** `{shown,
everything}`: anything of the sliders, what it is shown with, and
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
