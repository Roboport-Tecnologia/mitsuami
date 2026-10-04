#!/usr/bin/env bash
# Packages a built example as an AppImage:
#
#   appimage.sh <gtk|kde> <binary> <app id> <output.AppImage>
#
# The desktop entry and icon are `.github/apps/<app id>.{desktop,svg}`,
# named after the app's id, as GTK and KDE find an app's icon by it.
# linuxdeploy bundles what the binary links, and its GTK or Qt plugin what
# the toolkit loads at run time; glibc and the GPU drivers stay the host's,
# so the build's glibc is the oldest one the AppImage runs on.
set -euo pipefail

toolkit=$1
binary=$2
id=$3
output=$4

here=$(cd "$(dirname "$0")/.." && pwd)
tools=${APPIMAGE_TOOLS:-$PWD/.appimage-tools}
appdir=$(mktemp -d)/AppDir

# No FUSE in containers and on CI's runners: the tools unpack themselves.
export APPIMAGE_EXTRACT_AND_RUN=1
# linuxdeploy's own patchelf and strip are older than packed relative
# relocations (`.relr.dyn`), which Fedora and Arch build their libraries
# with: its patchelf breaks them, and its strip can't read them. The
# system's patchelf (0.18 reads them) is used, and nothing is stripped:
# distributions' libraries come stripped.
if command -v patchelf >/dev/null; then
  export PATCHELF=${PATCHELF:-$(command -v patchelf)}
fi
export NO_STRIP=1

# Release assets can be replaced under the same tag, so each tool is
# checked against the hash it had when it was pinned before it runs.
fetch() {
  [ -x "$tools/$1" ] && return
  curl -fsSL -o "$tools/$1.part" "$2"
  echo "$3  $tools/$1.part" | sha256sum -c --quiet - || { rm -f "$tools/$1.part"; exit 1; }
  mv "$tools/$1.part" "$tools/$1" && chmod +x "$tools/$1"
}
mkdir -p "$tools"
fetch linuxdeploy-x86_64.AppImage \
  https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-x86_64.AppImage \
  c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d

name=$(basename "$binary")
install -Dm755 "$binary" "$appdir/usr/bin/$name"
install -Dm644 "$here/apps/$id.desktop" "$appdir/usr/share/applications/$id.desktop"
install -Dm644 "$here/apps/$id.svg" "$appdir/usr/share/icons/hicolor/scalable/apps/$id.svg"

case "$toolkit" in
  gtk)
    fetch linuxdeploy-plugin-gtk.sh \
      https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/7a3fbc31a9e5075073ff8790f26effbac5f84453/linuxdeploy-plugin-gtk.sh \
      b0f4cbc684a0103a9651f0955b635eaea0096b3a66c0f5a2c2aa337960375171
    export DEPLOY_GTK_VERSION=4
    "$tools/linuxdeploy-x86_64.AppImage" --appdir "$appdir" --plugin gtk
    # The plugin's hook puts GTK on X11 and on Adwaita whatever the
    # desktop: the app runs on Wayland, where GPU surfaces are Wayland
    # subsurfaces, and follows the user's theme, as an installed one does.
    # Its light or dark Adwaita goes with them.
    sed -i -e '/^COLOR_SCHEME=/,/^APPIMAGE_GTK_THEME=/d' -e '/^export GDK_BACKEND=/d' -e '/^export GTK_THEME=/d' \
      "$appdir"/apprun-hooks/*gtk*.sh
    # GTK's media backends load the host's GStreamer, which may want a
    # newer GLib than the AppImage's; mitsuami plays no media.
    rm -rf "${appdir:?}"/usr/lib/gtk-4.0/*/media
    ;;
  kde)
    fetch linuxdeploy-plugin-qt-x86_64.AppImage \
      https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/1-alpha-20250213-1/linuxdeploy-plugin-qt-x86_64.AppImage \
      15106be885c1c48a021198e7e1e9a48ce9d02a86dd0a1848f00bdbf3c1c92724
    qmake=${QMAKE:-$(command -v qmake6 || echo /usr/lib/qt6/bin/qmake)}
    export QMAKE=$qmake
    plugins=$("$qmake" -query QT_INSTALL_PLUGINS)
    # The backend makes its QML from strings at run time, so the plugin
    # can't find its imports: they're listed here for it to deploy, with
    # the desktop style the backend sets (`org.kde.desktop`).
    qml=$(mktemp -d)
    cat > "$qml/imports.qml" <<'EOF'
