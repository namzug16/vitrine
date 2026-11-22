#!/usr/bin/env bash
# Builds target/release/Vitrine.app: the release binary, its Info.plist and an ad-hoc signature.
# Drag the result to /Applications to launch it from Finder or the Dock.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release
version=$(cargo pkgid | sed 's/.*[#@]//')
app=target/release/Vitrine.app

rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp target/release/vitrine "$app/Contents/MacOS/Vitrine"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>Vitrine</string>
  <key>CFBundleIdentifier</key><string>dev.namzug.vitrine</string>
  <key>CFBundleName</key><string>Vitrine</string>
  <key>CFBundleDisplayName</key><string>Vitrine</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
PLIST
codesign --force --sign - "$app"
echo "$app"
