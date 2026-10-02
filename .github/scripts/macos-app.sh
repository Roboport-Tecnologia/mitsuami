#!/usr/bin/env bash
# Packages a built example as a macOS app bundle, zipped:
#
#   macos-app.sh <binary> <app id> <version> <output.zip>
#
# The bundle's name is the desktop entry's (`.github/apps/<app id>.desktop`)
# and its icon is made from the same SVG as the AppImage's, so each app has
# one name and icon everywhere. `<app id>.plist`, when there is one, adds
# keys to the Info.plist. The bundle is signed ad hoc and not notarized:
# Gatekeeper asks before the first launch. Unsigned, a downloaded bundle
# whose Info.plist and icon its signature doesn't seal is reported as
# damaged instead.
set -euo pipefail

binary=$1
id=$2
version=$3
output=$4

here=$(cd "$(dirname "$0")/.." && pwd)
name=$(sed -n 's/^Name=//p' "$here/apps/$id.desktop")
exe=$(basename "$binary")
work=$(mktemp -d)
app="$work/$name.app"

mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/$exe"

# An iconset has each size at 1x and 2x; sips renders SVG.
iconset="$work/AppIcon.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
  sips -s format png -z "$size" "$size" "$here/apps/$id.svg" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  sips -s format png -z $((size * 2)) $((size * 2)) "$here/apps/$id.svg" \
    --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/AppIcon.icns"

# Mixed localizations: the bundle has no `.lproj` folders, so this lets
# the frameworks' own strings follow the user's language.
cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleAllowMixedLocalizations</key>
  <true/>
  <key>CFBundleExecutable</key>
  <string>$exe</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleIdentifier</key>
  <string>$id</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$name</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$version</string>
  <key>CFBundleVersion</key>
  <string>$version</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSPrincipalClass</key>
  <string>NSApplication</string>
</dict>
</plist>
EOF
if [ -f "$here/apps/$id.plist" ]; then
  /usr/libexec/PlistBuddy -c "Merge $here/apps/$id.plist" "$app/Contents/Info.plist" >/dev/null
fi
plutil -lint "$app/Contents/Info.plist" >/dev/null

codesign --force --sign - "$app"
codesign --verify --strict "$app"
ditto -c -k --keepParent "$app" "$output"
rm -rf "$work"