import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Qt.labs.qmlmodels
import org.kde.kirigami
import org.kde.desktop
Item {}
EOF
    export QML_SOURCES_PATHS=$qml
    # Wayland, as well as X11 (the plugin's default): one plugin from Qt
    # 6.10, `-egl` and `-generic` before.
    wayland=()
    for platform in libqwayland.so libqwayland-egl.so libqwayland-generic.so; do
      [ -e "$plugins/platforms/$platform" ] && wayland+=("$platform")
    done
    EXTRA_PLATFORM_PLUGINS=$(IFS=';'; echo "${wayland[*]}")
    export EXTRA_PLATFORM_PLUGINS
    export EXTRA_QT_MODULES=waylandclient
    # What Qt and Kirigami load by name, which linking doesn't show:
    # Wayland's shell, decorations and GPU buffers (without them Qt goes
    # to XWayland), SVG icons, the Breeze widget style (outside Plasma the
    # backend sets it), Kirigami's platform for the desktop style, and the
    # platform themes: Plasma's, which Plasma asks for
    # (`QT_QPA_PLATFORMTHEME=kde`) for its fonts, icons and dialogs, and
    # the portal's, which Qt picks elsewhere.
    extra=()
    for plugin in wayland-shell-integration wayland-decoration-client wayland-graphics-integration-client \
      iconengines/libqsvgicon.so imageformats/libqsvg.so styles/breeze6.so \
      kf6/kirigami/platform/org.kde.desktop.so \
      platformthemes/KDEPlasmaPlatformTheme6.so platformthemes/libqxdgdesktopportal.so; do
      if [ ! -e "$plugins/$plugin" ]; then
        echo "::warning::appimage.sh: no $plugin (is its package installed?)"
        continue
      fi
      mkdir -p "$(dirname "$appdir/usr/plugins/$plugin")"
      cp -r "$plugins/$plugin" "$appdir/usr/plugins/$plugin"
      extra+=(--deploy-deps-only "$appdir/usr/plugins/$plugin")
    done
    "$tools/linuxdeploy-x86_64.AppImage" --appdir "$appdir" --plugin qt "${extra[@]}"
    # The plugin's hook asks for GTK 2's platform theme on GNOME and Xfce,
    # which isn't in the AppImage; Qt picks the portal's there itself. Ours
    # replaces it (AppRun sources it by name). KDE's HEIF image plugin
    # brings libheif, which loads its decoders from where the build's
    # distribution keeps them: the host's there don't match it, so it
    # looks in the AppImage, which has none (Fedora's libheif has its
    # decoders built in).
    mkdir -p "$appdir/usr/lib/libheif"
    for hook in "$appdir"/apprun-hooks/*qt*.sh; do
      printf '%s\n' "# Written by mitsuami's appimage.sh." \
        'export LIBHEIF_PLUGIN_PATH="$this_dir/usr/lib/libheif"' > "$hook"
    done
    ;;
  *)
    echo "appimage.sh: the toolkit is gtk or kde, not '$toolkit'" >&2
    exit 1
    ;;
esac

LDAI_OUTPUT=$output OUTPUT=$output "$tools/linuxdeploy-x86_64.AppImage" --appdir "$appdir" \
  --desktop-file "$appdir/usr/share/applications/$id.desktop" \
  --icon-file "$appdir/usr/share/icons/hicolor/scalable/apps/$id.svg" \
  --output appimage
