#!/bin/sh
# Builds the experiment, lays it out as the game would ship -- out/game:
# the binaries, the engine's libraries, libstd; out/world/mods/<mod>:
# each mod's bundles and manifest -- and runs the server, then the client.
set -e
cd "$(dirname "$0")"
TARGET=x86_64-unknown-linux-gnu
cargo build --release --offline --target $TARGET
BUILT=target/$TARGET/release
rm -rf out && mkdir -p out/game out/world/mods/shapes out/world/mods/painter
cp $BUILT/server_host $BUILT/client_host $BUILT/libengine_common.so $BUILT/libengine_server.so $BUILT/libengine_client.so out/game/
cp "$(rustc --print target-libdir --target $TARGET)"/libstd-*.so out/game/
cp $BUILT/libshapes_common.so $BUILT/libshapes_server.so $BUILT/libshapes_client.so mods/shapes/manifest.csv out/world/mods/shapes/
cp $BUILT/libpainter_client.so mods/painter/manifest.csv out/world/mods/painter/
ls -la out/game out/world/mods/*
echo "== server"; out/game/server_host out/world
echo "== client"; out/game/client_host out/world
