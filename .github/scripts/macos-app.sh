#!/usr/bin/env bash
# Packages a built example as a macOS app bundle in a disk image:
#
#   macos-app.sh <binary> <app id> <version> <output.dmg>
#
# The disk image opens to the app beside a link to Applications, to drag it
# into, as Mac apps are installed.
#
# The bundle's name is the desktop entry's (`.github/apps/<app id>.desktop`)
# and its icon is made from the same SVG as the AppImage's, so each app has
# one name and icon everywhere. `<app id>.plist`, when there is one, adds
# keys to the Info.plist.
#
# With MACOS_SIGN_IDENTITY (a Developer ID Application identity in a
# keychain), the bundle is signed with the hardened runtime and notarized
# with an App Store Connect API key (MACOS_NOTARY_KEY, the .p8's path;
# MACOS_NOTARY_KEY_ID; MACOS_NOTARY_ISSUER, for a team key), and the ticket
# is stapled to it; then the disk image is signed, notarized and stapled
# too. So Gatekeeper opens both without asking, even offline, and the app
# keeps its ticket once it's copied out of the image. Without it, the
# bundle is signed ad hoc, for trying locally: Gatekeeper asks before a
# downloaded one's first launch. Unsigned, a downloaded bundle
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

# The disk image: the app and a link to Applications, compressed.
make_dmg() {
  local staging="$work/dmg"
  mkdir "$staging"
  ditto "$app" "$staging/$name.app"
  ln -s /Applications "$staging/Applications"
  # hdiutil fails now and then on CI's macOS runners ("Resource busy"):
  # try again a few times before giving up.
  local try
  for try in 1 2 3 4 5; do
    if hdiutil create -volname "$name" -srcfolder "$staging" -fs HFS+ -format UDZO -ov "$output" >/dev/null; then
      break
    fi
    if [ "$try" = 5 ]; then
      echo "macos-app.sh: couldn't make $name's disk image" >&2
      exit 1
    fi
    sleep $((try * 5))
  done
  rm -rf "$staging"
}

if [ -z "${MACOS_SIGN_IDENTITY:-}" ]; then
  codesign --force --sign - "$app"
  codesign --verify --strict "$app"
  make_dmg
  rm -rf "$work"
  exit 0
fi

issuer=()
if [ -n "${MACOS_NOTARY_ISSUER:-}" ]; then
  issuer=(--issuer "$MACOS_NOTARY_ISSUER")
fi

# Submits a zip or a disk image and waits; stops the script if Apple
# doesn't accept it, after printing its log.
notarize() {
  local result
  result=$(xcrun notarytool submit "$1" --key "$MACOS_NOTARY_KEY" \
    --key-id "$MACOS_NOTARY_KEY_ID" ${issuer[@]+"${issuer[@]}"} --wait --output-format json)
  echo "$result"
  if [ "$(echo "$result" | plutil -extract status raw -)" != Accepted ]; then
    xcrun notarytool log "$(echo "$result" | plutil -extract id raw -)" --key "$MACOS_NOTARY_KEY" \
      --key-id "$MACOS_NOTARY_KEY_ID" ${issuer[@]+"${issuer[@]}"} || true
    echo "macos-app.sh: $2 wasn't notarized" >&2
    exit 1
  fi
}

# The app first: notarytool takes it zipped, and the ticket is stapled to
# the bundle, so the copy in Applications has its own.
codesign --force --options runtime --timestamp --sign "$MACOS_SIGN_IDENTITY" "$app"
codesign --verify --strict "$app"
ditto -c -k --keepParent "$app" "$work/notarize.zip"
notarize "$work/notarize.zip" "$name"
xcrun stapler staple "$app"
spctl --assess --type execute --verbose "$app"

# Then the disk image around it.
make_dmg
codesign --force --timestamp --sign "$MACOS_SIGN_IDENTITY" "$output"
notarize "$output" "$name's disk image"
xcrun stapler staple "$output"
spctl --assess --type open --context context:primary-signature --verbose "$output"
rm -rf "$work"
