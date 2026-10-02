# mitsuami

Native, declarative, cross-platform UI for Rust.

You write one UI with a Vue-inspired layer: signals, components and
flexbox/grid layout. Each platform shows it with its own controls: AppKit on
macOS, WinUI 3 on Windows, and GTK 4 on Linux, or Qt Quick with Kirigami for
KDE Plasma. A button is an `NSButton` on macOS, a `gtk::Button` on GNOME, and
it looks, sizes and behaves as that platform's buttons do. mitsuami doesn't
try to make platforms look the same.

```rust
use mitsuami::prelude::*;

fn hello() -> impl View {
    let clicks = signal(0);
    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Text>{move || format!("Clicked {} times", clicks.get())}</Text>
            <Button @click=move || clicks.update(|c| *c += 1)>"Click me"</Button>
        </Column>
    }
}

fn main() {
    App::new().window("Hello", Size::new(320.0, 160.0), hello).run();
}
```

More in [the examples](#examples): one per widget, and a few whole apps in
[`crates/mitsuami/examples`](crates/mitsuami/examples) and
[`examples`](examples).

## Getting started

```toml
[dependencies]
mitsuami = "1.0"

[dev-dependencies]
mitsuami-test = "1.0"
```

The backend is picked by the target OS; there's nothing to configure. Rust
1.95 or newer (edition 2024).

| Platform | Backend | Needs |
|---|---|---|
| macOS 11+ | AppKit | Nothing else |
| Windows 10 1809+ (packaged apps) or 11 | WinUI 3 | The MSVC toolchain, and the Windows App Runtime 2.4 or later installed |
| Linux (default) | GTK 4 | GTK 4.10+ and libadwaita 1.4+ development files |
| Linux (KDE Plasma) | Qt Quick + Kirigami | Qt 6.5+ development files (Qt Quick, Qt Quick Controls, Qt Widgets); at run time Kirigami 6.14+ and `qqc2-desktop-style`, with Breeze for Plasma's look (without it, Fusion) |

For KDE Plasma, turn on the `kde` feature (with `gtk` on too, `kde` wins),
and the test kit's own `kde` feature:

```toml
mitsuami = { version = "1.0", default-features = false, features = ["kde"] }
mitsuami-test = { version = "1.0", features = ["kde"] }
```

## The basics

- **Views** are built with `view!`, or with the builder API it expands to
  (`Button::new("Save").on_click(…)`). `#[component]` makes a function a
  component with typed props.
- **State** is fine-grained: `signal`, `computed` and effects. A component
  runs once; only what reads a signal updates when it changes. Shared state
  lives in stores (`Store`, `use_store`), async data in `resource`, and
  async work in `action`.
- **Control flow**: `Show` and keyed `For`.
- **Layout** is flexbox and grid with CSS semantics (Taffy), in units like
  `px`, `em`, `%` and `fr`, and platform spacing tokens (`Spacing::Md`).
  HiDPI needs nothing from the app.
- **Styling** is semantic, not pixel-level: roles, button styles, text
  styles. The platform decides what they look like.
- **Accessibility**: every widget has a role, name and value. Tests find
  widgets through them.
- **Localization** with [Project Fluent](https://projectfluent.org):
  `locales!("../locales")` builds the app's translations in, and
  `t!("files", count = n)` shows a message in the first of the user's
  languages the app has. Numbers and dates are written by the platform,
  as the user's region writes them, and a right-to-left language mirrors
  the app, native controls and window chrome included.
- **Escape hatches**, when the shared widgets aren't enough:
  - `platform!` picks per-platform code, from one detail to a whole screen,
    while stores stay shared.
  - `.native(tweak(…))` sets something on the native widget directly.
  - `NativeView` embeds any `NSView`, GTK widget, QML item or XAML element.
  - Custom widgets: one definition, native where the platform has the
    control, and composed or drawn where it doesn't.
  - `GpuSurface` is a surface the app presents to with its own GPU API
    (wgpu, Metal, Vulkan, Direct3D), from its own thread.

Built-in widgets: `Text`, `Button`, `ToggleButton`, `TextInput`,
`PasswordInput`, `SearchInput`, `TextArea`, `Checkbox`, `Switch`,
`RadioGroup`, `Slider`, `NumberInput`, `Select`, `Progress`, `Spinner`,
`Separator`, `Image`, `Icon`, `ScrollView`, `List` and `Table` (virtualised), `Group`,
`Sidebar`, `Tabs`, `Toolbar`, `MenuButton`, menus, context menus, tooltips,
windows and dialogs. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) §13
lists the native control each one is on every platform.

## Examples

Each widget has an example that shows it in every variant, with a
playground to try its props:

```sh
cargo run -p mitsuami --example button
```

The others in [`crates/mitsuami/examples`](crates/mitsuami/examples) run the
same way: `checkbox`, `context_menu`, `file_drop`, `group`, `icon`, `image`,
`list`, `measurements`, `menu_button`, `menus`, `number_input`,
`password_input`, `progress`, `radio_group`, `scroll_view`, `search_input`,
`select`, `separator`, `sidebar`, `slider`, `spinner`, `switch`, `table`,
`tabs`, `text`, `text_area`, `text_input`, `toolbar`, `tooltip`, `windows`.

Whole apps:

- `todos`: components, `view!`, a store, a resource and an action.
- `contacts`: ten thousand rows in the platform's own list control,
  filtered by a search field.
- `escape_hatches`: custom widgets that are native where the platform has
  the control, and `platform!`.
- `files`: a simplified Finder on the real file system: sidebar places,
  the platform's list, a preview, menus, rename, trash and drops, with a
  path bar and file icons that are the platform's own where it has them.
- `l10n`: an app in five languages, switched while it runs: Fluent
  messages and plural forms, the platform's numbers and dates, and Arabic
  laid out right to left.

Two are crates of their own, so the workspace doesn't build wgpu:

- [`examples/showcase`](examples/showcase): every example in one window,
  picked from its sidebar.
  `cargo run --manifest-path examples/showcase/Cargo.toml`
- [`examples/gpu-surface`](examples/gpu-surface): a `GpuSurface` presented
  to with wgpu from a thread of its own: a cube in space to spin, and to
  fly around with the pointer captured.
  `cargo run --manifest-path examples/gpu-surface/Cargo.toml`

## Testing your app

`mitsuami-test` drives your UI the way a user and assistive technology do:
find widgets by role and text, click, type, and assert. The same test runs
headless (fast, deterministic metrics) or on the real native widgets.

```rust
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[mitsuami_test::test]
async fn clicking_counts(app: TestApp) {
    app.mount(hello); // the view from the example above
    app.get_by_role(Role::Button, "Click me").click().await;
    app.expect(by_text("Clicked 1 times")).to_be_visible().await;
}

mitsuami_test::main!();
```

Native UI has to own the main thread, so each test file is a target of its
own with `harness = false`:

```toml
[[test]]
name = "hello"
harness = false
```

`cargo test` runs it headless; `MITSUAMI_NATIVE=1 cargo test` runs it on
this machine's native backend. Tests control time (`app.advance(..)` moves
the clock `sleep` uses) and answer dialogs through scripted services
(`app.services()`), so they never open real dialogs or touch your
clipboard. The kit also has tree, layout and wireframe snapshots, and
stories (`#[mitsuami_test::story]`) that capture a view at each size, in
light and dark. [`crates/mitsuami/tests`](crates/mitsuami/tests) has plenty
of examples.

