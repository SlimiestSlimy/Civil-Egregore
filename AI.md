# For whoever works on TileSim with an AI

Rules that hold whatever the task. The handoff, the style guide
(`docs/style_guide.md`) and the testing protocol
(`docs/testing_protocol.md`) say the rest.

## Local and remote sessions

- **On the local machine** (the user's PC, which has a screen): Bevy is
  never skipped. The renderer is built and kept working, and **all
  further testing and measuring of the running world is done in the
  renderer** -- `cargo run --release -p renderer -- ...`; its census
  (`renderer/transient_data/measurements/census.csv`) carries the ticks
  and the seconds they took. The renderer may be closed and reopened as
  runs need.
- **On a remote session** (the cloud, no screen): the renderer is not
  built or run, so Bevy is skipped -- never `--workspace`, which builds
  it; test and lint per crate.

## Measuring under full load

The world is hot only in the halos about its keepers, which move with
them. To measure a fixed load, every superchunk shown is forced hot and
kept so -- the renderer's fifth argument:

```sh
cargo run --release -p renderer -- <superchunks> <sheep a superchunk> 0 0 1
```

## Seeds, commands and figures

- No test or tool has a seed written in it: all take the one in
  `transient_data/seed`, rolled every five counted runs
  (`utilities::seed`; `TILESIM_SEED=<seed>` or `=fresh` for one run). A
  test must hold on any seed; check a changed test on several fresh ones.
- Every command is run through the root program:
  `cargo run --release -- help` lists them all.
- A doc gives a performance figure only where it explains an
  optimization, before and after; nowhere else.

## Renames

Try to use the language server for renames, if easier: it follows a
name through every use, where a text replace also catches words that
only look the same. A crate's folder and package name, and names in
prose, are still moved and replaced by hand.
