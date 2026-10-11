# Instructions: reference

Function by function. The design: `instructions.md`. `lib.rs` hands on
the simulation's `Turn`, which every function is over.

## `compare.rs`

`instructions.md`, "Compares". **`Compare`**: the simulation's, what a
rule saw. **`holds(layer_type, cell)`**, **`lacks(layer_type, cell)`**:
of a layer of a bit a cell; **`value(plane, cell, seen)`**: of the
number a wide plane holds; **`attribute(entity, attribute, seen)`**:
of an entity's attribute, or of its having none.
**`from_here(turn, compare)`**: queues it, what is queued after under
it; **`end(turn)`**: queues its end.
