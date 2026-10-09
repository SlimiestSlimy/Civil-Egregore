# The Monte Carlo rules

The rules the cells run by, a file each. A rule is a function of one
superchunk's turn in a tick's first phase (`simulation/`): it samples
the cells of a layer at a chance -- Monte Carlo, so no cell is visited
that is not drawn -- reads the world as the tick found it, and queues
writes, which change nothing until the second phase. A rule knows no
other rule and no entity; the world's tick (`server/`) runs them
together with the entities (`entity_rules/`).

So far one: grass spreading over dirt and decaying
(`../docs/Civil Egregore.md`, "Sampling"), where a halo keeps it hot
(`../server/docs/server.md`, "Halos"). For now, too, only within three
superchunks across of the middle of the world's origin superchunk: no
part of the rule, a limit on the present test, since grass let spread
without end would lead the sheep, their halos, and so the world, to
grow without end. A superchunk the limit leaves out samples nothing;
one it crosses samples its grass and drops the samples outside. Grass
elsewhere lies as it was made, unless eaten.

## Layout

| folder | what is in it |
|---|---|
| `src/grass.rs` | grass over dirt |
| `src/trees.rs` | trees: spreading by how crowded they stand, growing through sixteen stages, dying |
| `tests/` | the rule's behaviour, judged |
| `docs/` | this, and the reference, function by function |

## Trees: more than a bit a cell

A tree is a cell set in `TREE`, with a stage of sixteen kept over four
more bits (`TREE_STAGE`): a wide plane, four bits a cell, a cell's
number held together and read and written whole through
`read::cells::value` and `write::cells::set_value`. The bit that says a tree stands
there is a plane of its own, not stage 0: sampling the trees and
counting those about one are then each one read of one plane, as for
grass, where a tree found by any of four planes being set would take
four.
