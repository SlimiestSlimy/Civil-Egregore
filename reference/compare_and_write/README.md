# Compare-and-write, kept for reference

Not built and not part of any crate: the old server's compare-and-write
builder and the least of its core needed to make sense of it, kept when
the rest of the old server was removed (2026-10-25), for the rewrite to
read.

- `compare_and_write.rs`: the builder. A write is a method of an open
  skip-tag set, so none is queued under no compare.
- `raw_compares_and_writes/`: the compares and writes the builder queues,
  each one instruction.
- `simulation_tick_state/`: the instruction (24 bytes, an opcode first),
  compares as instructions with their skip tags, writes one instruction
  each, and how a superchunk's queue is applied.
- `compare_and_write.md`: the old simulation API doc's "Compare-and-write"
  section. `raw_compares_and_writes.md`: the old doc of the compares and
  writes.

Paths inside the files (`crate::...`, `docs/...`) are the old server's.
