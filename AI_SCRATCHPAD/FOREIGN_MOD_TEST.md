# The foreign-machine mod test: a walkthrough for a fresh session

You are a Claude session on a machine that is **not** the developer's.
You start with no context, so everything you need is here. Read it all
before running anything.

## What this test is for

Civil Egregore is a game written in Rust whose mods are dynamic libraries
loaded at runtime. A mod is compiled against the game *as compiled*: it
links the game's own built libraries (`libprotocol.so` and so on) instead
of building the game's crates again. Types then cross between the game
and mods, and between mods, through `Box<dyn Any>` and `downcast_ref` --
which only works if both sides agree on each type's `TypeId`.

On the developer's machine that already works: the in-tree mod
`basegame` and an out-of-tree mod `meadow` load together, and `meadow`
downcasts a type defined in `basegame`.

The open question this test answers: **does a mod compiled on a
different machine, against that machine's own build of the game, load
into the developer's build?** You build the game and a tiny mod here,
check it loads here, and push the built mod to a branch. The developer
then copies it to their machine and loads it there. If cargo hashes
anything machine-specific (absolute paths, for instance) into a crate's
`-C metadata`, the two builds of `protocol` will differ: the mod's
symbols will not resolve, or its downcast will fail. Either result is
useful. **Do not try to fix the game to make it pass**; record what
happens.

## Rules

- Never commit to or push `main`. Your only push is a new branch,
  `foreign-mod-test`.
- Change no file of the repository; add only the one folder described
  in step 5.
- Do not build anything under `reference/` (archived code, not part of
  the build; one of them needs Bevy, which must never be built in a
  remote session).
- No third-party crates, no shell scripts committed.
- Linux x86_64. The toolchain is pinned by `rust-toolchain.toml`; let
  rustup install exactly that one.

## The layout you will find

- The root is a Cargo workspace: `utilities`, `protocol`,
  `game_packager`, `AI_SCRATCHPAD`. `utilities` and `protocol` are built
  as Rust dylibs (`crate-type = ["dylib"]`).
- `.cargo/config.toml` at the root gives every build the game's flags:
  `-C target-cpu=x86-64-v3 -C prefer-dynamic -C link-args=-Wl,-rpath,$ORIGIN`.
- `basegame/` is the in-tree mod, a Cargo workspace of its own (not a
  member of the root's). Its `.cargo/config.toml` builds into the root's
  `target/` and links the game's `libprotocol.so` with `--extern`. It
  has two bundles (a bundle is one crate, one dylib): `common` and
  `server`. `basegame_server` defines `pub struct GrassSeen` -- the type
  your mod downcasts.
- `AI_SCRATCHPAD` is a probe binary with two commands you will use:
  - `cargo run --release -q -p AI_SCRATCHPAD -- hash <file>` prints a
    file's hash, as a manifest gives a library's;
  - `cargo run --release -q -p AI_SCRATCHPAD -- load server <mod folder>...`
    reads the mods' manifests, registers their types, opens their
    libraries, runs each bundle's `bundle_set_up`, and lists every type.
- A mod's manifest is `manifest.cdnf` at its folder's root, in CDNF, the
  game's own format (sections by colons, lists by dashes, pairs by a
  colon mid-line; a blank line ends a text, two blank lines end a list).
  The examples below are exact; copy them.

## Step 1: build the game

From the repository root:

```
cargo build --release -p protocol
cargo build --release -p AI_SCRATCHPAD
cd basegame && cargo build --release && cd ..
```

Check these exist: `target/release/libprotocol.so`,
`target/release/libutilities.so`, `target/release/libbasegame_common.so`,
`target/release/libbasegame_server.so`.

Then capture the exact compiler command lines, for the report: touch a
source of each crate and rebuild verbosely.

```
touch protocol/src/lib.rs utilities/src/lib.rs
cargo build --release -v -p protocol 2>&1 | grep 'Running' > /tmp/game_build.txt
cd basegame && touch server/src/lib.rs && cargo build --release -v 2>&1 | grep 'Running' > /tmp/basegame_build.txt && cd ..
```

(If the second build reuses the first's output and prints no `Running`
line for a crate, that is fine; note it.)

## Step 2: hash and check basegame's manifest

`basegame/manifest.cdnf` holds hashes of the developer's libraries. Yours
differ, so rewrite the two `- hash :` lines with this machine's:

```
cargo run --release -q -p AI_SCRATCHPAD -- hash target/release/libbasegame_common.so
cargo run --release -q -p AI_SCRATCHPAD -- hash target/release/libbasegame_server.so
```

The first goes in the `::common::` bundle's `- hash :` line, the second
in `::server::`'s. This edit is local only: **do not commit it**. Then:

```
cargo run --release -q -p AI_SCRATCHPAD -- load server basegame
```

It should print `2 libraries loaded` and four types, `grass` and
`grass_seen` among them. If it does not, stop and report.

## Step 3: make the mod, outside the repository

