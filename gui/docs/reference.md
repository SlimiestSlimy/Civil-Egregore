# gui: reference

What each file holds. The design: `gui.md`.

## `lib.rs`

**`Gui`** `{worlds}`: the menus, a plugin -- `worlds` names the worlds
there are to open; building it starts the numbers tuned
(`utilities::tuning::start`). **`Worked`**: the set their work of a
frame is in. **`Screen`**: `MainMenu`, until a world is made or opened
from it, then `World`. **`Make(seed)`**, **`Open(name)`**,
**`Save(name)`**: the messages. **`Captured`** `{pointer, wheel,
keys}`: what the menus took this frame. **`offer`**: the sliders
offered as the screen has it. **`capture`**: `Captured`, last.

## `rows.rs`

`ACROSS`, `HIGH`, `WORDS`: the layout; `LISTED`: the worlds listed at a
time; `ROWS`: the rows there are parts for. **`Look`**: `Said`, `Back`,
`Clicked`. **`Row<Does>`** `{says, look, does}` and **`Row::new`**.
**`Shade`**, **`Bar`**, **`Says`**: the dimmed window, a row's bar, a
row's words. **`spawn(commands, marker)`**: them, hidden, each marked
as the menu's. **`under(window, rows)`**: the row the pointer is on.
**`clicked(rows, window, buttons)`**: what the row just clicked does.
**`show(open, rows, shade, bars, words)`**: the parts as the rows say.
**`Listing`** `{worlds, first}`: **`of(lister)`**, **`rows(back,
opens)`**, **`scroll(wheel)`**. **`type_into(line, typed, allowed,
most)`**: keys typed into a line; whether Enter was pressed.

## `main_menu.rs`

**`Does`**: `Nothing`, `SetsUp`, `Makes`, `Lists`, `GoesBack`,
`Opens(place)`, `Leaves`. **`Page`**: `First`, `New`, `Worlds`.
**`Part`**: marks its parts. **`MainMenu`**: the page, the seed typed,
the worlds listed -- **`listing(lister)`**, **`setting_up()`**: whether
a new world is being set up; **`seed()`**: the seed typed, if one is;
**`rows()`**. **`spawn`**: its parts. **`work`**: a row clicked done,
unless the pointer is the sliders'; a seed typed; the worlds gone
through by the wheel; Escape back a page; the rows shown -- all while
the window shows it.

## `options.rs`

`NAME`: the most letters a world's name has. **`Does`**: `Nothing`,
`GoesOn`, `Saves`, `Names`, `Lists`, `GoesBack`, `Opens(place)`,
`Leaves`. **`Page`**: `First`, `Worlds`, `Naming`. **`Part`**: marks
its parts. **`Options`**: whether they are open, the page, the world's
name, the name typed, the worlds listed -- **`listing(lister)`**,
**`open()`**, **`named()`** and **`name(named)`**: the world run's
name, as the window tells it; **`save_named(save)`**: the name typed
saved under, unless none or taken; **`rows()`**. **`spawn`**: its
parts. **`work`**: opened and closed by Escape over a world, unless a
slider's value is being typed; a row clicked done; a name typed; the
worlds gone through by the wheel; the rows shown.

## `sliders/mod.rs`

`MARGIN`, `ROW`, `TRACK`, `KNOB`, `BOX`, `GAP`, `PANEL`, `CLOSED`,
`NOTCH`: the layout. **`Shown`**: `Closed`, `Menu` or `Group(group)`.
**`Offered`**: `Hidden`, `Everything`, `Shading`. **`Sliders`**: what
is offered and shown, how far it is scrolled, whether the left button
is held over it, the slider dragged, the value typed, where the pointer
rested -- **`held()`**, **`typing()`**, **`over(window)`**,
**`offer(offered)`**: a group no longer offered closed;
**`show(what)`**, **`listed()`**, **`page()`**, **`rows_under()`**,
**`foot()`**, **`across()`**, **`row_under(window)`**: the arithmetic
of where things are. **`rows(group)`**: a group's numbers in the
sliders' file's order; **`middle(row)`**. **`Part`** `{shown,
everything}`: anything of the sliders, what it is shown with, and
whether only while every group is offered; **`Row`**: how far down it
is. **`toggle`**: the menu by `U`, or all closed. **`scroll`**: by the
wheel, and every part shown or hidden.

## `sliders/spawn.rs`

`NAME`: the words' height; `TIP`. **`Valued`**: a value in its box;
**`Moved`**: a knob or a track's filled part; **`Tip`**: where what a
slider does is said. **`over_menus(z)`**: stacked over the menus.
**`spawn`**: the button, the menu twice -- every group, and those that
make no worlds -- and each group (**`spawn_group`**).

## `sliders/slide.rs`

**`typed(key)`**: the digit or point a key types. **`slide`**: the menu
and the groups opened and left, sliders dragged and set back, values
typed, the numbers kept; **`show`**: knobs and values as they are.

## `sliders/tell.rs`

`REST`: seconds the pointer rests before a slider is told. **`tell`**.
