#!/bin/sh
# Builds dist/volcasm-<version>-macos-universal.zip (Volcasm.app) from the
# two release binaries in target/{aarch64,x86_64}-apple-darwin/release.
# Needs lipo, sips, iconutil and codesign (macOS). Run from the repository
# root after `cargo build --release -p volcasm-app --target <triple>` for
# both targets.
#
# The bundle is signed ad hoc only (no Developer ID): Gatekeeper blocks the
# first start until the user allows it (docs/testers.md).
set -eu
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
app=target/Volcasm.app
rm -rf "$app" target/volcasm.iconset
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/licenses"
lipo -create -output "$app/Contents/MacOS/volcasm" \
    target/aarch64-apple-darwin/release/volcasm \
    target/x86_64-apple-darwin/release/volcasm

# The icon exists only at 256 px; the larger sizes are left out.
iconset=target/volcasm.iconset
mkdir -p "$iconset"
for size in 16 32 128 256; do
    sips -z $size $size volcasm-app/assets/icon.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
done
for size in 16 128; do
    double=$((size * 2))
    cp "$iconset/icon_${double}x${double}.png" "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns -o "$app/Contents/Resources/volcasm.icns" "$iconset"

cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Volcasm</string>
    <key>CFBundleDisplayName</key><string>Volcasm</string>
    <key>CFBundleIdentifier</key><string>io.github.volcasm</string>
    <key>CFBundleExecutable</key><string>volcasm</string>
    <key>CFBundleIconFile</key><string>volcasm</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSMicrophoneUsageDescription</key>
    <string>Volcasm records the volca sample's audio output into a slot.</string>
</dict>
</plist>
EOF

packaging/licenses.sh "$app/Contents/Resources/licenses"
codesign --force --deep --sign - "$app"

mkdir -p dist
ditto -c -k --keepParent "$app" "dist/volcasm-$version-macos-universal.zip"
