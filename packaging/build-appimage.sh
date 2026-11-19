#!/usr/bin/env bash
# Build CardBuilder (Rust edition) as an AppImage: target/appimage/CardBuilder-<version>-x86_64.AppImage
#
#   SKIP_BUILD=1 ./packaging/build-appimage.sh   pack an existing target/release/cardbuilder
#
# Needs the official AppImage tools (downloaded once into $APPIMAGE_TOOLS_DIR,
# default ~/.cache/cardbuilder/appimage-tools):
#   appimagetool-x86_64.AppImage  — https://github.com/AppImage/appimagetool/releases
#   runtime-x86_64                — https://github.com/AppImage/type2-runtime/releases
set -euo pipefail
cd "$(dirname "$0")/.."

TOOLS="${APPIMAGE_TOOLS_DIR:-$HOME/.cache/cardbuilder/appimage-tools}"
VERSION=$(grep -m1 '^version' crates/cardbuilder/Cargo.toml | cut -d'"' -f2)
OUT=target/appimage
APPDIR="$OUT/CardBuilder.AppDir"
IMAGE="$OUT/CardBuilder-$VERSION-x86_64.AppImage"

mkdir -p "$TOOLS"
fetch() { # name url
  if [ ! -s "$TOOLS/$1" ]; then
    echo "downloading $1"
    curl -fL --retry 3 -o "$TOOLS/$1.part" "$2" && mv "$TOOLS/$1.part" "$TOOLS/$1"
  fi
  chmod +x "$TOOLS/$1"
}
fetch appimagetool-x86_64.AppImage https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
fetch runtime-x86_64 https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64

# SKIP_BUILD=1 packs an already built target/release/cardbuilder (the release workflow builds it
# inside an older distribution, so the AppImage also runs where glibc is older than here)
if [ -z "${SKIP_BUILD:-}" ]; then
  echo "building CardBuilder $VERSION (release)"
  cargo build --release -p cardbuilder
fi
test -x target/release/cardbuilder || { echo "no target/release/cardbuilder to pack" >&2; exit 1; }

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps" "$APPDIR/usr/share/metainfo" "$APPDIR/usr/share/mime/packages"
install -m755 target/release/cardbuilder "$APPDIR/usr/bin/cardbuilder"
strip --strip-unneeded "$APPDIR/usr/bin/cardbuilder" 2>/dev/null || true
install -m644 packaging/cardbuilder.desktop "$APPDIR/usr/share/applications/cardbuilder.desktop"
install -m644 packaging/cardbuilder.png "$APPDIR/usr/share/icons/hicolor/256x256/apps/cardbuilder.png"
# the .cb design-file type, for AppImage integration tools (appimaged, Gear Lever, AppImageLauncher)
install -m644 packaging/cardbuilder-mime.xml "$APPDIR/usr/share/mime/packages/cardbuilder.xml"
sed -e "s/@VERSION@/$VERSION/" -e "s/@DATE@/$(date +%F)/" packaging/cardbuilder.metainfo.xml > "$APPDIR/usr/share/metainfo/cardbuilder.appdata.xml"
cp packaging/cardbuilder.desktop "$APPDIR/cardbuilder.desktop"
cp packaging/cardbuilder.png "$APPDIR/cardbuilder.png"
ln -sf cardbuilder.png "$APPDIR/.DirIcon"
cat > "$APPDIR/AppRun" <<'RUN'
#!/bin/sh
# AppImage entry point
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/cardbuilder" "$@"
RUN
chmod +x "$APPDIR/AppRun"

# graphics, windowing and keyboard libraries (Vulkan/GL drivers, Wayland, X11, xkbcommon) are
# loaded from the host at run time, as they must be for GPU drivers — nothing else to bundle
echo "packing $IMAGE"
ARCH=x86_64 "$TOOLS/appimagetool-x86_64.AppImage" --appimage-extract-and-run --no-appstream \
  --runtime-file "$TOOLS/runtime-x86_64" "$APPDIR" "$IMAGE"
chmod +x "$IMAGE"
ls -lh "$IMAGE"
