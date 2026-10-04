# TileSim, function by function

The `tilesim` crate is the program: its commands (`src/commands.rs`),
and `src/main.rs`, which reads the command line and prints what they
say. Everything they run is a crate beside it, each with a `docs/` of its own: the world
made, ticked, saved and loaded (`world/`), the rules of the cells
(`mc_rules/`), the entities (`entity_rules/`). What TileSim is and every
decision about it: `tilesim.md`.

## `commands.rs`

Each command is given the rest of the command line after its folder,
and gives the line to print, or why it could not.
`tilesim new <folder> [name] [seed] [superchunks]`: a world
generated from the seed (**`new`**, `world::generate`) and saved in the
folder, which must not hold one. `tilesim run <folder> [ticks]`:
it loaded, ticked and saved again (**`run`**). `tilesim info
<folder>`: what its world file says (**`info`**). **`number`**: an
argument, or its default. **`USAGE`**: how to call the program.

## `main.rs`

**`main`**: the command the command line names, its line printed;
anything else prints `USAGE`.

## `transient_data.rs`

**`TRANSIENT_DATA`**; **`worlds()`**: where the tests' worlds go.
`diagnostics/` gathers nothing yet.
