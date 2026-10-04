# Handoff

Where the work stands, and what is left, for whoever picks it up next.
Delete this whole `delete_after_use/` folder once it has been read and
acted on.

## State

The branch is `claude/bit-array-area-ops-03ea5j`, clean and pushed. The
last code commit is the cooling timer (`9e46b5a`).

Checked on that commit:
- `cargo clippy --release --all-targets` (per crate, never `--workspace`): clean.
- `cargo test --release`: passes.
- `cargo test --release --test complete -p world -- --ignored`: passes, about 73 s.
- Tick instructions: 63,826,674, against the 63,611,138 reference (+0.34%; the limit is +1%).

## House rules

- Never `--workspace`: it builds Bevy, through the viewer. Test and lint per crate, in release.
- Never run `cargo fmt`.
- Simplify for readability without performance regressions: within 1% of the reference tick instructions.
- One distinct name per thing. Keep `docs/glossary.md` current.
- Every crate has the folder structure in `docs/style_guide.md`.
- Morton is the default. Cartesian is named as such (`CellCartesian`).
- Few tests, made by generators.
- Skip the viewer until told: it is still broken by renames.

## Measuring tick instructions

The tick instructions are callgrind's count for the reference run less
the same run with no ticks:

```sh
cargo build -q --release -p world --bin diagnostics
ir() { valgrind --tool=callgrind --callgrind-out-file=/tmp/ir.cg.$1 target/release/diagnostics pasture $1 333 4000 4 1 2>&1 | grep Collected | awk '{print $4}'; }
a=$(ir 300); b=$(ir 0); echo "tick Ir: $((a - b))  (reference: 63611138)"
```

This run does not exercise halos.

## In progress: drawing height top down (not committed, by request)

The goal is to find how the heightmap should look in a fully vertical,
top-down view. The scale: a cell is 2 m, drawn 8x8 px. People are 7 px
stickmen. The game runs at 256 ticks a second.

The terrain is 8-bit heights per cell. Neighbours mostly differ by 0 or
1, and a difference over 1 is a wall (`terrain::wall`), so the ground is
naturally terraced. The proposed layers, by strength:

1. **Walls**, strong, since they matter to play. A dark band on the
   downhill side, 2–4 px by the height difference, and a light 1 px lip
   on the uphill side. The light comes from the top left.
2. **Steps of 1**, subtle: a faint 1 px line on the lower cell, darker
   where the rise faces away from the light. They act as free contour
   lines.
3. **Broad shape**: hillshade from a slope smoothed over 5–9 cells,
   quantized to 3–4 bands to stay pixel art, plus a slight altitude
   tint. This is the layer that carries zoomed out.
4. **Optional cast shadows**: a sweep along the light's diagonal, so
   cliffs throw shadows. Heights do not change yet, so each superchunk
   needs this only once.

A throwaway render tool, `delete_after_use/heightart/`, was written but
**not built or run yet**, so expect a compile error or two. It is a
standalone crate (its own `[workspace]`, so not a member of the repo's)
with a path dependency on `terrain` and `png = "0.18"`. Run it from its
folder:

```sh
cd delete_after_use/heightart
cargo run --release -- [seed=4] [metres a height unit=1.0] [sun elevation°=35] [out folder=target/renders]
```

It writes:
- `0_texture` through `6_shadows_only`: a 128x96 cell patch (the one
  with the most walls in the middle superchunk), each layer added in
  turn, with stickmen and sheep for legibility.
- `7_mid`: the whole superchunk, 1 cell a pixel.
- `8_far`: 3x3 superchunks, 4 cells a pixel.

Next:
- Build and run it.
- Compare the looks with the user.
- Tune the strengths.

Open question for the user: how many metres is one height unit? Steps
of 1 being walkable on 2 m cells suggests at most about 1 m. It sets
shadow lengths and how strong the shading is.

## Backlog

- **#86**: read every character of every file. One name per thing,
  generalize repeats.
- **#72 Area allocator**: 256 MiB blocks, 256-byte units, a sorted
  interval list, owning handles. It comes after #73.
- **#73 Tessera deep clean**: settle the code and move the abstraction
  boundaries (design statement #5).
- **Viewer**: broken by the renames, so skipped until told.
- **Zooming out over cold superchunks**: it needs coarser LOD than
  `2^6` cells a pixel, and a small per-superchunk summary (the shaded
  colour), kept beside the image and rebuilt on flush.
- **Fast keepers**, when vehicles come:
  - At 256 ticks a second, 1 cell a tick is about Mach 1.5.
  - Keep `top speed × WARM_TICKS` plus the read reach under 1,024
    cells, with a compile-time check.
  - Faster vehicles keep a bigger halo: a per-kind radius in
    `HALO_KEEPERS`. A radius of *r* superchunks allows
    *r* × 1,024 / `WARM_TICKS` cells a tick.
- **Far future, not now**: catch-up of natural processes in cold
  superchunks.
