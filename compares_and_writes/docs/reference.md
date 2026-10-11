# Compares and writes: reference

Function by function. The design: `compares_and_writes.md`. `lib.rs` hands on
the simulation's `Turn`, which every function is over.

## `compare.rs`

`compares_and_writes.md`, "Compares". **`Compare`**: the simulation's, what a
rule saw. **`holds(layer_type, cell)`**, **`lacks(layer_type, cell)`**:
of a layer of a bit a cell; **`value(plane, cell, seen)`**: of the
number a wide plane holds; **`cell_free(cell)`**: an entity may be
made there; **`entity(entity)`**: it is there still;
**`attribute(entity, attribute, seen)`**: of an entity's attribute by
its layout, or of its having none; **`attribute_blocks(at, kind,
seen)`**: the same of blocks; **`attribute_member(at, kind, member,
seen)`**: of one member. **`open(turn, compare)`**, **`close(turn)`**:
queued, open until closed, one within another.
**`from_here(turn, compare)`**: queued, taking the place of the one
given before; **`end(turn)`**: its end.
**`with_writes(turn, compare, writes)`**: the compare, the writes and
its end in one call (`compares_and_writes.md`, "A compare and its writes at
once"); `lib.rs` hands on **`Write`**, a write as a value.

## `write.rs`

`compares_and_writes.md`, "Writes". **`cell(turn, layer_type, cell, value,
counted)`**; **`count(turn, counted)`**; **`attribute_member(turn, at,
kind, member, value)`**; **`attribute_whole(turn, at, attribute)`**;
**`attribute_removed(turn, at, kind)`**; **`wake(turn, at, wake)`**.

## `entity.rs`

`compares_and_writes.md`, "Writes". **`make(turn, header)`**;
**`move_to(turn, from, to)`**; **`remove(turn, at)`**;
**`cross(turn, left, after, default, attributes)`**.
