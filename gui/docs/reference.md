# gui: reference

What each file holds. The design: `gui.md`.

## `lib.rs`

**`Gui`** `{worlds}`: the menus, a plugin -- `worlds` names the worlds
there are to open. **`Worked`**: the set their work of a frame is in.

## `tuning.rs`

`NAMES`: the numbers' names, each at its place, and the places
(`STEP_LIGHT` ... `SHEEP`). **`Tuned`** `{name, line, range, group, what}`
-- `what` the tooltip -- and **`tuned(index)`**: a number as the
sliders' file (`sliders.csv`, at the crate's root) has it.
**`Group`**: `Shading`, `Land`, `Lines`, `Finer`, `Grass`, `Trees`,
`Sheep` (`GROUPS`, in the menu's order), **`name()`** and
**`lab_only()`** -- every group but shading.
**`Tuning`**: the numbers read together. **`unless_set(index)`**: what a number is
unless set, from the default settings. **`start()`**: what the
machine's settings have. **`now()`**, **`set(index, value)`**,
**`keep()`**: the numbers written to the settings, the others' lines
dropped. **`revision()`**: how many times how the world is generated
has changed -- not a generation itself (`server::Generation`, the
recipe), only a count of when one last changed; **`revise()`**: says
it has. **`reseed()`**: a new seed drawn off the clock, the world to
be generated again; **`seed_drawn()`**: it, 0 while none was.

## `sliders.rs`

`MARGIN`, `ROW`, `TRACK`, `KNOB`, `BOX`, `GAP`, `PANEL`, `NAME`,
`CLOSED`: the layout. **`Shown`**: `Closed`, `Menu` or `Group(group)`
-- what is on the screen, **`shown()`** and **`show(what)`**.
**`Part`**: anything of the sliders, and what it is shown with;
**`Moved`**: a knob or a track's filled part; **`Valued`**: a value in
its box. **`Hands`**: the slider dragged and the value being typed.
**`page()`**: the group shown; **`rows(group)`**; **`listed()`**: the
groups the menu has -- lab-only ones only in the lab; **`rows_under()`**, **`middle(row)`**,
**`foot()`**, **`across()`**, **`over(window)`**,
**`row_under(window)`**: the arithmetic of where things are. **`Row`**,
a part that scrolls (**`scroll`**, which also shows the parts of what
is shown and hides the rest); **`Tip`** and **`tell`**: what a slider
does, said when the pointer rests on it. **`in_lab()`**: every group
listed, the menu open from the start. **`held()`**: whether the left
button went down over the sliders and is still held -- the view is then
not dragged. **`setup`**: the button, the menu and the groups.
**`toggle`**: the menu by `U`, or all closed. **`typed(key)`**: the
digit or point a key types. **`slide`**: the menu and the groups
opened and left, sliders dragged and set back, values typed, the lab's
button pressed, the numbers kept and shown.

## `options.rs`

`ACROSS`, `ROW`, `WORDS`: the layout; `LISTED`: the worlds listed at a
time; `ROWS`: the rows there are parts for. **`Does`**: what a click
on a row does -- `Nothing`, `GoesOn`, `Lists`, `GoesBack`,
`Saves`, `Names`, `Opens(place)`, `Leaves`. **`Chosen(name)`**: the
message for a world chosen; **`Save(name)`**: for the world to be
saved. **`Page`**: `First`, `Worlds`, `Naming`; `NAME`: the most
letters a name has. **`Options`**: whether they are open, the page shown, the
worlds listed and the first shown -- **`listing(lister)`**,
**`open()`**, **`named()`** and **`name(named)`**: the world run's
name, as the window tells it; **`save_named(save)`**: the name typed
saved under, unless none or taken; **`rows()`**. **`Shade`**, **`Bar`**, **`Says`**: the
dimmed window, a row's bar, a row's words. **`setup`**: them, hidden.
**`row_under(window, rows)`**: the row the pointer is on. **`work`**:
opened and closed by Escape, unless it is leaving a value being typed;
a row clicked done; the worlds gone through by the wheel; the rows
shown as they are.
