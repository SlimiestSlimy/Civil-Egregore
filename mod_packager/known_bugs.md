# Known bugs

## The export check can name the wrong library

The packager checks every set up function by compiling and linking a
generated file against the built mods:

- a `const _: TypeSetUp = <crate>::<function>;` line, so the function
  exists, is reachable at its mod crate's root, and has the right
  signature;
- the function's name declared in an `unsafe extern "Rust"` block and
  kept in a `#[used]` static, so the linker must find it exported,
  unmangled, by `#[unsafe(no_mangle)]`.

The linker looks for that name in every library on the link line, not
in the mod's own. So a server mod that `pub use`s a set up
function defined in a common mod's crate passes: the path
resolves, and the name is exported, but by the common mod's library.
The game then looks the name up in the server mod's library, as the
manifest says, and does not find it.

**What happens:** the game refuses to load the mod, saying "undefined
symbol" for that function. Never a crash: the signature was checked.

**Why it is left:** checking each mod's exports on its own means
linking each mod's check alone, against its library and no other,
which the linker makes awkward: a mod's library needs its
dependencies' on the link line too.

**Until then:** define each set up function in its own mod's crate,
or in a helper crate only that mod uses.
