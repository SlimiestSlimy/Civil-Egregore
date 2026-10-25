# Reference

Not built and not part of the workspace: code kept to read while the
game is rewritten (2026-10-25).

- `compare_and_write/`: the old server's compare-and-write builder and
  the least of its core needed to make sense of it.
- `protocol/`: the old protocol crate, but its type registry (now the
  `type_registry` crate at the root): the wire, the frames, the server's
  parameters and shared types, the kinds.
- `client/`: the client, as it was, built on that protocol.

Paths inside them are as they were before the move.

The client needs back in the root `Cargo.toml`, when it returns:

```toml
[workspace.dependencies]
bevy = { version = "0.19", default-features = false, features = ["2d", "ui", "bevy_feathers"] }

# Bevy unoptimized is too slow to watch.
[profile.dev.package.bevy]
opt-level = 3
```
