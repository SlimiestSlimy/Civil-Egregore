#!/bin/sh
# What the game's build would do: write out/modding.toml, everything a
# mod built on this machine needs -- where the game's libraries are, the
# metadata of the crates inside them (Bevy's), and Bevy's derive macros.
# A mod's .cargo/config.toml includes it, through its civil_egregore link.
set -e
cd "$(dirname "$0")"
E=$PWD
T=$E/target/x86_64-unknown-linux-gnu/release
ECS=$(ls -t $T/deps/libbevy_ecs-*.rmeta | head -1)
cat > out/modding.toml <<END
# Written by the game's build: what a mod needs, on this machine.
[target.x86_64-unknown-linux-gnu]
rustflags = [
    # One libstd, shared; the game's libraries found beside the binary.
    "-C", "prefer-dynamic",
    # The game's crates, from its build.
    "--extern", "engine_client=$E/out/game/libengine_client.so",
    "--extern", "shapes_client=$E/out/world/mods/shapes/libshapes_client.so",
    # Bevy's derives name bevy_ecs.
    "--extern", "bevy_ecs=$ECS",
    # The metadata of the crates inside the game's libraries, and the macros.
    "-L", "dependency=$E/out/game",
    "-L", "dependency=$E/out/world/mods/shapes",
    "-L", "dependency=$T/deps",
    "-L", "dependency=$E/target/release/deps",
]
END
