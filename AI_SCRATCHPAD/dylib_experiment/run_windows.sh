#!/bin/sh
# The Windows build of the experiment, laid out as run.sh lays out the
# Linux one -- MinGW's runtime DLLs shipped beside libstd -- and run under
# Wine. Manifests name .dll files in place of .so ones.
set -e
cd "$(dirname "$0")"
TARGET=x86_64-pc-windows-gnu
cargo build --release --offline --target $TARGET
BUILT=target/$TARGET/release
OUT=out_windows
rm -rf $OUT && mkdir -p $OUT/game $OUT/world/mods/shapes $OUT/world/mods/painter
cp $BUILT/server_host.exe $BUILT/client_host.exe $BUILT/engine_common.dll $BUILT/engine_server.dll $BUILT/engine_client.dll $OUT/game/
cp "$(rustc --print target-libdir --target $TARGET)"/std-*.dll /usr/x86_64-w64-mingw32/bin/libgcc_s_seh-1.dll /usr/x86_64-w64-mingw32/bin/libwinpthread-1.dll $OUT/game/
cp $BUILT/shapes_common.dll $BUILT/shapes_server.dll $BUILT/shapes_client.dll $OUT/world/mods/shapes/
cp $BUILT/painter_client.dll $OUT/world/mods/painter/
for MOD in shapes painter; do sed 's/lib\([a-z_]*\)\.so/\1.dll/g' mods/$MOD/manifest.csv > $OUT/world/mods/$MOD/manifest.csv; done
WORLD="Z:$(realpath $OUT/world | tr / '\\')"
echo "== server"; WINEDEBUG=err+module wine $OUT/game/server_host.exe "$WORLD"
echo "== client"; WINEDEBUG=err+module wine $OUT/game/client_host.exe "$WORLD"
