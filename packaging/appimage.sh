#!/bin/sh
# Builds dist/volcasm-<version>-x86_64.AppImage from target/release/volcasm.
# Needs appimagetool on PATH (or in $APPIMAGETOOL). Run from the
# repository root after `cargo build --release -p volcasm-app`.
#
# Only the program goes into the AppImage. libpipewire and libasound come
# from the system: libpipewire has to match the running PipeWire daemon.
set -eu
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
appdir=target/AppDir
rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/icons/hicolor/256x256/apps" \
    "$appdir/usr/share/applications" "$appdir/usr/share/doc/volcasm"
cp target/release/volcasm "$appdir/usr/bin/"
cp volcasm-app/assets/volcasm.desktop "$appdir/usr/share/applications/"
cp volcasm-app/assets/icon.png "$appdir/usr/share/icons/hicolor/256x256/apps/volcasm.png"
packaging/licenses.sh "$appdir/usr/share/doc/volcasm"
# appimagetool wants the desktop file and icon at the top as well.
cp volcasm-app/assets/volcasm.desktop "$appdir/"
cp volcasm-app/assets/icon.png "$appdir/volcasm.png"
ln -s usr/bin/volcasm "$appdir/AppRun"
mkdir -p dist
ARCH=x86_64 "${APPIMAGETOOL:-appimagetool}" "$appdir" "dist/volcasm-$version-x86_64.AppImage"
