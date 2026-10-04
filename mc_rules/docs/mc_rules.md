# The Monte Carlo rules

The rules the cells run by, a file each. A rule is a function of one
superchunk's turn in a tick's first phase (`simulation/`): it samples
the cells of a layer at a chance -- Monte Carlo, so no cell is visited
that is not drawn -- reads the world as the tick found it, and queues
writes, which change nothing until the second phase. A rule knows no
other rule and no entity; the world's tick (`world/`) runs them
together with the entities (`entity_rules/`).

So far one: grass spreading over dirt and decaying
(`../docs/tilesim.md`, "Sampling") -- only in a circle three
superchunks across about the middle of the world's origin superchunk,
and there only where a halo keeps it hot (`../world/docs/world.md`,
"Halos"). A superchunk the circle misses samples nothing; one it
crosses samples its grass and drops the samples outside. Grass
elsewhere lies as it was made, unless eaten.

## Layout

| folder | what is in it |
|---|---|
| `src/grass.rs` | grass over dirt |
| `tests/` | the rule's behaviour, judged |
| `docs/` | this, and the reference, function by function |
