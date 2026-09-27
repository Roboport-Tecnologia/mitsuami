# mitsuami

Native, declarative, cross-platform UI for Rust: AppKit on macOS, WinUI 3 on
Windows, GTK 4 on Linux, or Qt Quick with Kirigami for KDE Plasma, driven by
one Vue-inspired layer with CSS-style flexbox/grid layout.

Status: **M1 to M5 done**. The AppKit, GTK 4 and WinUI 3 backends run real
apps on macOS, Linux and Windows, and the same tests pass headlessly and
against the native widgets of all three. On Linux, the `kde` feature swaps
GTK 4 for Qt Quick and Kirigami, KDE Plasma's toolkit, which passes the same
tests. The escape hatches work on each:
`platform!` for per-platform code, `NativeView` for any `NSView`, GTK widget,
QML item or XAML element, and custom widgets that are native where the platform has
the control, and built ad hoc from the platform's widgets, drawn or composed
where it doesn't. Apps can be written with `view!` and `#[component]`, and
keep shared state in stores, resources and actions. Writing a backend starts
with [`docs/BACKENDS.md`](docs/BACKENDS.md). The design is in
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

On Windows, mitsuami needs the Windows App Runtime 2.4 or later installed,
and builds with the MSVC toolchain.

```rust
use mitsuami::prelude::*;

fn counter(initial: i32) -> impl View {
    let count = signal(initial);
    Column::new().padding(16).gap(Spacing::Md).children((
        Text::new(move || format!("Count: {}", count.get())).text_style(TextStyle::Title),
        Button::new("Increment").on_click(move || count.update(|c| *c += 1)),
    ))
}
```

## Crates

| Crate | |
|---|---|
| `mitsuami` | Facade and prelude |
| `mitsuami-reactive` | Signals, computed values, effects, ownership, context |
| `mitsuami-core` | Node tree, styles and units, Taffy layout, a11y model, `Show`/`For`, backend contract |
| `mitsuami-widgets` | Built-in widgets |
| `mitsuami-macros` | `view!` and `#[component]` |
| `mitsuami-headless` | In-memory backend with deterministic metrics that validates the protocol |
| `mitsuami-test` | Test runner, a11y queries, actions, assertions, snapshots |
| `mitsuami-appkit` | AppKit backend (macOS) |
| `mitsuami-gtk` | GTK 4 backend (Linux) |
| `mitsuami-kirigami` | Qt Quick and Kirigami backend (Linux, KDE Plasma; the `kde` feature) |
| `mitsuami-winui` | WinUI 3 backend (Windows) |

## Testing

There are no unit tests. Everything is tested through public APIs.

```sh
cargo test --workspace                       # everything, headless
cargo test -p mitsuami --test layout grid    # one suite, filtered
MITSUAMI_NATIVE=1 cargo test                 # the same tests on the native backend
MITSUAMI_SHOW_WINDOWS=1 MITSUAMI_NATIVE=1 cargo test   # …and watch them
MITSUAMI_UPDATE_SNAPSHOTS=1 cargo test       # accept snapshot / visual baseline changes
MITSUAMI_WAIT_MS=5000 cargo test             # longer wait for background work in assertions
MITSUAMI_SKIP_MACHINE_SNAPSHOTS=1 cargo test # skip native snapshots that depend on fonts, OS and scale
MITSUAMI_IMAGE=macos-26 cargo test           # name the machine image native snapshots belong to (CI sets it)
MITSUAMI_NATIVE=1 cargo test -p mitsuami -p mitsuami-kirigami \
  --features mitsuami/kde,mitsuami-test/kde,mitsuami-kirigami/qt   # native tests on Kirigami
```

Native snapshots and visual baselines depend on the machine, so they are
kept per machine image: `tests/{snapshots,visual}/<backend>/<image>/`, where
the image defaults to the OS and its version (`macos-26@2x`). Only CI's are
kept in the repository; a machine's own are recorded on its first run and
ignored by git. When a CI run fails on missing or changed ones, it uploads
them, and `.github/scripts/accept-snapshots.sh <run id>` accepts them.

CI tests only run when started by hand, `gh workflow run test.yml --ref
<branch>`, and before a release (headless only) when a tag is pushed.

Stories (`#[mitsuami_test::story]`) render a view in a given state and
compare a capture with a baseline at each size, in light and dark: see
`crates/mitsuami/tests/stories.rs`.

Tests control time (`app.advance(..)` moves the clock that `sleep` uses)
and answer dialogs through scripted services (`app.services()`), so they
never open real dialogs or touch your clipboard.

Try the example apps with `cargo run -p mitsuami --example <name>`:

- `todos`: components, `view!`, a store, a resource and an action.
- `showcase`: a counter, a form, menus and a dialog, with the builder API.
- `escape_hatches`: custom widgets that are native where the platform has
  the control, and `platform!`.

On Linux, building needs the GTK 4 development files (4.10 or newer), and
native tests need `gtk4-broadwayd`, GTK's in-memory display server: tests
run on a private Broadway display (with desktop portals off), so windows get
their exact sizes and dialogs stay off your desktop. `MITSUAMI_SHOW_WINDOWS=1`
puts them on your display instead.

For KDE Plasma, build with `mitsuami = { version = "…", default-features =
false, features = ["kde"] }` (with `gtk` on too, `kde` wins). It needs the
Qt 6.5+ development files (Qt Quick, Qt Quick Controls, Qt Widgets) and, at
run time, Kirigami 6 and `qqc2-desktop-style`; Breeze gives it Plasma's look
(without it, the controls fall back to Fusion). `platform!` has `kde` and
`gtk` arms to tell the two Linux toolkits apart. Native tests run on Qt's
offscreen platform, with Breeze Light or Dark forced and animations off. The
test kit needs its own `kde` feature: `mitsuami-test = { …, features =
["kde"] }`.

UI test targets use `harness = false` and `mitsuami_test::main!()`, because
native UI has to own the main thread. See `crates/mitsuami/tests/` for
examples.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
