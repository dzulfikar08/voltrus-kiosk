#!/usr/bin/env bash
# macos-app.sh VERSION BINARY OUT_ZIP: wrap the universal binary in "Voltrus Kiosk.app"
# (icon, camera usage text, ad-hoc signature) and zip it. Runs on macOS (iconutil, codesign,
# ditto). Updates later replace only Contents/MacOS/voltrus-kiosk.
set -euo pipefail
version="$1"; binary="$2"; out="$3"
root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
app="$work/Voltrus Kiosk.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/voltrus-kiosk"
chmod 755 "$app/Contents/MacOS/voltrus-kiosk"

iconset="$work/AppIcon.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$root/assets/icon/icon-1024.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$root/assets/icon/icon-1024.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Voltrus Kiosk</string>
  <key>CFBundleDisplayName</key><string>Voltrus Kiosk</string>
  <key>CFBundleIdentifier</key><string>id.voltrus.kiosk</string>
  <key>CFBundleExecutable</key><string>voltrus-kiosk</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.business</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSCameraUsageDescription</key><string>Voltrus Kiosk uses the camera to recognise employees checking in.</string>
</dict></plist>
PLIST

codesign --force --sign - "$app"
codesign --verify "$app"
rm -f "$out"
ditto -c -k --keepParent "$app" "$out"
rm -rf "$work"
echo "$out"