Create a folder `foreign` beside the repository folder (`../foreign` from
the repository root). In what follows, `REPO` is the repository folder's
own name (as seen from `../foreign`).

`foreign/Cargo.toml`:

```toml
# A mod built on another machine: a workspace of its own, a crate a
# bundle, built against the game's libraries as compiled.
[workspace]
members = ["server"]
resolver = "3"

[profile.release]
debug = "line-tables-only"
```

`foreign/.cargo/config.toml` (outside the repository, so the root's
config is not read: the game's flags are said again here):

```toml
[target.x86_64-unknown-linux-gnu]
rustflags = [
    "-C", "target-cpu=x86-64-v3", "-C", "prefer-dynamic", "-C", "link-args=-Wl,-rpath,$ORIGIN",
    "--extern", "protocol=../REPO/target/release/libprotocol.so",
    "--extern", "basegame_server=../REPO/target/release/libbasegame_server.so",
    "-L", "dependency=../REPO/target/release/deps",
]
```

`foreign/server/Cargo.toml`:

```toml
[package]
name = "foreign_server"
edition = "2024"
description = "A mod built on another machine: its server bundle"

[lib]
crate-type = ["dylib"]

# None: the game's and the base game's libraries come by `--extern`.
[dependencies]
```

`foreign/server/src/lib.rs`:

```rust
//! A mod built on another machine: one sca proc, `foreign_grows`, made of
//! the base game's `grass_seen`, downcast at set up to the base game's type.

use basegame_server::GrassSeen;
use protocol::type_registry::{Export, TypeRegistryHandle};

/// What `foreign_grows` is made of.
pub struct ForeignGrows {
    /// Grass's ID, as the base game's `grass_seen` holds it.
    pub grass_id: u64,
}

/// The bundle set up.
#[unsafe(no_mangle)]
pub fn bundle_set_up(registry: &TypeRegistryHandle<'_>) -> Result<Vec<(&'static str, Export)>, String> {
    const GRASS_SEEN: &str = "basegame>server>grass_seen";
    let grass_seen = registry.code_of(GRASS_SEEN).ok_or_else(|| format!("{GRASS_SEEN}: nothing made of it"))?;
    let grass_seen = grass_seen.downcast_ref::<GrassSeen>().ok_or_else(|| format!("{GRASS_SEEN}: not the base game's GrassSeen"))?;
    Ok(vec![("foreign_grows", Box::new(ForeignGrows { grass_id: grass_seen.grass_id }))])
}
```

Build it, keeping the verbose output:

```
cd ../foreign && cargo build --release -v 2>&1 | tee /tmp/foreign_build.txt; cd -
```

## Step 4: load it here

From the repository root, hash the mod's library:

```
cargo run --release -q -p AI_SCRATCHPAD -- hash ../foreign/target/release/libforeign_server.so
```

Write `foreign/manifest.cdnf`, HASH being that output (keep the two
blank lines after `- basegame>server`: they end the list):

```
:foreign:

::server::
- side : server
- library : target/release/libforeign_server.so
- hash : HASH
- depends on :
- basegame>server


foreign_grows :
- kind : sca proc
```

Then:

```
cargo run --release -q -p AI_SCRATCHPAD -- load server basegame ../foreign
```

It should print `3 libraries loaded` and list
`foreign>server>foreign_grows`. Record the full output whatever it is.

## Step 5: deliver

```
git switch -c foreign-mod-test
mkdir foreign_mod_test foreign_mod_test/source
cp ../foreign/target/release/libforeign_server.so foreign_mod_test/
cp -r ../foreign/Cargo.toml ../foreign/.cargo ../foreign/server foreign_mod_test/source/
```

Write `foreign_mod_test/manifest.cdnf`: the same as step 4's but with
`- library : libforeign_server.so` (the library beside the manifest),
the same hash.

Write `foreign_mod_test/REPORT.md` with:

- `rustc -vV`;
- `uname -a`;
- the absolute path of the repository on this machine;
- every `Running` line from `/tmp/game_build.txt`,
  `/tmp/basegame_build.txt` and `/tmp/foreign_build.txt` (the full
  `rustc` command lines; above all each `-C metadata=` value for
  `protocol`, `utilities`, `basegame_server` and `foreign_server`);
- `sha256sum target/release/libprotocol.so target/release/libutilities.so target/release/libbasegame_server.so`;
- the output of step 2's and step 4's `load` commands.

Restore basegame's manifest before committing (`git checkout basegame/manifest.cdnf`),
then commit only the new folder (the `.so` may be ignored: force-add it)
and push the branch:

```
git add -f foreign_mod_test
git commit -m "foreign_mod_test: a mod built on another machine, for the developer to load"
git push -u origin foreign-mod-test
```

## Your reply

Say: the branch and commit hash pushed; whether step 4 loaded here; and
the `-C metadata` values for `protocol`, `utilities` and
`basegame_server`.
