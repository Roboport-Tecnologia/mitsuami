# mitsuami

## Native always wins

Being native on each platform is this project's reason to exist. mitsuami
bends to each platform's native controls, never the other way around:

- Widgets behave, size, animate and respond as the platform's own control
  does. Don't override a native control to make platforms uniform (sizes,
  snapping, step sizes, selection, focus rules).
- Props the app sets are passed through for the platform to use as it uses
  them; where a platform has no equivalent, it ignores them.
- Tests assert what every platform does (relative sizes, directions, events),
  not one uniform number. Where platforms differ, say so in a comment and in
  `docs/ARCHITECTURE.md`.
- A backend may fill in a behaviour its platform lacks when the other
  platforms share it and the platform's own apps build it the same way,
  e.g. GTK scales snap to their step in `change-value`. Say so in
  `docs/ARCHITECTURE.md`.
- The core enforces a rule only when a platform makes the alternative
  impossible, e.g. a `Select` always has an option chosen because GTK's
  drop-down can't show none.

## Layout

- `crates/mitsuami-core`: node tree, props (`widget.rs`), the `Command`
  protocol, layout (Taffy), accessibility tree (`ui.rs`).
- `crates/mitsuami-widgets`: the built-in widgets' builder API.
- `crates/mitsuami-headless`: in-memory backend with fixed metrics; it
  validates the protocol and stands in for a platform in tests.
- Backends: `mitsuami-appkit`, `mitsuami-gtk` (Linux only),
  `mitsuami-winui` (Windows only), `mitsuami-kirigami` (Linux, `qt` feature).
  `mitsuami-wayland` is GTK's and Kirigami's `GpuSurface` on Wayland.
- `crates/mitsuami-test`: the test kit (queries, actions, snapshots, stories).
- `docs/BACKENDS.md` is the backend contract; `docs/ARCHITECTURE.md` has the
  design, the MVP plan (§14) and implementation notes per milestone (§16).

## Adding a widget

Touch every layer: `WidgetKind` and its props in the core (`Prop`,
`affects_measure`, roles, a11y name and value, focus order, `absorb` for its
`Changed` event); the widget in `mitsuami-widgets` (plus a `__tag()` for
`view!`) and the prelude export in `mitsuami`; headless (measure, perform);
`mitsuami-test` (`format.rs` props and wireframe colour, locator actions);
all four backends (create, `set_prop`, measure, perform, `native_state`);
a test suite; a story in `tests/stories.rs`; `docs/BACKENDS.md` (widget,
props, events, actions) and a note in `docs/ARCHITECTURE.md`.

Backend rules the tests enforce:

- `native_state` reads what the native widget shows; after every settle the
  test kit checks it contains every prop the core has (the mirror check).
  Props the platform can't read back are kept on the node.
- Programmatic sets must not emit `Changed`: GTK and WinUI change signals fire
  for them, so guard (GTK mutes events while applying commands, WinUI compares
  with a `shown_*` value). Qt's user-only signals (`toggled`, `activated`,
  `moved`) are used instead.
- `perform` does what assistive technology does, and never opens modal UI
  (menus, pop-ups).

## Testing

- No unit tests: integration tests in `crates/mitsuami/tests`, written with the
  test kit, run headless and natively. Each file is a `[[test]]` with
  `harness = false` in `crates/mitsuami/Cargo.toml` and ends with
  `mitsuami_test::main!();`.
- `cargo test --workspace` runs headless; `MITSUAMI_NATIVE=1 cargo test
  --workspace` runs on this machine's native backend (AppKit on macOS).
- Native snapshots and visual baselines are per machine image
  (`tests/{snapshots,visual}/<backend>/<image>/`). Only CI's images are
  committed; this machine's (`macos-26@2x`) is recorded on first run and
  ignored by git. Don't set `MITSUAMI_IMAGE` locally, or you'd record into
  CI's image directory.
- CI runs only by hand: `gh workflow run test.yml --ref <branch>`. Baselines
  for new stories, on every platform, come from a failed CI run:
  `.github/scripts/accept-snapshots.sh <run id>`.
- Don't run CI for now, for the same reason as visual baselines below:
  development moves too fast. Type-check the other platforms from macOS
  (below) and say in `docs/ARCHITECTURE.md` what only type-checks; the
  maintainer runs CI in a later pass.
- Some input can't be simulated here, e.g. holding a button down: AppKit's
  tracking loops read the physical mouse button, and posting real events
  (`CGEvent`) needs an Accessibility permission this terminal lacks. Build a
  small `swiftc` app in the scratchpad with the variants side by side and
  ask the maintainer to try it (run with `!`). That's how AppKit steppers
  made in code were found not to repeat (`docs/ARCHITECTURE.md`,
  NumberInput).
- Visual stories aren't validated yet: development moves too fast for
  baselines to keep up. Missing or failing visual baselines (stories, and
  the other platforms' captures) are expected; don't treat them as
  regressions, and don't try to fix them. The maintainer will review every
  capture by eye in a later pass. Still add a story for each new widget.

## Checking other platforms from macOS

Type-check (no linking, no running) the backends that can't build here:

```sh
# WinUI
cargo clippy -p mitsuami-winui -p mitsuami --all-targets --target x86_64-pc-windows-msvc
# GTK and Kirigami (DOCS_RS skips the gtk-rs -sys build scripts)
MITSUAMI_SKIP_QT=1 DOCS_RS=1 CARGO_TARGET_DIR=target/linux-check cargo clippy \
  -p mitsuami-gtk -p mitsuami-kirigami -p mitsuami --all-targets \
  --features mitsuami/kde,mitsuami-kirigami/qt,mitsuami-test/kde --target x86_64-unknown-linux-gnu
```

Needs the `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu` rustup
targets. `MITSUAMI_SKIP_QT=1` skips Kirigami's C++ shim; compile it (no
linking) against Homebrew's Qt 6 (`brew install qt`):

```sh
cd crates/mitsuami-kirigami && Q=/opt/homebrew/lib && clang++ -std=c++17 -fsyntax-only \
  -Icpp -F$Q -I/opt/homebrew/include $(for f in QtCore QtGui QtQml QtQuick \
  QtQuickControls2 QtWidgets QtQmlIntegration; do echo -I$Q/$f.framework/Headers; done) \
  -I$(echo $Q/QtGui.framework/Headers/6.*/QtGui) cpp/shim.cpp
```
 Say in `docs/ARCHITECTURE.md` when code was only type-checked.

WinUI bindings are generated: add types and members to
`crates/mitsuami-winui/bindgen/filter.txt`, then
`cargo run --manifest-path crates/mitsuami-winui/bindgen/Cargo.toml`.

## Conventions

- Match the surrounding code's comment density and voice: short, plain
  sentences that say why.
- Commit subjects are imperative and plain ("Show Lists as gtk::ListViews on
  GTK"), with the milestone in parentheses when there is one ("(M7)").
