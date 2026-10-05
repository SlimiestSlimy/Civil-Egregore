# Instructions: reference

What a rule is made of: small pieces of behaviour, each asked on a
superchunk's turn, built on the public parts of `simulation`,
`worldgen` and `pathfinding`. A rule of the cells or of an entity puts
a few together.

## `walking.rs`

What an entity that walks asks, of a turn: the simulation's cells, the
terrain's walls and `pathfinding` met here. **`around_unwalled(turn,
at)`**: the 3x3 cells about `at` no wall is before, nine bits.
**`area_walls(turn, centre)`**: the area's walls, for paths.
**`step_towards(turn, at, goals, passable)`**, **`step_to(turn, at, to,
passable)`**: the cell to step to for the nearest goal, or for one
cell, round the entities in the way. **`seek(turn, at, type)`**: the
step to the nearest cell the type holds at, the area first, then tiles
by scale, `FARTHEST_SCALE` first and the finest that reach after -- a
**`SoughtStep`** `{to, scale}`.
