# gui

TileSim's menus, laid over whatever window shows the world: the
sliders, the numbers they tune, and the options Escape opens. It knows
nothing of the world or of what draws it, and depends on Bevy's
interface and on `utilities` alone. A window adds `Gui` to its app,
giving it what names the worlds there are to open; it reads the
numbers (`tuning::now()`), the seed drawn (`tuning::seed_drawn()`),
whether a menu has the pointer (`sliders::over`, `sliders::held`,
`Options::open`), and a message for each world chosen
(`options::Chosen`). Its own work of a frame is the set `Worked`: what
is to see it the same frame runs after.

## Sliders

At the window's top right, the numbers of `src/tuning.rs`, in groups,
one group on the screen at a time. Closed, there is one small button,
`sliders`: a click on it, or `U`, opens the menu, which lists the
groups a row each; a click on one opens it, and the first row of a
group goes back to the menu. `U` closes whatever is open.

In a group, the left button drags a knob, the right sets the number
back to its default. Beside each is a box with its value: a click on
it and the value is typed -- digits and a point, Enter to set it,
Escape to leave it. A value typed may pass the slider's range, which
is only what the knob reaches; the knob then stays at its end.

The numbers are kept whenever one is settled, and taken up again the
next run, in the machine's settings (`utilities::settings`): one file,
`settings.txt`, in a folder `tilesim` where the system keeps what a
user's programs hold -- `~/.local/share/tilesim` on Linux,
`%LOCALAPPDATA%\tilesim` on Windows. A line a number, its name and its
value. What each is unless set is in the default settings
(`utilities/default_settings.txt`), which a machine with no file
is given a copy of. Built with `--features default_settings`, the
renderer runs on the default settings alone and keeps nothing.

- **Shading**: the near view's -- how light and dark the border lines
  are, a wall's band facing away from the sun and towards it and how
  it fades, how much longer it is a height of wall, the cast shadows, how much relief and how much texture.
  The painter reads them each frame. As tuned: the border lines 35%
  lighter and darker, a wall's band 49% darker at its foot facing away
  from the sun and 55% with the sun on it, fading by 80% across it,
  cast shadows 40% darker, 70% of the relief, and twice the texture.
- **Ocean and land**, **mesh lines**, **finer meshes**, **grass**,
  **trees**, **sheep**: how the world is generated, listed in the lab
  only, where they are read: below. **Sheep** is how many each
  superchunk the lab shows starts with -- none, unless set.

What is longer than the window is scrolled by the wheel, the pointer
over it. The pointer rested on a slider's row for a moment, and what the slider
does is said beside it (`Tuned::what`).

## Two files written by hand

Neither is made by a program. `sliders.txt`, here at the crate's root:
a line a slider -- its number's name, the least and the most its knob
reaches, its group, and what it does; a group's sliders are shown in
their lines' order. `utilities/default_settings.txt`:
what each number is unless set. Both are built into the program;
`src/tuning.rs` has only the numbers' names and places, and a test
holds the two files to them. Lines are found by name, in whatever
order they come: the places are how the code reads a number, nothing
more.

## Options

Escape opens the options over the window's middle, and closes them: a
row to go on, a row to save the world, a row to open one, a row to
leave TileSim. The keys are theirs while they are open. While
they are open the view is not dragged or zoomed by the pointer; the
world ticks on behind them.

**Save** says the world is to be saved (`options::Save`) under the
name the window gave for it (`Options::name`). A world with none yet
is named first: the name typed, Enter or a click to save. A world's
name is its folder's, so only what a folder may be named is typed
(`utilities::settings::world_name`), and a name a world there is
already has is refused. Saving it is the window's.

**Open a world** lists the worlds there are -- as the window names
them: the renderer, those saved in the worlds' folder
(`utilities::settings::worlds()`) -- a row each, ten at a time, the
wheel going through the rest; its first row goes back. A click on one
closes the options and says it was chosen: opening it is the window's.
The list is asked for each time it is opened.

## Layout

| file | holds |
|---|---|
| `src/lib.rs` | `Gui`, the menus added to an app; `Worked` |
| `sliders.txt` | the sliders, written by hand: each one's range, group and what it does |
| `src/tuning.rs` | the numbers tuned, by name and place, kept between runs; the seed drawn |
| `src/sliders.rs` | the sliders that set them, their value boxes and the button |
| `src/options.rs` | the options Escape opens: going on, saving the world, opening one, leaving |
