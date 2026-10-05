# Civil Egregore, function by function

The `Civil_Egregore` crate is the program, and nothing else: `src/main.rs`
holds which crates have commands, and hands the command line to the one
its first word names (`utilities::commands`). Everything run is a crate
beside it, each with a `docs/` of its own: the world made, ticked, saved
and loaded (`server/`, whose commands are `server/src/commands.rs`), the
rules of the cells (`mc_rules/`), the entities (`entity_rules/`). The
root has no tests: its commands are the crates', tested there. What
Civil Egregore is and every decision about it: `Civil Egregore.md`.

## `main.rs`

**`CRATES`**: the crates with commands, by name -- `server`, `tessera`
-- each with its table of them. **`main`**: the command line handed to
`utilities::commands::program`; `Civil_Egregore help` prints every command of
every crate.
