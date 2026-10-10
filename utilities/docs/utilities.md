# Utilities

General-purpose utilities, shared by every crate in Civil Egregore and owned by
none.

- **Diagnostics** (`src/diagnostics/`), what every crate's diagnostics
  are made with:
  - **tables and reports** (`table/`): the one table printer; a
    measurement's report -- its tables, notes, and the command and
    commit it came from -- printed and kept as CSV in a folder the
    caller names (each crate's `transient_data/measurements/`), and
    read back;
  - **the process's memory** (`process_memory.rs`), as the system
    counts it: held now and at its peak, from `/proc/self/status`
    (Linux only), and tracked over a run for the average.
- **Transient data** (`transient_data.rs`): where a crate's runs leave
  what they make, `transient_data/` beside its `Cargo.toml`, out of git.
  Each crate names its own in its `src/transient_data.rs`.
- **Commands** (`commands.rs`): a command line's first word found among
  a crate's commands and handed the rest. A crate's tools are functions
  listed with their parameters, each declared once with its default;
  the one program routes to a crate by name and knows none of its tools.
- **Settings** (`settings.rs`): this machine's settings, kept from run
  to run in one file in one folder of Civil Egregore's own, wherever the
  system keeps such, a line each, shared by whatever has settings (so
  far the renderer's sliders), found by name in any order -- what the
  file lacks is the default settings'. The default settings are such a file
  built into the program: copied to a machine that has none, never
  over one that has, and all there is under the build feature
  `force_default_settings`. The same folder holds the worlds, in `worlds`,
  unless the settings name another folder for them.
- **What is tuned by eye** (`tuning.rs`): the numbers a window's
  sliders set -- the near view's shading, read each frame, and how a
  new world is made, read once when one is -- by name and place: a
  value handed about, nothing of it held in a static. What a slider
  reaches, its group and what it does are in the sliders' file
  (`sliders.csv`, at the crate's root), written by hand; what each is
  unless set is in the default settings.
- **The seed** (`seed.rs`): the one seed every crate's tests and tools
  start from, in the workspace's `transient_data/seed.csv`, rolled every 5
  counted runs; `CIVIL_EGREGORE_SEED` picks one for a run.
- **A seeded random source** (`rng.rs`), its whole state one word: every
  random number in Civil Egregore comes from it.
- **A chance** (`chance.rs`): how likely a thing is, as a whole number
  of parts in 2^32. See "Chances" below.
- **Fixed point** (`fixed_point.rs`): a logarithm to base 2 by
  whole-number arithmetic. See "Fixed point" below.
- **Hashing** (`hash.rs`): a key's slot in a table, by
  Fibonacci hashing, a word's bits mixed, SplitMix64's way, and many
  words folded into one hash, the same on every machine.
- **A fixed-capacity list** (`fixed_list.rs`), allocated once, never
  growing: for structures sized once and reused.
- **The cache** (`cache.rs`): memory asked for ahead of its being read.

## Layout

| folder | what is in it |
|---|---|
| `src/` | the utilities above |
| `src/diagnostics/` | tables, reports, the process's memory |
| `tests/` | each, judged |
| `docs/` | this, and the reference, function by function |
| `transient_data/` | out of git: what its tests keep |

## Chances

A world follows from its seed alone, the same to the bit on every
machine (`../../server/docs/server.md`, "The same on every machine").
A float cannot promise that where a logarithm is taken of it: `ln` is
the machine's maths library's, and two libraries round its last bit
their own ways. One sample chosen a cell apart on one machine is another
world a thousand ticks on. So nothing a world follows from is a float.

A `Chance` is a whole number of parts in `PARTS`, 2^32: a half is 2^31
parts, once in 100,000 is 42,950. A rule states its chances as
constants -- `Chance::one_in(100_000)`, `Chance::HALF` -- and two things
that never both happen are added with `plus`. The unit was chosen over
"one in N" kept as N because parts add, and because a draw against them
is a shift and a comparison: `Rng::chance` is true when a draw's high 32
bits are under the parts. `Rng::chance_among(part, whole)` says which
of two things it is that happened with `whole`: a number under
`whole`'s parts, under `part`'s or not -- what the grass uses to tell a
spread from a decay, with no division of one chance by another.

The smallest chance there is, one part, is about once in 4.3 billion.
`one_in` rounds to the nearest part, so once in 100,000 is off by a few
parts in a million: the precision is `PARTS`, a number chosen here, and
the same everywhere.

### The gap

Sampling (`../../simulation/docs/simulation.md`, "Sampling") does not
toss a coin a cell. It draws how many set cells to pass over before the
next chosen one, a gap of the geometric law: `floor(log(u) / log(1 -
p))`, `u` uniform in `(0, 1]`, `p` the chance. The base of the
logarithms does not matter, as it is a ratio; base 2 is what whole
numbers find quickly.

`Chance::passed_over(draw)` is that, in fixed point:

- `u` is the draw's high 53 bits, as a number from 1 to 2^53 over 2^53:
  never 0, so it has a logarithm. Its logarithm is `log2` of the number
  less 53, a negative number; negated, `53 - log2(number)`, with 48 bits
  of fraction.
- `log(1 - p)` is `log2(PARTS - parts)` less 32, negative too; negated,
  `32 - log2(PARTS - parts)`. It depends on the chance alone, so it is
  worked out once, where the chance is made -- as the program is built,
  for a rule's constants -- and kept in the `Chance` beside its parts.
- The gap is the first over the second: one division of two whole
  numbers, rounded down.

Fifty-three bits of draw, not 32, because a gap may be far longer than
`PARTS`: at one part the mean gap is 2^32 cells, and the long tail of
the law is in the bits under those.

How precise a gap is: the second number, for the smallest chance, is
some 94,000, so the gaps of a chance of one part are right to about a
part in 100,000; for once in 100,000 to a part in four billion. A test
draws 200,000 gaps for chances from a half to one part and holds each
to the float formula's, and their mean to the law's
(`tests/fine/chance.rs`).

## Fixed point

`log2(value)` gives the logarithm to base 2 of a whole number, times
2^48 (`LOG2_FRACTION_BITS`): a whole number again.

Its whole part is where the number's highest bit is. What is left is
the number over that power of two, in `[1, 2)`, the mantissa, and its
logarithm is the fraction. Two ways find it.

**By squaring** (`log2_by_squaring`). Square the mantissa: if the square
is 2 or more the next bit of the fraction is 1, and the square is
halved; else the bit is 0. A bit a squaring, the squares kept in 128
bits. It is exact to about a part in 2^56 and plain to see right, and
it is slow: a multiplication for each of 48 bits, some 900 instructions.

**By table and series** (`log2`). The seven bits after the mantissa's
highest pick one of 128 rows; each row is a stretch of `[1, 2)` a part
in 128 wide, and the tables hold its middle's logarithm and 1 over its
middle. The mantissa times that inverse is 1 and a little, under a part
in 256 either way; the logarithm of 1 and a little is a series in the
little -- `x - x^2/2 + x^3/3 - ...` to the sixth power, the next term
under a part in 2^58 -- times 1 over the natural logarithm of 2. The row's
logarithm and that, added, are the fraction. Eight multiplications. The
tables are made by the squaring, as the crate is built.

The second is the one used. Sampling's gaps made the tick reference
(`Civil_Egregore server pasture 300 333 4000 4 1`, less the run with no
ticks, counted by callgrind) 64.29 million instructions by squaring,
against 58.50 million with the float logarithm it replaced; by table
and series it is 58.74 million.

The two agree to within four of the last bit, on every number tried
and at every row's ends, and both are the float's to a part in 2^40
(`tests/fine/chance.rs`). A power of two's logarithm is exact, which
the gap relies on: the draw that is 1 has a gap of 0, never under.
The result is the same on every machine either way; which bits it has
is fixed by the code, not by a library.

## What may not be called

`clippy.toml`, at the workspace's root, forbids a float's logarithm,
exponential, power and trigonometry (`disallowed-methods`: `ln`, `exp`,
`powf`, `sin` and their like, on `f32` and `f64`), the float draw
`Rng::unit`, and `Chance::fraction`, in every crate without a
`clippy.toml` of its own. The crates that are no part of what a world
follows from have an empty one: the renderer, the menus, Tessera, the
AI's scratchpad, and this crate, where the float draw is defined and
measurements are tabled. A float's sum, product, quotient and square
root are exact by the standard and are let be. A test that wants a
float for what it expects allows it by name, with the reason.