## Crates

| Crate | |
|---|---|
| `mitsuami` | Facade and prelude; picks the backend |
| `mitsuami-reactive` | Signals, computed values, effects, ownership, context |
| `mitsuami-core` | Node tree, styles and units, Taffy layout, a11y model, `Show`/`For`, backend contract |
| `mitsuami-widgets` | Built-in widgets |
| `mitsuami-macros` | `view!` and `#[component]` |
| `mitsuami-headless` | In-memory backend with deterministic metrics that validates the protocol |
| `mitsuami-test` | Test runner, a11y queries, actions, assertions, snapshots, stories |
| `mitsuami-appkit` | AppKit backend (macOS) |
| `mitsuami-gtk` | GTK 4 backend (Linux) |
| `mitsuami-kirigami` | Qt Quick and Kirigami backend (Linux, KDE Plasma; the `kde` feature) |
| `mitsuami-linux` | `GpuSurface` for GTK and Kirigami: a Wayland subsurface or an X11 child window |
| `mitsuami-winui` | WinUI 3 backend (Windows) |

## Working on mitsuami

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md): the design, how each
  widget and window feature works on every platform (§13, §14), what each
  backend taught (§15), and the plan (§16).
- [`docs/BACKENDS.md`](docs/BACKENDS.md): the backend contract. Writing a
  backend starts there.
- [`CLAUDE.md`](CLAUDE.md): the project's rules, including what adding a
  widget touches and how to type-check the other platforms from macOS.

There are no unit tests: everything is tested through public APIs, in
[`crates/mitsuami/tests`](crates/mitsuami/tests).

```sh
cargo test --workspace                       # everything, headless
cargo test -p mitsuami --test layout grid    # one suite, filtered
MITSUAMI_NATIVE=1 cargo test --workspace     # the same tests on the native backend
MITSUAMI_SHOW_WINDOWS=1 MITSUAMI_NATIVE=1 cargo test   # …and watch them
MITSUAMI_UPDATE_SNAPSHOTS=1 cargo test       # accept snapshot / visual baseline changes
MITSUAMI_WAIT_MS=5000 cargo test             # longer wait for background work in assertions
MITSUAMI_SKIP_MACHINE_SNAPSHOTS=1 cargo test # skip native snapshots that depend on fonts, OS and scale
MITSUAMI_NATIVE=1 cargo test -p mitsuami -p mitsuami-kirigami \
  --features mitsuami/kde,mitsuami-test/kde,mitsuami-kirigami/qt   # native tests on Kirigami
```

On Linux, native tests need `gtk4-broadwayd`, GTK's in-memory display
server: they run on a private Broadway display (with desktop portals off),
so windows get their exact sizes and dialogs stay off your desktop.
`MITSUAMI_SHOW_WINDOWS=1` puts them on your display instead. On Kirigami,
they run on Qt's offscreen platform, with Breeze Light or Dark forced and
animations off.

Native snapshots and visual baselines depend on the machine, so they are
kept per machine image: `tests/{snapshots,visual}/<backend>/<image>/`, where
the image defaults to the OS and its version (`macos-26@2x`). Only CI's are
kept in the repository; a machine's own are recorded on its first run and
ignored by git. Don't set `MITSUAMI_IMAGE` locally: it names CI's image.
When a CI run fails on missing or changed ones, it uploads them, and
`.github/scripts/accept-snapshots.sh <run id>` accepts them.

CI only runs when started by hand, `gh workflow run test.yml --ref
<branch>`, and before a release (headless only) when a tag is pushed.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
