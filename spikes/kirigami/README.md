# Kirigami spike (KDE Plasma)

A throwaway spike that drives Qt Quick Controls in a Kirigami window
imperatively from Rust, the way mitsuami's command protocol would: create,
place, measure, act, observe, capture. It is not a workspace member. The
backend it led to is `crates/mitsuami-kirigami`; what that taught is in
`docs/ARCHITECTURE.md` §15 (Kirigami).

It needs the Qt 6 development files (Quick, Qml, QuickControls2, Widgets),
Kirigami and `qqc2-desktop-style`:

```
cargo run                  # scripted checks offscreen, report, exit
cargo run -- --interactive # the same window on your desktop
XDG_CONFIG_HOME=<dir with a kdeglobals> QT_QUICK_BACKEND=software cargo run
```

The scripted run writes a capture to `$TMPDIR/mitsuami-kirigami-spike.png`
(`SPIKE_CAPTURE=<name>` renames it).

## How it's built

`src/shim.cpp` is a small C API over Qt Quick. Every item is a `QObject*`
created from a line of QML (`QQC2.Button { }`), compiled once per text by one
`QQmlEngine`; props go through `QObject::setProperty`, so the shim knows no
controls. Signals reach Rust through one moc'd `Receiver` per connection
(string-based `connect`, since controls' signals live on private types).
`build.rs` runs moc and compiles the shim with `cc`, finding Qt with
pkg-config. No `cxx-qt`, no CMake.

## Findings (Qt 6.11, Kirigami 6.30, Arch)

- **Structure.** `Kirigami.ApplicationWindow` with one `Kirigami.Page`
  (padding 0) holding a plain `Item` as the content host. Controls are
  parented with `setParentItem` and placed with `setPosition`/`setSize`;
  frames read back exactly. `childItems()` is in stacking order, so
  `Insert { index }` is `stackBefore`. A control's first creation takes 0.4 to 7 ms, later ones 0.27 ms
  once the component is cached; the window takes 170 ms (Kirigami's import).
- **Content size.** The page header (the page title, Kirigami's global
  toolbar) is 46 px, measured as window height minus host height after one
  event pass. Setting the window to content + header gives the host exactly
  the content size, and the host's `widthChanged`/`heightChanged` give
  `WindowResized`. No minimum size is imposed.
- **Measuring is synchronous.** `implicitWidth`/`implicitHeight` are right
  as soon as a control is created, before it's parented or shown, and right
  after a text change, with no polish or event pass (even for the desktop
  style's QStyle-drawn controls). Wrapping text: set `width`, read
  `implicitHeight`. Min-content: `WordWrap` at width 1 gives the longest word
  as `contentWidth` (82 px for "extraordinarily"). `Text.Wrap` breaks words
  there, so min-content needs `WordWrap`.
- **Programmatic changes don't look like user ones.** `toggled` (checkbox,
  switch) and `textEdited` (text field) fire only for the user;
  `checkedChanged`/`textChanged` fire for both. So no muting is needed,
  unlike GTK and WinUI. `accepted` fires for Return only.
- **Accessibility actions work offscreen.** `QAccessible` interfaces exist
  for controls: `Press` on a button clicks it, `Toggle` on a checkbox or
  switch toggles it with `toggled`. Both also move focus to the control (as
  a click does), which AppKit's press doesn't. Disabled controls accept the
  action and do nothing, so the backend must check `enabled` first.
  Accessible names read back empty offscreen: to look at.
- **Real key events.** `QKeyEvent`s sent to the window reach the focused
  item: typing, Backspace, Return (`accepted`) and Space on a button
  (`clicked`) go through Qt's real paths. Better than GTK 4, which needed
  keybinding signals.
- **Focus.** `forceActiveFocus` plus the window's `activeFocusItemChanged`,
  resolved to the nearest item carrying a node id, works offscreen.
- **Tab order.** Qt Quick's chain follows item order within each parent.
  An event filter on the window that handles Tab and Shift+Tab along our
  window-wide order (skipping disabled and hidden items, wrapping) works:
  order [switch, button, field] visited in that order from the checkbox.
- **Scrolling.** `QQC2.ScrollView` wraps a `Flickable`: our content goes
  into `flickable.contentItem`, we set `contentWidth`/`contentHeight` from
  frames, and `contentX`/`contentY` scroll, reporting `contentYChanged`.
  Scroll bars take space from the view unless a policy turns them off.
- **Capture.** `QQuickWindow::grabWindow()` works on the `offscreen`
  platform, synchronously, in under 1 ms. The offscreen platform renders
  with Qt Quick's software backend, so baselines won't depend on the GPU.
- **Test isolation.** `QT_QPA_PLATFORM=offscreen` replaces Broadway: no
  daemon, exact window sizes. Light and dark come from a private
  `XDG_CONFIG_HOME` whose `kdeglobals` carries the color groups inline
  (`[Colors:Window]`, …); Kirigami's theme colors follow it. Fonts don't
  (they come from the Plasma platform theme, absent here), so tests should
  set the app font themselves.
- **Style.** `org.kde.desktop` draws with QStyle, so it needs a
  `QApplication`. Without the Breeze widget style installed (the `breeze`
  package) it falls back to Fusion: right structure and colors, older
  look. CI would install `breeze` and the Kirigami packages.

## Open when the spike ended

All of these are done in the backend, `crates/mitsuami-kirigami` (see
`docs/ARCHITECTURE.md` §15, Kirigami).

- Menus (a `Kirigami.GlobalDrawer` or hamburger `QQC2.Menu` in the page
  toolbar), alerts (`Kirigami.PromptDialog`) and file dialogs
  (`QtQuick.Dialogs.FileDialog`, which goes through the KDE portal).
- Drawn widgets: a `QQuickPaintedItem` subclass with `QPainter`.
- The run loop: Qt's event dispatcher is GLib's on Linux; the tick would be
  a zero-timeout `QTimer`, the thread-safe wake a queued
  `QMetaObject::invokeMethod`.
- Text styles: `Kirigami.Heading` levels for titles, `Kirigami.Theme.smallFont`
  for captions, `fixedWidthFont` for monospace. Button variants: `highlighted`
  for Primary, `flat` for Plain; Destructive has no style in Breeze.
