# For whoever works on TileSim with an AI

Rules that hold whatever the task. The handoff, the style guide
(`docs/style_guide.md`) and the testing protocol
(`docs/testing_protocol.md`) say the rest.

## Local and remote sessions

- **On the local machine** (the user's PC, which has a screen): Bevy is
  never skipped. The viewer is built and kept working, and **all
  further testing and measuring of the running world is done in the
  viewer** -- `cargo run --release -p viewer -- ...`; its census
  (`viewer/transient_data/measurements/census.csv`) carries the ticks
  and the seconds they took. The viewer may be closed and reopened as
  runs need.
- **On a remote session** (the cloud, no screen): the viewer is not
  built or run, so Bevy is skipped -- never `--workspace`, which builds
  it; test and lint per crate.

## Measuring under full load

The world is hot only in the halos about its keepers, and grass grows
only in a circle about the origin: neither is a load. To measure, every
superchunk is forced hot and grass grows everywhere -- the viewer's
fifth argument:

```sh
cargo run --release -p viewer -- <superchunks> <sheep a superchunk> 0 0 1
```
