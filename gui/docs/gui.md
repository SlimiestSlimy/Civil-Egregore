# gui

Civil Egregore's menus, laid over whatever window shows the world: the
main menu the window opens on, the options Escape opens over a world,
and the sliders. It knows nothing of the world or of what draws it,
and depends on Bevy's interface and on `utilities` alone. A window
adds `Gui` to its app, giving it what names the worlds there are to
open; it reads which screen is up (`Screen`), what of the pointer, the
wheel and the keys the menus took (`Captured`), and a message for each
world to be made (`Make`), opened (`Open`) or saved (`Save`). Its own
work of a frame is the set `Worked`: what is to see it the same frame
runs after.

A part a module: `main_menu`, `options`, `sliders` (itself a folder,
a part a file), and `rows`, what the main menu and the options are
both made of.

## The main menu

What the window opens on: nothing runs behind it. A row each, over the
window's middle: **new world**, **open a world**, **exit**.

**New world** is a page of its own. The seed is typed there in
hexadecimal -- none typed, one is drawn at random -- and `U` opens the
sliders, every group of them offered (below): the world's size, whether
it is forced hot, its sheep, how it is generated. **Make the world**,
or Enter, says it is to be made (`Make`), with the seed and the
sliders' numbers as they are then; the window has the server make it
(`server::Start::from_tuning`), and shows it (`Screen::World`). Escape goes
back a page.

**Open a world** lists those there are, as the options do (below); a
click on one says it is to be opened (`Open(name)`), and the window
shows it.

## Sliders

At the window's top right, the numbers of `utilities::tuning` -- held
as the resource `CurrentTuning`, nowhere else -- in groups, one group on the screen at a time. Closed, there is one small
button, `sliders`: a click on it, or `U`, opens the menu, which lists
the groups offered a row each; a click on one opens it, and the first
row of a group goes back to the menu. `U` closes whatever is open.

Which groups are offered is the screen's: every one while a new world
is set up; over a world all but the world's own, read only when a
world is made (`Group::setup_only`) -- the shading, and how a world is
generated, the world run made again from its start as one of those
changes; none over the rest of the main menu, nor under the options.

A **toggle** -- forced hot, camera loads -- is on or off: a click on
it turns it, and its box says which. Of the rest, in a group, the left button drags a knob, the right sets the number
back to its default. Beside each is a box with its value: a click on
it and the value is typed -- digits and a point, Enter to set it,
Escape to leave it. A value typed may pass the slider's range, which
is only what the knob reaches; the knob then stays at its end.

The numbers are kept whenever one is settled, and taken up again the
next run, in the machine's settings (`utilities::settings`): one file,
`settings.csv`, in a folder `Civil Egregore` where the system keeps what a
user's programs hold -- `~/.local/share/Civil Egregore` on Linux,
`%LOCALAPPDATA%\Civil Egregore` on Windows. A line a number, its name and its
value. What each is unless set is in the default settings
(`utilities/default_settings.csv`), which a machine with no file
is given a copy of. Built with `--features force_default_settings`,
the window runs on the default settings alone and keeps nothing.

- **Shading**: the near view's -- how light and dark the border lines
  are, a wall's band facing away from the sun and towards it and how
  it fades, how much longer it is a height of wall, the cast shadows,
  how much relief and how much texture. The painter reads them each
  frame.
- **World**: its side in superchunks, a square about the origin -- 0
  for no end; whether it is forced hot, every superchunk of it hot
  throughout, which only a world with a side can be; and the sheep
  each of its superchunks starts with -- every superchunk of a world
  with a side, the origin's alone of one with none, as sheep
  everywhere would keep the whole of an endless world hot.
- **Ocean and land**, **mesh lines**, **finer meshes**, **grass**,
  **trees**: how the world is generated (`worldgen::Generation::from_tuning`).

What is longer than the window is scrolled by the wheel, the pointer
over it. The pointer rested on a slider's row for a moment, and what
the slider does is said beside it (`Tuned::what`).

## Options

Escape opens the options over a world, and closes them: a row to go
on, a row to save the world, a row to open one, a row to leave Civil
Egregore. While they are open the pointer, the wheel and the keys are
theirs; the world ticks on behind them.

**Save** says the world is to be saved (`Save`) under the name the
window gave for it (`Options::name`). A world with none yet is named
first: the name typed, Enter or a click to save. A world's name is its
folder's, so only what a folder may be named is typed
(`utilities::settings::world_name`), and a name a world there is
already has is refused. Saving it is the window's.

**Open a world** lists the worlds there are -- as the window names
them: the renderer, those saved in the worlds' folder
(`utilities::settings::worlds()`) -- a row each, ten at a time, the
wheel going through the rest; its first row goes back. A click on one
closes the options and says it is to be opened (`Open`): opening it is
the window's. The list is asked for each time it is opened.

## Layout

| file | holds |
|---|---|
| `src/lib.rs` | `Gui`, the menus added to an app; `Screen`, `Captured`, the messages; `Worked` |
| `src/rows.rs` | a menu of rows over the window's middle, a click each; the worlds listed; keys typed into a line |
| `src/main_menu.rs` | what the window opens on: a new world set up, a world opened, leaving |
| `src/options.rs` | what Escape opens over a world: going on, saving it, opening another, leaving |
| `src/sliders/mod.rs` | the sliders' state and layout: what is offered and shown, scrolled, held, typed |
| `src/sliders/spawn.rs` | their parts: the button, the menus, every group's sliders |
| `src/sliders/slide.rs` | what a click on them does: groups opened, knobs dragged, values typed |
| `src/sliders/tell.rs` | what a slider does, said when the pointer rests on it |

The numbers themselves, and the sliders' file written by hand, are
`utilities`' (`src/tuning.rs`, `sliders.csv`).
