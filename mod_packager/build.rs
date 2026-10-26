//! The game's flags and toolchain, as cargo builds the packager with
//! them -- the game's own, its `.cargo/config.toml` and
//! `rust-toolchain.toml` read by cargo and rustup, never by us -- kept
//! in the packager, to build mods with the same.

fn main() {
    // Cargo's flags, one after another, split by 0x1F.
    let flags = std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    println!("cargo::rustc-env=GAME_RUSTFLAGS={flags}");
    // Rustup's toolchain, if rustup chose it.
    println!("cargo::rerun-if-env-changed=RUSTUP_TOOLCHAIN");
    if let Ok(toolchain) = std::env::var("RUSTUP_TOOLCHAIN") {
        println!("cargo::rustc-env=GAME_TOOLCHAIN={toolchain}");
        // Its channel, as a rust-toolchain.toml says it on any machine: the host's triple off.
        let host = std::env::var("HOST").unwrap_or_default();
        let channel = toolchain.strip_suffix(&format!("-{host}")).unwrap_or(&toolchain);
        println!("cargo::rustc-env=GAME_CHANNEL={channel}");
    }
}
