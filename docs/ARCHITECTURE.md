# Architecture

mitsuami is native, declarative, cross-platform UI for Rust. Apps are written once, against a Vue-inspired layer. Each platform shows them with its own toolkit: AppKit on macOS, WinUI 3 on Windows, and on Linux GTK 4, or Qt Quick with Kirigami for KDE Plasma.

This document is the design and what building it taught us. §1–§12 are the design: the model, the contract with backends, the escape hatches, the API and testing. §13 and §14 go through the widgets, the windows and the app shell, one section each, with how every platform does it and why. §15 has what each backend taught us that isn't about one widget. §16–§19 are the plan, the decisions, the risks and the open questions. Writing a backend? [BACKENDS.md](BACKENDS.md) is the step-by-step guide.

**Status:** the milestones of the MVP plan are done (§16), and the crates are released as 0.0.1. Development moves fast, and not everything has run on every platform: each section of §13 and §14 ends by saying where it has run natively, and what is only type-checked.

**Paths.** `examples/` and `tests/` mean `crates/mitsuami/examples/` and `crates/mitsuami/tests/`, unless they're `examples/showcase` and `examples/gpu-surface`, which are crates of their own at the top of the repository.

---

## 1. Goals and non-goals

**Goals**

- **Native widgets only:** AppKit (macOS), WinUI 3 (Windows), GTK 4 (Linux), or Qt Quick with Kirigami for KDE Plasma (Linux, a cargo feature).
- **Native always wins.** Widgets behave, size, animate and respond as the platform's own controls do. We don't bend a platform to make platforms uniform.
- **One shared declarative layer,** Vue-inspired: components that run once, `signal`/`computed`/`watch`, props, events and slots, provide and inject.
- **Layout owned by us:** flexbox and grid with CSS semantics, in abstract units (`px`, `em`, `rem`, `%`, `vw`, `vh`, `fr`) and platform spacing tokens.
- **HiDPI works** without app code doing anything special.
- **Accessibility-ready from day one:** every node carries semantics, and tests find controls through them.
- **First-class escape hatches** (§7):
  1. platform-specific **screens** that share the same logic;
  2. raw **native views** embedded in the shared tree;
  3. **custom widgets** with shared logic and one render per platform;
  4. raw **settings** of a built-in widget.
- **Platform backends are detached** from the core, behind a narrow, data-oriented contract (§6).
- **Testing is first class:** integration and end-to-end tests with one API, visual regression, and a testing toolkit shipped to app authors (§12).

**Non-goals (for now)**

- Pixel-identical rendering across platforms. We want a *native feel*, not sameness.
- Arbitrary CSS styling of native controls. Visual styling is semantic (roles, styles, text styles), not pixel-level.
- Mobile targets. The architecture shouldn't rule them out, but they aren't on the roadmap.

### How native wins

These rules decide most questions in §13 and §14:

- **Props the app sets are passed through** for the platform to use as it uses them; where a platform has no equivalent, it ignores them. A prop is sent only if the app gives it, so each platform's default stays its own.
- **A widget or option is shared only if every platform has it.** What only some platforms have is a *tweak*: a raw setting on the native control (§7.4). What one platform lacks is the app's to build, as a custom widget (§7.3).
- **A backend may fill in a behaviour its platform lacks** when the other platforms share it and the platform's own apps build it the same way: GTK scales snap to their step in `change-value`, as GTK apps do (§13, Slider).
- **The core enforces a rule only when a platform makes the alternative impossible:** a `Select` always has an option chosen, because GTK's drop-down can't show none.
- **Tests assert what every platform does** (relative sizes, directions, events), not one number. Where platforms differ, the tests and this document say so.

---

## 2. Big picture

```
┌──────────────────────────────────────────────────────────────────────┐
│  App code                                                            │
│  ┌───────────────┐   ┌──────────────────────────────────────────┐    │
│  │ Domain logic  │   │ Views (components)                       │    │
│  │ plain Rust,   │◄──│ shared views + optional per-platform ones│    │
│  │ no UI deps    │   │ view! / builder API                      │    │
│  └───────────────┘   └──────────────────────────────────────────┘    │
│          ▲ stores / composables (use_xxx) bridge the two             │
├──────────┼───────────────────────────────────────────────────────────┤
│  mitsuami-core                                                       │
│  reactive runtime · component model · node tree (source of truth)    │
│  style + unit resolution · layout (Taffy) · a11y tree · focus ·      │
│  event dispatch · scheduler (batching, flush)                        │
├──────────────────────── Backend contract ────────────────────────────┤
│   Commands (data) ──►                       ◄── Events (data)        │
│   measure(id, request) (sync query)         run loop / services      │
├─────────────┬─────────────┬─────────────┬─────────────┬──────────────┤
│ appkit      │ winui       │ gtk         │ kirigami    │ headless     │
│ objc2       │ windows-rs, │ gtk4-rs,    │ C++ shim    │ in memory,   │
│             │ WinAppSDK   │ libadwaita  │ over QML    │ for tests    │
└─────────────┴─────────────┴─────────────┴─────────────┴──────────────┘
```

"Backend" means two different things here, and they stay separate:

- **Platform backend:** the AppKit, WinUI, GTK or Kirigami renderer. It is detached from the core by the command and event contract (§6).
- **App backend:** the application's domain logic. It is plain Rust that knows nothing about UI. Every platform's views bind to the same logic (§9).

### Crates

| Crate | Responsibility |
|---|---|
| `mitsuami-reactive` | Signals, computed values, effects, watchers, scopes and ownership, batching. Single-threaded, no UI knowledge. |
| `mitsuami-core` | Node tree, components, widget kinds and props, styles and units, layout (Taffy), the a11y model, focus, events, the scheduler, the backend contract. |
| `mitsuami-widgets` | The built-in widgets' builder API: typed props, events and a11y defaults. Platform-free. |
| `mitsuami-macros` | `#[component]` and `view!`, sugar over the builder API. (`platform!` is a `macro_rules!` in `mitsuami`.) |
| `mitsuami-appkit`, `mitsuami-winui`, `mitsuami-gtk`, `mitsuami-kirigami` | The backends, with their `NativeRender` traits for custom widgets, native views and tweaks. |
| `mitsuami-linux` | `GpuSurface` for the Linux backends: a Wayland subsurface or an X11 child window, pointer locks and a shortcuts inhibitor (§13.26). Only a `NoSurface` elsewhere. |
| `mitsuami-headless` | In-memory backend with deterministic measurement and wireframe rendering. It validates the protocol, and it's the default backend for tests. |
| `mitsuami-test`, `mitsuami-test-macros` | The testing toolkit (§12), with `#[mitsuami_test::test]` and `#[mitsuami_test::story]`: runner, a11y queries, actions, assertions, fake clock and services, snapshots, stories, visual capture and diff. |
| `cargo-mitsuami` | The CLI: `cargo mitsuami visual` runs the native tests, `visual review` shows what changed, `visual accept` accepts it. Later: scaffolding and a gallery. |
| `mitsuami` | The facade. Re-exports everything and picks the backend by `cfg(target_os)`: AppKit on macOS, WinUI 3 on Windows, and on Linux GTK 4, or Qt Quick and Kirigami with the `kde` feature. A toolkit is only used on its own platform. |

---

## 3. Rendering model: fine-grained reactivity over a retained tree

This is Vue 3's Vapor mode, closer to Solid or Leptos than to a virtual DOM:

- A component function runs **once** (Vue's `setup()`). It creates signals and returns a view description.
- Building the view creates **nodes** in a retained tree owned by `mitsuami-core`. Each node gets a stable `NodeId`.
- Every dynamic prop is an **effect** bound to one `(NodeId, Prop)` pair. When a signal changes, only that prop is sent to the backend again. Nothing diffs whole subtrees.
- Structure changes through control-flow primitives: `Show` (`v-if`, whose branches are any children: several are a fragment, laid out in the parent as if written there) and keyed `For` (`v-for`), and the platform-virtualised `List` (§4). `Switch`/`Match` and `Dynamic` (`<component :is>`) are planned; the first needs another name, since `Switch` is a widget.

Why not a virtual DOM? Native widgets are expensive to create and have state of their own (focus, selection, scroll position, IME). Fine-grained updates change only what changed, and never recreate a widget by accident.

### The node tree is the source of truth

A node, simplified from `crates/mitsuami-core/src/ui/mod.rs`:

```rust
struct Node {
    kind: WidgetKind,          // Window, Container, Text, Button, …, Custom(&'static str), Native, Fragment
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    native_parent: Option<NodeId>,   // fragments (Show, For) are spliced out of the native tree
    native_children: Vec<NodeId>,
    props: Vec<Prop>,
    style: Style,              // layout, resolved against metrics and the viewport when laid out
    a11y: A11yProps,           // always present (§8)
    handlers: Vec<Handler>,
    taffy: Option<taffy::NodeId>,
    frame: Rect,               // last layout, logical units, relative to the native parent
    // … and per-kind state: a window's size and how it follows its content,
    // a scroll offset, a list's row width, a tab strip's size
}
```

Layout, accessibility, focus order, hit testing for drawn widgets and tests all read this tree. The native tree is a **mirror** of it, and tests check that it is (§12).

### One tick

1. Something happens: a native event, a timer, or a task completes on the UI thread.
2. Handlers run and change signals. Changes are **batched**.
3. Effects flush. Each queues a `Command` (prop updates, inserts, removals) and marks layout dirty where needed.
4. Taffy lays out what's dirty, calling into the backend to **measure** leaf widgets.
5. Frames that changed become `SetFrame` commands.
6. The backend applies the commit, within one run-loop turn: structure and props in one `apply` call, then the frames in a second (§6).

The backend drives ticks from its platform's run loop (§6).

---

## 4. Layout and units

### We own layout; native widgets are positioned absolutely

This is the React Native and Yoga model:

- Every container (`Container`, and `Row`, `Column` and `Grid`, which make one) is a thin native **layout host** that does no layout of its own:
  - AppKit: a flipped `NSView` subclass.
  - WinUI: a `Canvas`. A custom `Panel` would need composable-type subclassing from Rust, which is hard.
  - GTK: a `gtk::Widget` subclass whose `size_allocate` places children at our frames.
  - Kirigami: a plain QML `Item`, its children at `x`, `y`, `width` and `height`.
- Leaf widgets report their **intrinsic size** through `measure(id, request)`, with the known sizes and the space available (a definite width, min-content or max-content):
  - AppKit: `fittingSize` / `intrinsicContentSize`, plus `preferredMaxLayoutWidth` for wrapping text.
  - WinUI: `UIElement.Measure` and `DesiredSize`, only once the element is in the live tree, with the frame's `Width`/`Height` lifted to Auto for the call (§15, WinUI).
  - GTK: `gtk_widget_measure`.
  - Kirigami: `implicitWidth`/`implicitHeight` (§15, Kirigami).
- [Taffy](https://github.com/DioxusLabs/taffy) computes flexbox, grid and block layout, using those sizes for leaves. It caches measurements per node and constraints; a prop that `affects_measure` marks the node dirty.

Pros: the same layout semantics on every backend, the CSS mental model, and one layout engine to test headless. Cons: we give up native auto-layout, and measuring crosses into native code synchronously.

**Known gap: min-content text** falls back to max-content on AppKit and WinUI, so text never shrinks below one line inside flex rows there. It still wraps under a definite width (columns, fixed widths). GTK and Kirigami measure it properly.

### Scroll views and lists

`ScrollView` is the exception at the edges: the native scroll container scrolls, and we lay out its content. As in CSS, a scroll view's natural size is its content's, so its siblings need `.shrink(0.0)` to keep their size. Window-coordinate frames, visibility (clipped by enclosing scroll views) and `scroll_into_view` are computed in the core, so all backends agree.

`List` goes further: it is the platform's list control (`NSTableView`, `ListView`, `gtk::ListView`, QML `ListView`). Virtualising lists is a solved problem, and the platform solves it: it scrolls, decides which rows to realise (prefetching around the view), recycles them, and owns the selection. We build and lay out what's in the rows:

- **The platform decides which rows exist.** When it realises a row, the backend reports `RowShown(key)` and the core mounts that row, in a host `Container` carrying its `RowKey`; when it lets one go, `RowHidden(key)`, and the core disposes it. The data is just the keys (`Prop::Rows`).
- **The core lays out each mounted row** on its own, at the width the list gives its rows (`RowWidth`), and sends its size; the platform makes the row that high and places it. Where rows are is the platform's: the core reads a row's position back (`native_state`) for frames, visibility and the a11y tree, and scrolls to a row with `ScrollToRow`, which works for rows that aren't mounted.
- **Rows are keyed** like `For`'s: a data change keeps the mounted rows whose keys stay, with their state.
- **Selection and activation are the platform's:** backends report `Changed(Rows)` and `RowActivated`; the app binds the selected keys (`List::selected`) and handles `on_activate`.

§13 (List) has how each platform does it.

### Units

```rust
enum Length {
    Px(f32),        // logical px = DIP = point, not physical pixels
    Em(f32),        // relative to the node's inherited font size
    Rem(f32),       // relative to the platform's body font size (follows the user's text size)
    Percent(f32),   // of the containing block
    Vw(f32), Vh(f32), Vmin(f32), Vmax(f32),  // of the window's content area
    Fr(f32),        // grid tracks only
    Token(Spacing), // platform spacing: Spacing::{None, Xs, Sm, Md, Lg, Xl}
    Auto,
}
```

- **HiDPI comes free.** Every toolkit already works in logical units (points, effective pixels, GTK's logical pixels, Qt's device-independent pixels). We never touch physical pixels, and a scale change needs no relayout.
- `em`, `rem`, `vw`, `vh` and tokens are resolved in the core before styles reach Taffy, which knows lengths, percentages, `fr` and auto. When anything they depend on changes (a window's size, the text size), the core resolves the styles again.
- **Tokens give platform spacing.** `Spacing::Md` is 8 pt on macOS and 12 on WinUI and GNOME; the backend gives the values in its `PlatformMetrics`.
- Ergonomics: `16.px()`, `1.5.em()`, `50.pct()`, `100.vw()`, `1.fr()`, `Spacing::Md`.

### Window sizes

`WindowSize` says how a window's content area is sized: `Fixed(size)`, or its height from its content, since control heights differ per platform and the same content adds up to a different height on each:

- **`FitHeight(width)`** lays the window out once at max-content height and sends that height through the ordinary `SetWindowSize`. After its first layout it's an ordinary window, which the user can resize and which doesn't jump when content comes and goes.
- **`FollowHeight(width)`** keeps following the content's height, and the user can't change it; **`FollowHeightUntilResized(width)`** follows until the user changes the height.

§14 (Window size) has how each platform holds a height. Platforms size windows in physical pixels, so the size they report back can differ from the one asked for by a fraction of a point.

### Responsive and adaptive

- `use_viewport()` is a signal of the window's content size, like media queries.
- `use_size(node_ref)` is a signal of a node's size, like container queries: `let panel = node_ref();` and `.node_ref(panel)` on the node.
- Both are the core's frames, so they need nothing from backends. Text and controls measure per platform, so a breakpoint on a control's size falls at a different width on each.
- Sizes are reported in the same turn: after each commit, `Ui::tick` sets the sizes that changed and commits again, so a view that switches at a breakpoint is laid out with its new branch before the run loop sleeps, and the old one never shows. A view whose size flips with its own size never settles: after 8 reports in a turn the rest wait for the next, as browsers' `ResizeObserver` does, so the run loop goes on (`a_view_that_never_settles_doesnt_hold_the_run_loop`).
- A `NodeRef` follows its node: `Show` and `For` building it again set it again, and it's `None` while nothing is built. Sizes are zero until the first layout.
- Not yet: a node's position (its frame in the window moves with scrolling, which isn't a layout).
- `tests/measurements.rs` runs on every backend and headless. `examples/measurements.rs` has a sidebar that moves at a breakpoint, and cards in as many columns as their panel has room for.

The platform is chosen at compile time with `platform!` (§7.1).

---

## 5. Styling

A style has two halves.

**Layout** applies to every node, as builder methods: `width`, `height`, `min_*`, `max_*`, `size`, `aspect_ratio`, `padding*` and `margin*` (with logical `_start`/`_end` edges), `grow`, `shrink`, `basis`, `align_self`, `justify_self`, `absolute()` with `top`, `bottom`, `start` and `end`, `grid_column`, `grid_row`, `direction` (left-to-right or right-to-left, §8) and `hidden`. Containers add `flex_direction`, `gap` (`row_gap`, `column_gap`), `align`, `justify`, `wrap`, and a grid's `columns` and `rows`. Every setter but `wrap`, `columns` and `rows` takes a literal, a signal or a closure, and `style_with` edits several fields reactively. `hidden` is a flag of its own, so un-hiding restores the node's `display` (a grid stays a grid).

**Visual** styling is semantic:

- **Text styles:** `TextStyle::{LargeTitle, Title, Headline, Body, Callout, Caption, Monospace}`, mapped to `NSFont.preferredFont(forTextStyle:)`, WinUI's type ramp, GNOME's style classes and Kirigami's heading and theme fonts.
- **Text options** every platform's label has: a colour, a weight over the style's (`FontWeight::{Regular, Medium, Semibold, Bold}`), italics, and an alignment (`TextAlign::{Start, Center, End}`, which the core resolves to left or right for the text's direction). No font family or point size: they'd bypass the type ramp and the user's text size (§13, Text).
- **Semantic colours** (`Color::Label`, `SecondaryLabel`, `Accent`, `Separator`, `Error`, `Warning`, `Success`, `ControlBackground`, `WindowBackground`) that follow dark mode, high contrast and the accent. `Color::Rgba` is a fixed colour, allowed on text, icons and in drawn widgets, deliberately not on other native controls.
- **Button roles and styles:** `ButtonRole::{Normal, Default, Cancel, Destructive}` (what the button does: Return clicks the default one, Escape the cancel one, where the platform does that) and `ButtonStyle::{Automatic, Bordered, Borderless}` (how it's drawn), each mapped the platform's way (§13, Button).
- Past the semantic props, a widget's `.native(tweak)` sets raw platform settings (§7.4).

Planned: a background, border, corner radius and opacity on containers, which are plain views.

---

## 6. Backend contract

> Implementing a backend? The step-by-step guide is [BACKENDS.md](BACKENDS.md).

The core speaks **only in `NodeId`s and plain data**. Each backend keeps its own map from `NodeId` to native handle, so the core has no generic parameters and no `dyn Any` handles.

```rust
// crates/mitsuami-core/src/{command,backend,services}.rs, abridged.
pub enum Command {
    Create        { id: NodeId, kind: WidgetKind, props: Vec<Prop> }, // zero frame; initial props, reactive ones too
    SetProp       { id: NodeId, prop: Prop },
    Insert        { parent: NodeId, child: NodeId, index: usize },    // a ScrollView has exactly one child
    Remove        { parent: NodeId, child: NodeId },
    Destroy       { id: NodeId },                  // every native node of a removed subtree, children first
    SetFrame      { id: NodeId, frame: Rect },     // parent-relative, logical units; never for windows
    SetA11y       { id: NodeId, a11y: A11yProps },
    SetWindowSize { id: NodeId, size: Size },      // the content area
    SetFocusOrder { window: NodeId, order: Vec<NodeId> },  // Tab order, owned by the core
    ScrollTo      { id: NodeId, offset: Point },   // already clamped; the backend reports Scrolled
    Focus         { id: NodeId },
    SelectText    { id: NodeId, range: Range<usize> }, // after its field's Focus; scalar values, on grapheme boundaries
    ScrollToRow   { id: NodeId, row: RowKey },     // lists: the platform scrolls to a row
}

pub enum UiEvent {   // backend → core, through the EventSink
    Click, Changed(EventValue), Submit, Search(String), FocusIn, FocusOut, Scrolled(Point),
    RowShown(RowKey), RowHidden(RowKey), RowActivated(RowKey), RowWidth(f32), ColumnWidths(Vec<f32>),    // lists and tables
    WindowResized(Size), WindowCloseRequested,
    FullScreenChanged(bool), MaximizedChanged(bool), SidebarShownChanged(bool),  // the user's, absorbed as props
    MetricsChanged, Remeasure,
    ContextMenuItem(u32), MenuItem(u32), Key(Shortcut),
    DropHover(bool), FilesDropped(Vec<PathBuf>),
    SurfaceReady(SurfaceHandle), SurfaceResized(SurfaceSize), SurfaceInput(SurfaceInput),
    PointerLockEnded, KeyboardGrabEnded,                                          // GPU surfaces
    Pointer(PointerEvent),  // drawn custom widgets (§7.3)
    Custom(AnyValue),       // custom widgets and native views, their own event types
}
// EventValue: Text(String), Bool(bool), Number(f64), Rows(Vec<RowKey>), Index(usize)

pub trait Backend {
    fn init(&mut self, events: EventSink);
    fn metrics(&self) -> PlatformMetrics;   // scale, spacing, font sizes, dark mode, contrast, motion, tab and group insets
    fn group_insets(&self, id: NodeId) -> Option<Insets> { None }  // a group or tab view whose box isn't the metrics'
    fn tab_insets(&self, id: NodeId) -> Option<Insets> { None }
    fn apply(&mut self, batch: &[Command]);
    fn measure(&mut self, id: NodeId, request: MeasureRequest) -> Size;
    fn services(&self) -> Box<dyn Services>;  // clipboard, dialogs, menus (tests replace it with a fake)
    fn set_app_info(&mut self, info: &AppInfo);  // the app's id, name and icon (§14)

    // For tests and assistive technology, part of the contract from day one.
    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError>;   // act on the native control
    fn synthesize(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError>; // keys, scrolls, drags
    fn native_state(&self, id: NodeId) -> Option<NativeState>;  // what the widget shows: props, frame, children, focus
    fn capture(&mut self, id: NodeId, reply: Reply<Result<Image, CaptureError>>);  // offscreen screenshot
}

pub trait Services {
    fn clipboard_text(&mut self, reply: Reply<Option<String>>);
    fn set_clipboard_text(&mut self, text: &str, reply: Reply<Result<(), ServiceError>>);
    fn alert(&mut self, parent: Option<NodeId>, alert: &Alert, reply: Reply<usize>);   // never blocks
    fn open_file(&mut self, parent: Option<NodeId>, request: &OpenFile, reply: Reply<Option<Vec<PathBuf>>>);
    fn save_file(&mut self, parent: Option<NodeId>, request: &SaveFile, reply: Reply<Option<PathBuf>>);
    fn set_menu(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>);
}
```

Anything a platform might complete later is **reply-based**, even where AppKit answers at once: `capture`, the clipboard, dialogs. `measure`, `native_state` and `metrics` stay synchronous; see [BACKENDS.md §16](BACKENDS.md#16-sync-and-async-in-the-contract).

A platform's **run loop** drives the `Ui` through a few hooks:

- **`Ui::tick()`** runs ready tasks and due timers, dispatches events and commits, repeating until idle. The loop calls it before it sleeps.
- **`Ui::set_commit_scheduler`** and **`Ui::set_waker`** (thread-safe) make the loop turn when something changes.
- **`Ui::time_to_next_timer()`** says when to wake up for `sleep`.

Each platform's own mechanism: a `CFRunLoopObserver` and one re-armed `CFRunLoopTimer` in common modes on AppKit; a GLib idle source and timeout on GTK; Qt's event dispatcher's `aboutToBlock` and a single-shot `QTimer` on Kirigami; on WinUI a message loop of our own, with ticks also queued on the `DispatcherQueue`, which runs inside modal loops. [BACKENDS.md §15](BACKENDS.md#15-the-run-loop) has the details.

What the contract gives us:

- **Serializable and inspectable.** Commands can be logged, snapshot-tested and replayed, and could feed a devtools inspector later.
- **A trivial headless backend.** It makes layout, trees and accessibility testable without a display, and it's the default for tests (§12).
- **Controlled inputs without feedback loops** (`v-model`): the backend emits `Changed(text)`, the core updates the signal, and the effect sends `SetProp` back only if the value differs from what the widget already holds. The caret, selection and IME composition survive.
- **Programmatic sets never report.** Setting a prop must not emit `Changed`: GTK and WinUI change signals fire for them, so their backends guard (GTK mutes events while applying commands, WinUI compares with the value it showed); Qt's user-only signals (`toggled`, `activated`, `moved`) are used instead.
- **Threading:** all UI work runs on the main thread, and the reactive runtime is `!Send`.
  - `spawn_local` runs futures on the UI thread.
  - `spawn_blocking` runs work on another thread and resumes the task on the UI thread, through standard `Waker`s that call the run loop's thread-safe waker.
  - `sleep` uses the `Ui`'s clock, which tests replace with a manual one (`app.advance(…)`).
- **Scopes:** tasks and event handlers run in the reactive scope of the component that created them, so `inject`, `spawn_local` and `sleep` work inside them, and disposing the component cancels its tasks.
- **Order in a batch:** a node is created with its initial props, reactive ones included, before it's inserted; frames come in a second `apply` after structure and props, since layout measures widgets that must exist; focus requests come at the end of the batch's structure, once the node is in a window where a toolkit can focus it.

---
## 7. Escape hatches

For when the shared widgets aren't enough. `examples/escape_hatches/` is one screen for every platform with three custom widgets (§7.3); `tests/escape_hatches.rs` covers the rest, native views included.

### 7.1 Platform-specific screens with shared behaviour

Logic lives in **composables** (Vue's `useXxx`) or **stores** (§9). Views are thin, so writing two costs little.

```rust
// shared, platform-agnostic: signals and actions, one instance per app
#[derive(Clone, Copy)]
pub struct Review { pub stars: Signal<u8>, pub comment: Signal<String>, /* … */ }
impl Store for Review { fn create() -> Review { /* … */ } }

// every screen: let review = use_store::<Review>();
pub fn review_screen() -> impl View {
    platform! {
        macos => macos::review_screen(),   // trailing labels, NSStepper, button at the trailing edge
        _     => shared::review_screen(),  // the Windows and Linux version
    }
}
```

- `platform!` is compile-time (`cfg`), so code for other platforms is never compiled into the binary. Arms may have different types.
- Arms are `macos`, `windows`, `linux`, several joined with `|`, and a final `_`. The first matching arm wins.
- On Linux, `gtk` and `kde` name the toolkit, settled when `mitsuami` is built (its `kde` feature); `linux` matches either.
- Without a `_` arm, building for a platform no arm names fails, so a missing screen can't ship by accident.
- Per-platform view files follow a convention: `review/mod.rs`, `review/macos.rs`, `review/shared.rs`. The shared screen is compiled everywhere so it can be tested everywhere.
- Capability predicates on arms (`macos if has(…)`) wait for capabilities (§11).

### 7.2 A native view inside the shared tree

```rust
NativeView::appkit(|cx: &mut AppKitCx| {
    let stepper = NSStepper::new(cx.mtm());
    let emitter = cx.emitter();
    cx.on_action(&*stepper, move |s| emitter.emit(s.doubleValue().round() as u8));
    stepper
})
.update(review.stars, |stepper, stars| stepper.setDoubleValue(*stars as f64))  // applied again when stars changes
.on_event(move |stars: &u8| review.rate(*stars))
.measure(|view, request| /* optional; intrinsicContentSize by default */)
.a11y_label("Stars")
```

- It takes part in layout, accessibility and events like any other node. The core sees `WidgetKind::Native`.
- The factory and the current value of every `update` travel as one `Prop::Native` payload (an `Opaque`: compared by identity, printed as its label). When any value changes, the payload is sent again and every update applied again. One payload, because a factory prop and update props sharing a key had the first update replace the factory in `Create`.
- Native callbacks never touch signals directly; they `emit` events that are queued and dispatched on the next turn, like any native event.
- Accessibility actions (`Activate`, `Increment`, `Decrement`) go to the view's accessibility element, as a screen reader's would: an `NSStepper` isn't an accessibility element, its cell is, so the backend walks down to it.
- Headless tests show a native view as an empty box sized by its styles.
- Each backend has one: `NativeView::appkit`, `NativeView::gtk`, `NativeView::qml` and `NativeView::xaml`. `mitsuami::appkit` re-exports `objc2`, `objc2_app_kit` and `objc2_foundation` at the backend's versions, `mitsuami::gtk::gtk` the `gtk4` bindings, and `mitsuami::winui::bindings` the XAML ones.

### 7.3 Custom widgets: shared logic, one render per platform

Custom widgets come in three tiers. Pick the lowest that works:

1. **Composed** from existing widgets. It runs everywhere, and it's the tier for what the canvas can't draw (text, editing). A plain component works; as a custom widget's render (`Renderer::composed`), it keeps the widget's props and events, so it can stand in for a native render.
2. **Drawn:** a shared `Drawn` implementation with a small 2D API (`Canvas`: fill and stroke rects, rounded rects, ellipses and paths) in semantic colours. It runs everywhere and follows dark mode and the accent, but isn't native.
3. **Native per platform:** one shared definition plus one render per platform.

```rust
// Shared definition, platform-free.
pub struct Rating;
impl CustomWidget for Rating {
    const NAME: &'static str = "Rating";
    type Props = RatingProps;                 // { value: u8, max: u8, editable: bool }
    type Event = RatingEvent;                 // Changed(u8)
    fn a11y(p: &RatingProps) -> A11yProps {   // semantics are shared
        A11yProps::new(Role::Slider).label("Rating").value(format!("{} of {}", p.value, p.max))
    }
    fn action(p: &RatingProps, action: &A11yAction) -> Option<RatingEvent> { /* Increment → Changed(value + 1) … */ }
}

// Tier 2, shared too.
impl Drawn for Rating {
    fn measure(p: &RatingProps, request: &MeasureRequest, metrics: &PlatformMetrics) -> Size;
    fn draw(p: &RatingProps, canvas: &mut Canvas);
    fn pointer(p: &RatingProps, size: Size, event: &PointerEvent) -> Option<RatingEvent>;
}

// Tier 3: one impl per platform, each under cfg, e.g. rating/macos.rs.
#[cfg(target_os = "macos")]
impl NativeRender for Rating {                 // the trait is mitsuami-appkit's
    type View = NSLevelIndicator;
    fn create(p: &RatingProps, cx: &mut AppKitCx) -> Retained<NSLevelIndicator>;
    fn update(v: &NSLevelIndicator, old: &RatingProps, new: &RatingProps);
    fn measure(v: &NSLevelIndicator, p: &RatingProps, request: &MeasureRequest) -> Option<Size> { None } // None: intrinsicContentSize
    fn read(v: &NSLevelIndicator, p: &RatingProps) -> RatingProps;  // read back, for the mirror check
    // and an optional perform, for accessibility actions
}

// Which render each platform uses.
impl Render for Rating {
    fn renderer() -> Renderer<Self> {
        platform! {
            macos => mitsuami::appkit::native::<Self>().with_drawn(),
            _ => Renderer::drawn(),
        }
    }
}
```

- **A usage site is the same everywhere:** `Rating::view(move || RatingProps::new(stars.get())).on_event(…)`, or `<Rating props=… @event=…/>` (§10). Props that don't change are passed as they are once their type derives `IntoValue` (`Dot::view(DotProps { … })` in `tests/macros.rs`): a blanket `IntoValue<T> for T` would overlap the closure impl, so each props type opts in, and `IntoValue`'s compile error says so. `.drawn()` and `.composed()` pick those renders where a native one exists.
- **A widget native on one platform stands in on the others.** The renderer uses the first render it has: native, then drawn, then composed. A native render is either the platform's own control (`native::<W>()`) or built **ad hoc** from the platform's widgets the way its apps build it (`ad_hoc::<W>()`), so it still looks at home. `Renderer::is_native()` is true only for the platform's own control, so a screen can be honest about it.
- **The `escape_hatches` example** has three:
  - a lock: native on GTK (`GtkLockButton`) and Kirigami (Qt's `DelayButton`, which acts once held), composed elsewhere;
  - a rating: native on macOS (`NSLevelIndicator`) and Windows (`RatingControl`), ad hoc on GTK and Kirigami (star buttons, as GNOME Software and Discover build it), drawn elsewhere;
  - a pips pager: native on Windows (`PipsPager`) and Kirigami (Qt's `PageIndicator`), drawn elsewhere.
- **The `files` example** (a simplified Finder) has one, where a platform's file manager shows something better than the built-in widgets can:
  - a path bar: native on macOS (`NSPathControl`, Finder's own, which shortens a deep path to icons) and Windows (`BreadcrumbBar`, File Explorer's), composed elsewhere as Nautilus and Dolphin build theirs (a button per folder, the ones above the last few in a menu). Its a11y action (a folder's path as the value) chooses a folder, so tests drive the native one the way a screen reader does and the composed one by its buttons.
  - Its file icons, trashing and opening files were escape hatches too, until they became `FileIcon` (§13.30) and the `trash` and `launch` services (§14.10, §14.11).
  - Where it has run: AppKit (`tests/files.rs` natively, and a capture checked by eye) and headless. The WinUI render (`BreadcrumbBar`, added to the bindings for it) is only type-checked; GTK's and KDE's composed ones too.
- **A composed render** gets the props (reactive), a way to emit the widget's events, and the app's accessible label, which it puts on the control that stands for the widget. It builds as a plain container, so its built-in widgets carry the semantics.
- **Coverage is checked at compile time:** using a widget requires `Render`, and `Render` has to name a render that exists on the platform being built. A missing render doesn't build.
- **Transport:** commands carry `WidgetKind::Custom(NAME)` and `Prop::Custom(CustomProps)`: the props as an `AnyValue` (type-erased, but still compared and printed with their own `PartialEq` and `Debug`) plus the widget's definition (semantics, action mapping, renders). Events come back as `UiEvent::Custom(AnyValue)`. Props don't need to be serializable. The native render travels with the props, so backends keep no registry. A custom widget can't be created without its props, which is why `Create` carries a node's initial props, reactive ones included; headless rejects a custom widget or native view created without its prop.
- **Semantics** come from `CustomWidget::a11y`; the app's overrides (`.a11y_label(…)`) win. Accessibility actions go to the native render first; if it doesn't handle one, the core emits the event `CustomWidget::action` maps it to. Drawn and native renders behave the same for assistive technology and tests.
- **The drawn tier runs in the core.** The core measures drawn widgets itself, draws them after layout (on new props, a new size or new metrics) and sends the result as `Prop::Drawing(DisplayList)` with the frames. Backends only rasterize display lists and report `UiEvent::Pointer`, which the core turns into widget events with `Drawn::pointer`. Headless wireframes draw them too, so drawn widgets show up in reviews without pixels. The example's drawn rating sizes its stars from the body font, so it sits close to the native rating controls. Drawn widgets can't draw text yet.
- **Synthesized clicks** (`SyntheticInput::Click`) are for drawn widgets only. Native controls track the mouse in a loop of their own; tests drive them with accessibility actions.
- **Headless** lays out natively rendered widgets with their drawn render (hence `.with_drawn()` above), or as empty boxes without one.
- **Controlled:** a render emits an event when the user changes the view; the app answers with new props, and `update` shows them. If the app ignores the event, the view shows something the core doesn't know about, and the mirror check (through `read`) reports it.

§15 has how each backend hosts renders and draws display lists.

### 7.4 Raw settings of a built-in widget

Semantic props cover what every platform has. For what only one platform has, a built-in widget takes a `Tweak`: a closure over the native control itself, made by the backend's `tweak`, picked per platform with `platform!`.

```rust
Button::new("Continue").role(ButtonRole::Default).native(platform! {
    macos => appkit::tweak(|b: &NSButton| b.setControlSize(NSControlSize::Large)),
    gtk => gtk::tweak(|b: &gtk::Button| b.add_css_class("circular")),
    kde => kirigami::tweak(|b: &QmlObject| b.set_real("padding", 16.0)),
    windows => winui::tweak(|b: &Button| b.cast::<IControl>()?.SetCornerRadius(round)),
})
```

- It travels as `Prop::Tweak` (an `Opaque`), and runs after the widget's other props, and again whenever one changes, so what it sets wins. Tweaks should be idempotent; one that connects a signal must guard itself (the example's GTK switch tweak marks the switch with a widget name).
- `tweak_with(value, |b, v| …)` runs again when the value changes; only the tweak is sent again.
- Tweaks run inside `apply`, with events muted where the backend mutes them.
- A tweak can make the native control disagree with the core's props (an icon for a label, say); the mirror check then fails in tests. Set what the semantic props don't.
- `_ => Tweak::none()` leaves the other platforms alone. Headless tests keep the tweak but don't run it.
- **The widget's type names the native one** (`Tweakable`, in each backend). Kirigami's is always the QML item, a `QmlObject` whose properties are set by name, and WinUI's closures return a `windows_core::Result`:

| Widget | AppKit | GTK | Kirigami | WinUI |
|---|---|---|---|---|
| `Text` | `NSTextField` | `gtk::Label` | `QQC2.Label` | `TextBlock` |
| `Button` | `NSButton` | `gtk::Button` | `QQC2.Button` | `Button` |
| `ToggleButton` | `NSButton` | `gtk::ToggleButton` | checkable `QQC2.Button` | `ToggleButton` |
| `MenuButton` | pull-down `NSPopUpButton` | `gtk::MenuButton` | `QQC2.Button` | `DropDownButton` |
| `Checkbox` | `NSButton` | `gtk::CheckButton` | `QQC2.CheckBox` | `CheckBox` |
| `Switch` | `NSSwitch` | `gtk::Switch` | `QQC2.Switch` | `ToggleSwitch` |
| `Select` | `NSPopUpButton` | `gtk::DropDown` | `QQC2.ComboBox` | `ComboBox` |
| `RadioGroup` | `NSStackView` | `gtk::Box` | `ColumnLayout` | `RadioButtons` |
| `Slider` | `NSSlider` | `gtk::Scale` | `QQC2.Slider` | `Slider` |
| `NumberInput` | `appkit::NumberField` (its `field()` and `stepper()`) | `gtk::SpinButton` | `QQC2.SpinBox` | `NumberBox` |
| `Progress` | `NSProgressIndicator` | `gtk::ProgressBar` | `QQC2.ProgressBar` | `ProgressBar` |
| `Spinner` | `NSProgressIndicator` | `gtk::Spinner` | `QQC2.BusyIndicator` | `ProgressRing` |
| `Separator` | `NSBox` | `gtk::Separator` | `Kirigami.Separator` | `Border` |
| `TextInput` | `NSTextField` | `gtk::Entry` | `QQC2.TextField` | `TextBox` |
| `PasswordInput` | `NSSecureTextField` | `gtk::PasswordEntry` | `Kirigami.PasswordField` | `PasswordBox` |
| `SearchInput` | `NSSearchField` | `gtk::SearchEntry` | `Kirigami.SearchField` | `AutoSuggestBox` |
| `TextArea` | `NSTextView` | `gtk::TextView` | `QQC2.TextArea` | `TextBox` |
| `ScrollView` | `NSScrollView` | `gtk::ScrolledWindow` | `QQC2.ScrollView` | `ScrollViewer` |
| `List` | `NSTableView` | `gtk::ListView` | `ListView` | `ListView` |
| `Table` | `NSTableView` | `gtk::ColumnView` | `TableView` | `ListView` |
| `Image` | `NSImageView` | `gtk::Picture` | `Image` | `Image` |
| `Icon` | `NSImageView` | `gtk::Image` | `Kirigami.Icon` | `FontIcon` |
| `Group` | `NSBox` | the card (`gtk::Box`) | `QQC2.GroupBox` | the card's `Border` |

A text area's and a list's tweak gets the view inside the scroll view, not the scroll view the node stands for. A `ScrollView`'s own tweak gets the scroll view; on Kirigami its `contentItem` is the `Flickable` that scrolls. `List`'s type parameters have defaults, so `Tweak<List>` names it without its item and key types.

Every widget's example but Icon's and ToggleButton's has one tweak per platform, and §13 says what each does. `Tweakable` lives in each backend, which depends on `mitsuami-widgets` for it.

---

## 8. Accessibility and i18n

- **Every node carries `A11yProps`:** role, label, description, value, `labelled_by` and hidden. The a11y tree adds each widget's state: checked and mixed, selected, enabled, read-only, password. Planned: ranges, expanded and busy states, `described_by`, live regions and action lists.
- **Built-in widgets derive defaults:** a button's label comes from its caption, a field's from its label or placeholder. Apps override with `.a11y_label("…")` and the like.
- **Most of the work comes free because the controls are native:** NSAccessibility, UIA, GtkAccessible and Qt's accessibility already understand native controls. Backends set the label and description on them (`setAccessibilityLabel`, `AutomationProperties`, GTK's accessible properties, QML's `Accessible.name`).
- **Drawn widgets are where real work is needed.** They have semantics in the core's tree, but no platform a11y yet. The plan is each backend's native protocol; [AccessKit](https://github.com/AccessKit/accesskit) is an option for drawn subtrees, since it gives the same model on every platform.
- **The core owns focus order.** The Tab order is reading (tree) order, so it's right in right-to-left layouts and for absolutely positioned controls. `.tab_index(n)` moves controls ahead. Backends get it as `SetFocusOrder` and chain native focus to it (§15). Which controls can take focus stays the platform's: on macOS it depends on the Keyboard navigation setting.
- **Focus from code is a `NodeRef`'s:** `focus()` focuses its control, as a click or Tab would, and `select_text(range)` focuses a text field or text area and selects part of its text, counted in grapheme clusters, the characters people see, so a selection never splits one (an accent written as a combining mark, a family emoji joined with zero-width joiners, a flag; `unicode-segmentation`, re-exported for apps to count with): the files example's Rename selects the name without its extension, as Finder, Nautilus, Dolphin and File Explorer do, and its Go to Folder the path typed last. Asked while nothing is built (a window's `on_open` runs before its content is), it's done once the node is. The core sends `Focus`, then `SelectText` with the range cut to the field's text and turned into Unicode scalar values (`char`s), on grapheme boundaries, after the batch's structure, props and Tab order, so the field has its text and focus first. Each backend converts characters to its own units:
  - AppKit selects through the window's field editor (`setSelectedRange:`, in UTF-16 units), a text area through its text view.
  - GTK: `gtk_editable_select_region` on a field (its entry passes it to its text widget), `gtk_text_buffer_select_range` in a text area, both in characters. Synthesized typing deletes the selection before `insert-at-cursor`, as GTK's own typing does: the keybinding signal leaves it.
  - Qt: `select` (UTF-16 positions) on the `TextField`, or the text area's text edit; a `PasswordField`'s positions follow its text, not its dots.
  - WinUI: `TextBox.Select` (UTF-16 offsets on XAML's text, whose lines end in one `\r`) on the field, the text area or the search box's own text box. A `PasswordBox` can only select all (`SelectAll`), and its selection can't be read: a partial range leaves the caret at the end, and it reports none. Focus and a selection asked for before the element is in XAML's live tree (a dialog opened in the same batch) are kept on the window and applied once it is, over the first control in the Tab order; the window's activation may focus that one for a moment first.
  - What focusing alone does to a field's text is the platform's (AppKit and GTK select all of it): only `select_text` says, after it. Tests read the selection back (`NativeState::selection`, in `char`s; the test kit's `text_selection()`, in graphemes, taking all of one a platform's selection ends inside), and typing goes over it (`selections_count_graphemes`).
  - Where it has run: AppKit and headless (`tests/focus.rs`, the files example's Rename and Go to Folder), in sheets too; `examples/focus.rs` is for trying it by hand. Only type-checked on GTK, Kirigami and WinUI. To try by hand on macOS: that a sheet's field keeps its selection once the sheet is shown and key (test windows never are).
- **`PlatformMetrics`** has dark mode, high contrast and reduced motion (`Ui::metrics()`, updated on `MetricsChanged`). Planned: the text scale, and all of them as signals.
- **Right to left:** styles use logical edges (`padding_start`, `margin_end`, `start`). `.direction(TextDirection::Rtl)`, inherited, flips them and gives Taffy the direction, which mirrors rows (`right_to_left_mirrors_rows_and_logical_edges`). Planned: taking the direction from the locale.
- Text is never baked into images. All strings go through props, so they can be localised.

---

## 9. App logic and state

- **Domain logic is plain Rust:** no mitsuami dependency, `Send` where useful, async-friendly, and testable through its own public API.
- **Stores** (Pinia-like) are the UI-thread adapter. They own signals and expose actions that call into domain logic. A store is a plain `Clone` struct implementing `Store` (`fn create() -> Self`). `use_store::<S>()` returns the app's one instance, created on first use in a scope that lives as long as the app (`App` and `TestApp` call `provide_stores()` in their app scope), so a store outlives the views that use it, and its resources and tasks keep running.
- **A store's actions spawn with `Store::spawn`,** which runs the task in the store's scope. An action runs in its caller's scope, so `spawn_local` would tie its work to the view that called it: the Files example's rename, started from a dialog's button, was cancelled when the dialog closed, with no error. `Store::spawn` finds the store the caller's `use_store` would (`provide`d, through `Owner::providing`, or the app's) and spawns in the scope it was provided or made in. Each store's scope is known before `create` runs, so `create` can spawn too.
- **Async** uses plain futures: `spawn_local`, `spawn_blocking` and `sleep`, plus `alert`, `open_file` and `save_file` for dialogs.
- **Resources and actions** wrap these as signals, all `Copy`:
  - `resource(fetch)` and `resource_on(source, fetch)` load a `Result<T, E>`: `loading()`, `data()`, `error()`, `refetch()`, `set_data()`. They fetch when created, and `resource_on` again whenever the signals `source` reads change. A new fetch cancels the one in flight. The last data stays while reloading and after an error, so views can show both.
  - `action(fn)` runs an async operation on `dispatch(input)`: `pending()` while any dispatch runs, and `value()` of the latest dispatch (an earlier one finishing later doesn't overwrite it).
  - Both are owned by the scope that created them: disposing it cancels what's in flight.
- **`provide` / `inject` replaces globals,** so screens can be tested with mock stores: a store `provide`d in a scope takes precedence over the app's instance there.

Every per-platform screen (§7.1) uses the same stores and composables. That is the reuse boundary.

---

## 10. API

### From Vue

| Vue | mitsuami |
|---|---|
| `ref(x)` / `reactive` | `signal(x)` (`ref` is a Rust keyword). `Signal<T>` is `Copy` (arena-backed) |
| `computed` | `computed(move \|\| …)` |
| `watch` / `watchEffect` | `watch(source, cb)` (not for the initial value) / `effect(move \|\| …)` |
| props | typed parameters: `#[component] fn Foo(label: String, #[prop(default)] n: i32)`; `Value<T>` for reactive ones |
| `emit('x')` | typed callback props: `on_change: Callback<f32>`, set with `@change=…`, fired with `on_change.call(v)` |
| `v-model` | `bind=signal` (two-way) |
| `v-if` / `v-show` | `<Show when=… fallback=…>` / `hidden=…` |
| `v-for` + `:key` | `<For each=… key=… let:item>` |
| slots | `children: Slot`; named slots later |
| `provide` / `inject` | `provide(ctx)` / `inject::<T>()` |
| `onUnmounted` | `on_cleanup`. There's no `onMounted`: a component runs once, when it's built |
| template refs | `let r = node_ref(); <TextInput node_ref=r/>`, then `r.get()` (an `Option<NodeId>`); `use_size(r)` takes the ref; `r.focus()` and `r.select_text(0..5)` focus it (§8) |

```rust
#[component]
fn Counter(initial: i32) -> impl View {
    let count = signal(initial);
    let doubled = computed(move || count.get() * 2);

    view! {
        <Column gap=Spacing::Md padding=2.em() align=Align::Center>
            <Text text_style=TextStyle::Title>{move || format!("Count: {}", count.get())}</Text>
            <Button role=ButtonRole::Default @click=move || count.update(|c| *c += 1)>"Increment"</Button>
            <Show when={move || doubled.get() > 10}>
                <Text>"That's a big number"</Text>
            </Show>
        </Column>
    }
}
```

### `view!` and the builder API

The builder API is the real API. `view!` expands to it, one tag at a time: `<Tag a=x @e=h flag>children</Tag>` is `Tag::__tag().a(x).on_e(h).flag().__children(|| children)`. So the counter is:

```rust
Column::__tag().gap(Spacing::Md).padding(2.em()).align(Align::Center).__children(|| (
    Text::__tag().text_style(TextStyle::Title).__children(|| move || format!("Count: {}", count.get())),
    Button::__tag().role(ButtonRole::Default).on_click(move || count.update(|c| *c += 1)).__children(|| "Increment"),
    Show::__tag().when(move || doubled.get() > 10).__children(move || Text::__tag().__children(|| "That's a big number")),
))
```

which builds the same tree as `Column::new().gap(…).children((Text::new(…).text_style(…), Button::new("Increment")…, Show::new(…, …)))`.

- **Attributes are builder methods,** checked by the compiler like any call. Values are literals, paths, calls, method chains and closures; anything else goes in braces. A `>` ends the tag, so comparisons need braces: `when={a > b}`. Enum values are written with their type (`Align::Center`): a macro can't infer types.
- **Children are passed in a closure,** which is how `Show` rebuilds a branch and `For` renders a row (`let:item` names the row's parameter). Other tags call it right away, so their children are built like the builder API's and siblings can share variables; only `Show`, `For` and `Window` (whose content is built at each opening) get a `move` closure, which owns what it uses. One child is passed as is, which gives `Text` its text and `Button` its caption; several become a tuple.
- **What a constructor takes is an attribute too.** `Select::new("Plan")` is `<Select label="Plan">`: `label` sets the same `Prop::Label` the platform labels its control with (`a11y_label` only names the core's node, so an AppKit number field and its stepper would go unlabelled). `ScrollView::horizontal()` is `<ScrollView axes=ScrollAxes::Horizontal>`. A builder that takes two arguments has a one-argument form for attributes: `range_with=(0.0, 100.0)`. Every example is written with `view!`, and `tests/macros.rs` checks that these tags build what their constructors build.
- **Required attributes are types.** `<Show>` is `ShowWithoutWhen` until `when` is set, and only a `Show` has children. `<For>` wants `each`, then `key`. `<Window>` is a `Window<()>` until `title` is set, in any order among its attributes, since each builder method keeps the title's type.
- **Custom widgets are tags too:** `<Rating props=move || RatingProps { … } a11y_label="Rating" @event=move |e| …/>`. The tag is a `Custom<Rating, ()>` until `props` is set, and only then a `View`.
- **Mistakes are compile errors** that point at the tag or attribute: an unknown attribute is an unknown method (with rustc's "did you mean"), and a tag missing a required attribute isn't a `View`; the error's note says so.
- **`#[component]`** turns the function into a builder struct with one method per parameter, usable without `view!` (`Counter::new().initial(3)`). Each required prop is a type parameter, `()` until set, and the builder is a `View` only once all are set, so a missing prop is a compile error. `#[prop(default)]`, `#[prop(default = expr)]`, `Option<T>`, `Callback<T>` and `children: Slot` are optional; `#[prop(into)]` accepts `impl Into<T>`, and `Value<T>` accepts a literal, a signal or a closure.
- **A component runs once, when it's built, in a scope of its own.** What it `provide`s reaches its children but not its siblings, and its scope is disposed with it.

---

## 11. Platform versions and capabilities

We don't pick one fixed OS version per platform. Each backend has a **hard floor**, the oldest version it can run on; everything above it is meant to be a **capability**, queried at run time, with built-in widgets degrading where one is missing.

### Floors

| Platform | Floor | Why |
|---|---|---|
| Windows | Whatever Windows App SDK 2.4 / WinUI 3 supports (historically Windows 10 1809, build 17763; to be confirmed for 2.4) | The SDK version the bindings are generated from. |
| macOS | macOS 11 | The practical floor for arm64 and current Rust targets. Everything newer is looked up at run time. |
| Linux (GTK) | GTK 4.10 and libadwaita 1.4 | Fixed at build time: gtk4-rs's `v4_10` and libadwaita's `v1_4` features (the sidebar's navigation split view needs 1.4). libadwaita 1.7's inline view switcher is looked up at run time (§14.5). |
| Linux (KDE Plasma) | Qt 6.5 and Kirigami 6 | The backend's build asks for Qt 6.5, the first Qt 6 LTS with what it uses; a given Kirigami release may need a newer Qt. Developed on Qt 6.11 and Kirigami 6.30. Kirigami's features resolve in QML at run time. The select's list opens as a window of its own from Qt 6.8 (§13.8). |

### Capabilities (planned)

None of this is built yet. The design:

```rust
#[non_exhaustive]
pub enum Capability {
    // cross-platform semantics, answered by every backend
    NativeSwitch, SymbolIcons, NativeFileDialog, SystemAccentColor,
    WindowBackdropMaterial,        // Mica on Windows 11, vibrancy or glass on macOS, none on GTK
    SplitViewSidebar, SearchField, DatePicker, …
    // platform-specific ones live in each backend's own enum
}

caps().has(Capability::WindowBackdropMaterial)   // a plain bool, fixed for the process's lifetime
```

Detection follows each platform:

- **macOS: at run time.** Objective-C is dynamic, so the backend checks the OS version, `respondsToSelector:` and class existence at startup. One binary adapts to whatever macOS it runs on.
- **Windows: at build time, plus the OS build.** The WinAppSDK version is bundled with the app, so its features are fixed. OS-dependent ones, like Mica on Windows 11, are checked from the build number.
- **Linux: at build time.** gtk4-rs links newer symbols directly, so a binary built for GTK 4.12 wouldn't load on 4.10; per-version features (and a reduced GTK 4.8 mode) would let distributions pick theirs. Looking symbols up at run time is possible, and the tab view already does it for libadwaita 1.7.

How they'd be used:

- **Built-in widgets degrade on their own,** each fallback documented on the widget (a `Switch` as a checkbox-style toggle where there's no native switch); app code only branches when it wants different behaviour.
- **Apps declare hard requirements** (`App::new().require(Capability::X)`), and startup fails with a clear, native error instead of breaking halfway.
- **Views branch on capabilities:** `platform!` arms with predicates (`macos if has(WindowBackdropMaterial) => …`), and a `<Supports cap=… fallback=…>` component.
- **Custom widgets** query `cx.caps()` to choose an implementation.

This settles "run-time or compile-time checks": the platform is compile-time (`platform!`), capabilities are run-time, fixed per process.

---

## 12. Testing

Testing is a first-class feature, for mitsuami itself and for apps built with it.

### Policy

- **No unit tests.** There are no `#[cfg(test)] mod tests` blocks and no tests of private functions. Every test goes through a public API.
- **Integration tests** use the public API and the headless backend. They're fast and deterministic, and run everywhere, in CI and locally.
- **End-to-end tests** are the same tests on a real native backend, in real windows, driven the way a user would drive them.
- **Visual regression tests** (Chromatic-style) are a third track on the same infrastructure.
- The tooling we build for ourselves is the tooling we ship: `mitsuami-test` is a public crate for app authors, and mitsuami's own suite is its first user.

### One test API, two backends

```rust
use mitsuami_test::prelude::*;

#[mitsuami_test::test]
async fn increments(app: TestApp) {
    app.mount(|| Counter::new().initial(0));
    app.get_by_role(Role::Button, "Increment").click().await;
    app.expect(by_text("Count: 1")).to_be_visible().await;
}

mitsuami_test::main!();
```

```
cargo test                      # headless (default): the integration tier
MITSUAMI_NATIVE=1 cargo test    # the same tests on AppKit, WinUI, GTK or Kirigami: the e2e tier
                                # (or `-- --native` for a single harness = false target)
cargo mitsuami visual           # the visual tier (below)
```

Kirigami needs the features: `-p mitsuami -p mitsuami-kirigami --features mitsuami/kde,mitsuami-test/kde,mitsuami-kirigami/qt`. `cargo test -- --native` would also reach the workspace's libtest harnesses, which reject the flag, so `MITSUAMI_NATIVE=1` is the workspace-wide switch.

- **Queries go through the accessibility tree,** like Testing Library: `get_by_role`, `get_by_label`, `get_by_text`, and `get_by_test_id` as a last resort. If a test can't find a control by role and label, a screen-reader user can't either.
- **Actions are what assistive technology does:** `click`, `check`, `uncheck`, `fill`, `select_option`, `set_number`, `increment`, `decrement`, `select`, `focus`, `scroll_into_view`, `choose_menu_item`. The backend carries them out on the native control (`perform`): AppKit's `accessibilityPerformPress`, UIA patterns on WinUI (Invoke, Value, Toggle, RangeValue), GTK's `activate` and signals, Qt's `QAccessible` actions. The same vocabulary will power screen-reader support.
- **Raw input** (`type_text`, `press(shortcut)`, `scroll_by`, `click_at` on drawn widgets, `drag_files`, `drag_leave` and `drop_files`: files dragged onto a node, not a list's `drag_files`, whose drag source `dragged_files` reads) goes through the platform's own input path where it allows (`synthesize`).
- **No sleeps, no flakiness.** We own the scheduler, so `await` on an action or assertion means "run until the UI is settled": effects flushed, layout done, the native commit applied, pending tasks idle, and the platform caught up (`TestHooks::settle`). Assertions retry, Playwright-style, while the app has tasks, with a timeout (`MITSUAMI_WAIT_MS`).
- **A deterministic environment:**
  - a manual clock, which `app.advance(duration)` moves;
  - the appearance forced to light (or a story's variant), and on each platform what else leaks from the desktop kept out (§15);
  - scripted services: `app.services()` is a `FakeServicesHandle` that records dialogs and the clipboard and answers them, even natively, so tests never show real dialogs or touch the real clipboard;
  - planned: a fixed locale and text scale per test.
- **Dependency injection:** tests `provide` fake stores or services before mounting, the same `provide`/`inject` apps already use.
- **The mirror check.** After every settle, the test kit compares what each native widget shows (`native_state`: text, value, checked, enabled, frame, children, focus, scroll offset, every prop the core has) with the core's tree, and fails on any difference. Props a platform can't read back are kept on the node. This has caught every serious backend bug so far. `Locator::native_state()` reads it in a test.
- **Snapshots:** the tree (every node's kind, props and frame), the a11y tree, the command log, and a **wireframe**: an SVG of the headless layout (boxes, labels, roles). The wireframe is deterministic, platform-independent and diffable in review, a cheap first line of visual testing. Files are `<file>__<test>@<name>.tree.txt`, `.a11y.txt`, `.commands.txt` and `.wireframe.svg`. A changed one is written as `.new` next to it; a missing one is recorded, except on CI, where it's written as `.new` and fails. Headless snapshots don't depend on the machine and live in `tests/snapshots/`; native ones in `tests/snapshots/<backend>/<image>/` (below).

### The main-thread problem

Native UI must run on the process's main thread, and macOS enforces it; libtest runs tests on worker threads. So:

- Test targets use `harness = false` and end with `mitsuami_test::main!()`, our runner. `#[mitsuami_test::test]` registers each test with `inventory`.
- The runner owns the main thread and the native event loop, and runs test futures on it.
- Natively, tests run serially in one process, with a fresh window each. While a test awaits native work (a capture), the test executor lets the backend run.
- Filtering and output follow libtest's conventions (`--exact`, `--list`, `--ignored`, `--skip`), so `cargo test name_filter` and IDE runners keep working.

### Visual regression

**Stories.** A story renders a component in a given state. Stories double as a component gallery.

```rust
#[mitsuami_test::story(sizes = [(320, 200)], variants = [Light, Dark])]
fn counter_big_number() -> impl View { Counter::new().initial(42) }
```

- `play = …` runs a test script first (focus a field, type text) and captures the result.
- Each size and variant runs as its own test, `<story>@<width>x<height>-<variant>` (`signup_ready@280xfit-dark`), in a window opened at that size with the variant's appearance forced. A height of `fit` fits the content at its first layout (`WindowSize::FitHeight`): control heights differ per platform, so a fixed height that suits one clips or pads another. `sizes` defaults to 800×600, and `variants` to `[Light, Dark]`.
- Headless has no pixels, so there a story only checks that the view mounts and the script plays.
- `tests/stories.rs` has every built-in widget as a story.

**Capture** happens in-process and offscreen, so it needs no screen-recording permission and doesn't depend on window placement (`Backend::capture`): AppKit's `cacheDisplayInRect:toBitmapImageRep:`, WinUI's `RenderTargetBitmap`, GTK's `WidgetPaintable` rendered with Cairo, Qt's `QQuickWindow::grabWindow`.

**Baselines** belong to the machine image that recorded them, since native rendering differs between OS versions and display scales: `tests/visual/<backend>/<image>/<file>__<story>@<width>x<height>-<variant>.png`, and native snapshots likewise in `tests/snapshots/<backend>/<image>/`.

- The image is `MITSUAMI_IMAGE`, which CI sets to its pinned runner image (`macos-26`, `ubuntu-24.04`, `windows-2025`), or for the KDE job to its Arch Linux container, pinned to a day of the Arch Linux Archive (`archlinux-2026.09.20`). Otherwise it's the OS and its version (`macos-26`, `ubuntu-24.04`, `windows-26100`). A scale other than 1× is appended (`macos-26@2x`), since captures are in physical pixels.
- Only CI's baselines are kept in the repository. A developer's machine records its own under its image, which git ignores, and compares only with those. Setting `MITSUAMI_IMAGE` locally would record into CI's.
- A snapshot failure doesn't stop a test. It fails at the end with all of them, having written every changed one next to its baseline (`.new`, `.new.png`, and a `.diff.png`); off CI a missing one is recorded, on CI it's written as `.new` too. CI uploads them as a `snapshots-<image>` artifact (`-kde` appended for KDE's job), and `.github/scripts/accept-snapshots.sh <run id>` moves them over the baselines. That's also how a new image, or a new platform's first baselines, get recorded.
- Stored as plain files for now (git LFS when volume demands it). A hosted store can be added later behind the same interface.
- The variants are light and dark; the scale is the machine's. Planned: forcing scales, high contrast and large text.

**Diffing.**

- A perceptual diff, pixelmatch's: colours are compared by their distance in YIQ, and pixels that differ only by anti-aliasing don't count. `VisualOptions` sets the `threshold` for how different a colour must look (0.1 by default), `max_changed` for the fraction of pixels that may change (0.1% by default), and regions to leave out: nodes found by a query, where they are at capture time (`ignore(by_test_id("clock"))`; a node whose content changes needs a fixed size, so the baseline's content is in the same region), or rects in window coordinates (`ignore_rect`). A story takes `threshold`, `max_changed` and `ignore = [query, …]` as attributes. The `.diff.png` shows changes in red, anti-aliasing in yellow and ignored regions in blue.
- A **layout diff** says *why* pixels moved: a layout change or a native rendering change. Each baseline has the layout it was captured with next to it, `<name>.layout.txt` (the tree snapshot's format), recorded and accepted with the PNG. When pixels change, the failure lists the nodes whose layout or props differ, or says the layout is the same and the platform draws it differently. A layout change alone doesn't fail.

**Review.**

- `cargo mitsuami visual review` (the `cargo-mitsuami` crate; in this repository an alias in `.cargo/config.toml` runs the workspace's copy, and apps `cargo install` it) finds every pending capture under `tests/visual/` and serves a review page on the loopback interface, behind a random token in its URL. Each change shows why its pixels changed (from the layouts), and its baseline and capture side by side, as a swipe, as an onion skin, or as the diff image. Accept moves the capture and its layout over the baseline; Reject deletes them. `--out <dir>` writes the same page as a static, read-only report.
- `cargo mitsuami visual accept [filter]` accepts without the page, and `cargo mitsuami visual` runs the native tests and says what's left to review.
- CI (`.github/workflows/test.yml`) runs the tests and stories on macOS, Windows and Linux runners and in KDE's container, and fails on unapproved diffs. Each failed job uploads the static report as a `visual-review-<image>` artifact (`-kde` for KDE's), next to `snapshots-<image>`, and says in the run's summary how many captures it holds (`.github/scripts/collect-review.sh`). CI runs only when started by hand (`gh workflow run test.yml --ref <branch>`), and headless before a release.
- Visual baselines aren't maintained yet: development moves too fast for them to keep up, so missing or failing ones are expected until the maintainer reviews every capture by eye.
- Later: a PR comment with a summary, and a small hosted review service.

### What mitsuami tests about itself

- **The reactive runtime,** through its public API in `crates/mitsuami-reactive/tests/`: ownership and cleanup, batching, glitch-freedom, flush ordering.
- **Layout and units,** headless, against frames computed by a browser for the equivalent HTML and CSS (`tests/layout.rs`).
- **The backend conformance suite** (`tests/conformance.rs`), which every backend must pass natively: each built-in widget's props, events, measurement, a11y roles, and focus traversal. It's the executable definition of the backend contract.
- **A suite per widget and feature** in `tests/`, run headless and natively, and a story for each widget in `tests/stories.rs`.
- **Each backend's real services** in its own `tests/services.rs` (a private clipboard, the real menus, an alert answered by clicking, a cancelled file dialog, a file moved to the user's trash), since app tests use the scripted fake.

Some input can't be simulated: AppKit's tracking loops read the physical mouse button, and posting real events needs an Accessibility permission. There, a small app with the variants side by side is built and tried by hand, and §13 and §14 say what was found that way.

---
## 13. Widgets

Every built-in widget is the platform's own control. §14 covers windows and the app shell (toolbars, sidebars, tabs, menus).

| Widget | AppKit | GTK 4 | Kirigami | WinUI 3 |
|---|---|---|---|---|
| Container, Row, Column, Grid | flipped `NSView` host | custom `gtk::Widget` host | `Item` host | `Canvas` host |
| Text | `NSTextField` (label) | `gtk::Label` | `QQC2.Label` (`Kirigami.SelectableLabel` when selectable) | `TextBlock` |
| Button | `NSButton` | `gtk::Button` | `QQC2.Button` | `Button` |
| ToggleButton | `NSButton` (push on, push off) | `gtk::ToggleButton` | checkable `QQC2.Button` | `ToggleButton` |
| MenuButton | pull-down `NSPopUpButton` | `gtk::MenuButton` | `QQC2.Button` with a `QQC2.Menu` | `DropDownButton` |
| Checkbox | `NSButton` (checkbox) | `gtk::CheckButton` | `QQC2.CheckBox` | `CheckBox` |
| Switch | `NSSwitch` | `gtk::Switch` | `QQC2.Switch` | `ToggleSwitch` |
| RadioGroup | radio `NSButton`s in an `NSStackView` | `gtk::CheckButton`s in one group, in a `gtk::Box` | `QQC2.RadioButton`s in a `ColumnLayout` | `RadioButtons` |
| Select | `NSPopUpButton` | `gtk::DropDown` | `QQC2.ComboBox` | `ComboBox` |
| Slider | `NSSlider` | `gtk::Scale` | `QQC2.Slider` | `Slider` |
| NumberInput | `NSTextField` + `NSStepper` | `gtk::SpinButton` | `QQC2.SpinBox` | `NumberBox` |
| Progress | `NSProgressIndicator` (bar) | `gtk::ProgressBar` | `QQC2.ProgressBar` | `ProgressBar` |
| Spinner | `NSProgressIndicator` (spinning) | `gtk::Spinner` | `QQC2.BusyIndicator` | `ProgressRing` |
| Separator | `NSBox` (separator) | `gtk::Separator` | `Kirigami.Separator` | `Border` in the divider brush |
| TextInput | `NSTextField` | `gtk::Entry` | `QQC2.TextField` | `TextBox` |
| PasswordInput | `NSSecureTextField` | `gtk::PasswordEntry` | `Kirigami.PasswordField` | `PasswordBox` |
| SearchInput | `NSSearchField` | `gtk::SearchEntry` | `Kirigami.SearchField` | `AutoSuggestBox` (find icon) |
| TextArea | `NSTextView` in `NSScrollView` | `gtk::TextView` in `ScrolledWindow` | `QQC2.TextArea` in `ScrollView` | `TextBox` (`AcceptsReturn`) |
| Image | `NSImageView` | `gtk::Picture` | QML `Image` | `Image` |
| Icon | SF Symbol in an `NSImageView` | themed icon in a `gtk::Image` | `Kirigami.Icon` | `FontIcon` (Segoe Fluent Icons) |
| FileIcon | `NSWorkspace`'s icon (or QuickLook's thumbnail) in an `NSImageView` | GIO's icon (or the cached thumbnail) in a `gtk::Image` | the MIME type's icon in a `Kirigami.Icon` | the shell's image (`IShellItemImageFactory`) in an `Image` |
| Group | `NSBox` | heading over a libadwaita card | `QQC2.GroupBox` | heading over a card `Border` |
| ScrollView | `NSScrollView` | `gtk::ScrolledWindow` | `QQC2.ScrollView` | `ScrollViewer` |
| List (virtualised) | `NSTableView` | `gtk::ListView` | QML `ListView` | `ListView` |
| Table (virtualised) | `NSTableView` with columns and a header | `gtk::ColumnView` | QML `TableView` under a `HorizontalHeaderView` | `ListView` under a header of column buttons |
| GpuSurface | `NSView` backed by a `CAMetalLayer` | Wayland subsurface or X11 child window | Wayland subsurface or X11 child window | child HWND |

Tooltips, context menus and file drops are props any widget takes (§13.23–§13.25); containers and lists take keys (§13.29), and lists' and tables' rows drag files out (§13.31).

**What every widget section has.** Each widget got its semantic props from a survey of what the four platforms' controls offer: an option is semantic only if every platform has it (§1), and each section says what was left out and why. Everything else is a tweak (§7.4). Each widget has an example (`examples/<widget>.rs`, all of them in `examples/showcase`) with the semantic props to try and one tweak per platform, a story in `tests/stories.rs`, and a suite in `tests/`. Each section ends with where it has run: natively and checked by eye, or only type-checked from macOS (§12), with what's still to verify there.

**Which widgets there are** follows 2ksbox, whose Qt Quick launcher mitsuami was started to replace (§16): widgets are added as it needs them, and only widgets every platform has a native control for. From it so far: `NumberInput` (its `SpinBox`), `Image` (its shader preview), tooltips (its elided status line), `Separator`, and `GpuSurface` for its player. Left to the app: a disclosure header (Qt Quick has none; 2ksbox builds its own from a `ToolButton`). The `files` example is the other source: `FileIcon`, the trash, launching and dragging rows out came from what it had to build itself.

### 13.1 Text

A label, in a text style (§5), wrapping at words.

- **`max_lines(n)`:** at most so many lines, the last cut off with the platform's ellipsis (AppKit `maximumNumberOfLines`, GTK `lines` with `ellipsize`, Qt `maximumLineCount` with `elide`, XAML `MaxLines` with `TextTrimming`). The label is measured as limited, and still read out in full. 0 lifts the limit, as on AppKit and WinUI. A limited label also shrinks below its longest word, as an eliding label does on every platform (GTK's minimum width is its ellipsis): the core gives it a zero minimum width unless the app set one, where flex's automatic minimum would be that word's width, a path's whole width say. It ran on AppKit and headless (`a_line_limit_lets_text_shrink_below_its_longest_word`); the other backends only type-check. Qt's eliding labels also drop the lines past their height, so Kirigami measures them with the height lifted: a label's frame is 0 high until its first layout, and measured as one line from it.
- **`selectable(true)`:** the user can select and copy the text (AppKit's and GTK's `selectable`, XAML's `IsTextSelectionEnabled`). Qt's `QQC2.Label` can't select, so on KDE a selectable label is another item, `Kirigami.SelectableLabel` (a read-only text area drawn as a label), chosen when it's made: the option takes a plain `bool` and goes in the `Create`. Kirigami's selectable label has no line limit or elision, so there `max_lines` is kept on the node, not shown.
- **Colour, weight, italics and alignment** (`color`, `weight`, `italic`, `text_align`): every platform's label has them. Semantic colours stay the platform's own, so they follow the appearance live:
  - AppKit: `textColor` from the catalogue colours (`labelColor`, `secondaryLabelColor`, `controlAccentColor`, `systemRedColor`, `systemOrangeColor`, `systemGreenColor`), `Rgba` a fixed sRGB colour. Weight rebuilds the style's font with `systemFontOfSize:weight:` (the monospaced one for `Monospace`), and italics adds the italic trait; the font is rebuilt from style, weight and italics whenever one changes, so their order doesn't matter. `alignment` Left, Center or Right. Read back: the weight trait (nearest step), the italic trait, `alignment`, and `textColor` compared with the colour sent.
  - GTK: colours are theme style classes where GTK has one (`dim-label`, `accent`, `error`, `warning`, `success`; `Label` is no class), so they follow light, dark, high contrast and the accent; the rest and `Rgba` are a Pango foreground attribute, the theme ones resolved once when set, so those don't follow a theme change. Weight and italics are Pango attributes, over the style's class. Alignment is `xalign` and `justify`, with the label set left-to-right, since GTK mirrors both in a right-to-left widget. Drawn colours use the theme's `error_color`, `warning_color` and `success_color`, with libadwaita's values as the fallback.
  - Kirigami: colours are `Kirigami.Theme` bindings in the label's QML (`textColor`; `disabledTextColor` for the secondary label, as Kirigami's subtitles use; `highlightColor`, `negativeTextColor`, `neutralTextColor`, `positiveTextColor`), chosen through properties of ours (`setProperty` can't make a binding), which `native_state` reads. Weight and italics are `font.weight` and `font.italic` bindings, apart from the style's family and size. `horizontalAlignment` Left, HCenter or Right: Qt mirrors an explicit one only under `LayoutMirroring`, which nothing enables.
  - WinUI: colour is a `Foreground` setter in a style loaded with `XamlReader`, based on the text style's (`BasedOn`), so theme brushes follow the theme as Fluent's own styles do (`TextFillColorPrimaryBrush`, `TextFillColorSecondaryBrush`, `AccentTextFillColorPrimaryBrush`, `SystemFillColorCriticalBrush`, `CautionBrush`, `SuccessBrush`). A brush can't be told apart once resolved, so the colour is reported from the node. Weight (400 to 700) and italics are local values, which beat style setters, so a new text style keeps them. `TextAlignment` Left, Center or Right.
  - Weight and italics change a label's size (`affects_measure`); alignment and colour don't.
- **Left out:** a font family or point size (§5). Style classes, rich text and letter spacing are each platform's own.
- **The example's tweaks:** expansion tooltips on AppKit (`allowsExpansionToolTips`: the whole text in a tooltip when it's cut off, so the example cuts its line off there), Pango attributes on GTK (underlined), Markdown (`textFormat`) on Qt, `CharacterSpacing` on WinUI. The `text_tweaked` story has the same; a still capture doesn't show AppKit's. GTK's `use-markup` would parallel Qt's Markdown, but the mirror check reads the label's `text`, which drops the markup. The GTK tweak ran natively on GTK; AppKit's is only type-checked.
- **Text styles per platform:** GNOME's type scale on GTK (`title-1`, `title-2`, `heading`, `caption`, `monospace`; GNOME has no callout size, so callouts use the body's). On Kirigami without Plasma's platform theme, the small font falls back to a 12 pt system font, larger than the body, so captions are 0.8 × the body, Plasma's ratio.
- **Where it has run:** the line limit on every backend, checked by eye. Colour, weight, italics, alignment and `selectable` on AppKit and headless (`tests/text.rs`, the `text_options` story); only type-checked on GTK, Kirigami and WinUI (Kirigami's QML parsed with Homebrew's `qmllint`, without its modules). To verify there: whether GTK's own theme (not libadwaita's) styles `.accent`, `.error`, `.warning` and `.success` labels (the mirror check reads the class, not the colour); Pango's automatic direction putting a right-to-left paragraph's left alignment on the right; Kirigami's colour and weight bindings, and that a label without a colour draws as the desktop style's does; WinUI's `SetBasedOn` on a freshly loaded style, and a coloured label without a text style replacing an implicit `TextBlock` style, if any; how the colours and weights look, in light and dark (`examples/text.rs` has a control for each).

### 13.2 Button

- **A role and a style,** since what a button does and how it's drawn are separate choices (a borderless destructive button). Both are sent only if the app picks one, and backends keep them on the node: no toolkit reads every role back.
  - **Default:** AppKit's `keyEquivalent` Return; GTK's `suggested-action`; Qt's `Accessible.defaultButton`, which the desktop style turns into `QStyleOptionButton::DefaultButton` (Breeze tints it, except when flat; `highlighted` only draws the focus frame there, so it showed nothing), and which screen readers hear; WinUI's `AccentButtonStyle`. Return clicks the default button only on AppKit so far: GTK's default widget isn't set yet, and Qt Quick has no Return-clicks-default outside dialogs.
  - **Cancel** is Escape on AppKit only. GTK, Qt and WinUI have no cancel button outside their dialogs, so it's a normal button there.
  - **Destructive:** GTK's `destructive-action`, AppKit's `hasDestructiveAction`. Breeze and Fluent have no destructive style, so it's a normal button on Kirigami and WinUI.
  - **Borderless:** AppKit `bordered` off, GTK `has-frame` off, Qt `flat`, WinUI `SubtleButtonStyle`. On WinUI one XAML style carries both, so Borderless wins over Default.
  - Default buttons draw their accent only in the key window on AppKit, so test captures, whose windows are never key, show them grey (§15, AppKit).
- **An icon before the caption** (`icon(name)`, a name in the platform's own set, as for `Icon`, §13.19), placed as each platform places a button's icon:
  - AppKit: the button's image, `imagePosition` leading, sized for the bezel.
  - GTK: libadwaita's `ButtonContent`, as GNOME apps do it.
  - Qt: `icon.name`.
  - WinUI: a horizontal `StackPanel` of a `FontIcon` and the caption, 8 apart as in WinUI's gallery.
- **`icon_only`** hides the caption, which stays the accessible name: AppKit's image-only position, GTK's own icon button with the caption as its accessible label, Qt's `display: IconOnly`, and on WinUI the glyph alone with the caption as its automation name (UIA reads nothing from a glyph or a panel). A tooltip saying the same is up to the app. Without an icon the caption shows.
  - AppKit measures a borderless image-only button smaller than its symbol (15 × 9 for the trash can's 15 × 17), so the backend makes it at least the image's size.
  - A new title resets an `NSButton`'s image position, and `view!` sets the caption after `icon_only`, so the backend sets the image position again with the title (`a_new_caption_keeps_the_icon_alone`).
- **Presses go through the platform's accessibility press** (§15), as a screen reader's do.
- **The example** (`examples/button.rs`) shows every role in every style, a playground of the semantic props, toggle buttons, and one tweak per platform: a large control size on AppKit (a `bezelColor` tint didn't show in captures), GTK's `circular` class, a `padding` of 16 on Qt (only type-checked), a `CornerRadius` of 16 on WinUI.
- **Where it has run:** roles and styles on every backend, checked by eye; icons on every backend and headless (`tests/icon.rs`).

### 13.3 ToggleButton

- **A button that stays pressed,** which every platform has as a control of its own or a button's mode: an `NSButton` of the push-on-push-off type, `gtk::ToggleButton`, a checkable `QQC2.Button`, XAML's `ToggleButton`. It's a widget of its own rather than a `Button` prop, since GTK and WinUI make it another class.
- **A button's caption, icon, `icon_only` and style** (not its role: no platform has a default or cancel toggle), and a checkbox's `checked`, `bind` and `on_change`.
- **Pressed is drawn the platform's way:** a darker bezel on AppKit (macOS 26), the accent fill on WinUI, pressed on GTK and Qt. Borderless is AppKit's `bordered` off, GTK's `has-frame` off, Qt's `flat`, and on WinUI a style of its own clearing the fill and border at rest, as Fluent has no subtle toggle button.
- **A toggle button to assistive technology** (`Role::ToggleButton`, `checked` while pressed), as AppKit's toggle, GTK's toggle button, ARIA's pressed button and Qt's and UIA's checkable buttons are. `Activate` is assistive technology's press (AppKit), toggle (UIA's Toggle pattern, Qt's `Toggle` or `Press`), or `set_active` on GTK; Space presses it.
- **The app's changes aren't reported:** GTK's `toggled` fires for them (muted while commands apply), WinUI's `Checked` and `Unchecked` are compared with the value shown, and Qt's `toggled` and AppKit's action are the user's only.
- **Where it has run:** AppKit and headless (`tests/toggle_button.rs`, the `toggle_buttons` story). Only type-checked on GTK, Kirigami and WinUI.

### 13.4 MenuButton

- **A button that opens a menu of actions,** as a toolbar's "Add" or "New" does: a pull-down `NSPopUpButton` on AppKit, `gtk::MenuButton` on GTK, `DropDownButton` on WinUI, and on Qt a `QQC2.Button` that pops up a `QQC2.Menu`, as Kirigami apps make one. A `Select` is for choosing a value; this is for actions, and it keeps no choice.
- **Its menu is a context menu's** (§13.24): the same builder (`MenuItem`, `MenuSeparator`, `Menu` for submenus, checks and radio items), the same data (`Prop::Menu`, next to `Prop::ContextMenu`, so a menu button can have both), and each backend's context menu code, with the choice reported as `UiEvent::MenuItem`.
- **Caption, icon and style are a `Button`'s,** and each platform draws its own arrow:
  - AppKit: a pull-down's title is its first menu item, so the backend puts the caption and icon there and makes the menu again with it first. It keeps the caption as the accessible name when only the image shows.
  - GTK: a caption shows GTK's arrow. An icon with a caption is libadwaita's `ButtonContent` with `always-show-arrow`. An icon alone has no arrow, as GNOME's icon menu buttons have none. The actions are in a group of their own (`button.*`), apart from the context menu's.
  - Kirigami: its role is `Accessible.ButtonMenu`, for which the desktop style should draw a menu arrow, as Breeze does for a `QPushButton` with a menu. The menu pops up under the button and moves into the window's overlay while open, as context menus do. The button shows pressed while it's open.
  - WinUI: the content is a `Button`'s. Fluent has no subtle drop-down button, and `SubtleButtonStyle` would replace the template and its chevron, so borderless is a style that only clears the fill and border; it still fills on hover. A flyout opens above its target by default, so the menu is set to open below (`Placement` `Bottom`), as WinUI's own drop-down buttons set it.
- **`menu_with(|| entries)`** builds the menu again whenever what it reads changes, as `Menu::children_with` does in a bar; the composed path bar's menu of folders above uses it. `context_menu_with` is the same for context menus. It ran on AppKit and headless (`a_built_menu_follows_what_it_reads`); the composed path bar, GTK's and Kirigami's, only type-checks.
- **No click of its own:** clicking opens the menu, which is modal, so `perform` takes `MenuItem(id)` (the item's own path, as for context menus) and refuses `Activate`. It's a `Role::MenuButton` to assistive technology, named by its caption, and in the Tab order.
- **The example's tweaks:** a large control size on AppKit, the popover opening upwards on GTK (`direction`), `flat` on Qt, a pill-shaped `CornerRadius` on WinUI.
- **Where it has run:** AppKit, WinUI and headless. Only type-checked on GTK and Kirigami; to verify there: that Breeze draws the arrow for a `QQC2.Button` with the `ButtonMenu` role; that Kirigami's menu pops up under the button in the overlay; that WinUI's borderless style keeps the chevron; the arrow's room in each measure.

### 13.5 Checkbox

- **`mixed`** shows the mixed state over `checked` (some of what the box stands for is checked, as in "Select all"). Every platform has it: `NSControlStateValueMixed`, GTK's `inconsistent`, Qt's partly checked `checkState`, XAML's null `IsChecked`. The a11y tree has `mixed` beside `checked`.
- **A click leaves the mixed state, and lands where the platform lands:** AppKit checks the box; Qt's cycle from partly checked goes to checked; XAML's toggle from null goes to unchecked; GTK flips `active` underneath. The box reports what it landed on, the core absorbs `Mixed(false)`, and the app works out `mixed` again. Headless checks the box. Tests only check that the box leaves the mixed state and reports where it went.
- **Clicks never go back into it.** AppKit only allows the mixed state while it's shown, and Qt's `tristate` (which a partly checked `checkState` turns on) goes off after a click, so user clicks can't cycle into it. GTK leaves `inconsistent` to the app, so the backend clears it on a user toggle, as GTK apps do.
- **`checked` is kept underneath while mixed:** AppKit and Qt keep it on the node (the control has one state), GTK's `active` holds it, and WinUI keeps it in `shown_checked`. Leaving the mixed state always reports, whatever it lands on.
- **The example's tweaks:** the box after its label on AppKit (`imagePosition` trailing), GTK's `selection-mode` class (a round check where the theme has one), Qt's `spacing`, a round box on WinUI (`CornerRadius`).
- **Where it has run:** every backend, checked by eye.

### 13.6 Switch

- **No semantic options past `checked`.** What the platforms offer isn't shared: control sizes on AppKit, GTK's `state` apart from `active` (for settings that take time to apply), on and off text on WinUI, a caption Qt's switch draws itself.
- **The example's tweaks** are one of those each: a small control on AppKit, a delayed state on GTK, the switch's own text on Qt, `OnContent`/`OffContent` on WinUI.
- **Where it has run:** every backend, checked by eye.

### 13.7 RadioGroup

- **Radio buttons, one for each option, down a column.** Its options and choice are a `Select`'s (`Prop::Options`, `Prop::SelectedIndex`, `Changed(Index)`); the label is its accessible name, not drawn, as a select's.
- **None chosen is allowed,** unlike a `Select`: every platform's radio buttons can all be off, as HTML's start, so the core enforces nothing. `RadioGroup::selected` and `bind` take an `Option<usize>` (`IntoValue` takes an `Option` as a literal), and an index past the options chooses none. The user can't get back to none: a click on the chosen button changes nothing on every platform, which the tests check.
- **How far apart the buttons are is the platform's:** `NSStackView`'s own spacing on AppKit (8), 6 on GTK (as GNOME's dialogs space related controls), Kirigami's `smallSpacing`, `RadioButtons`' own on WinUI. The tests only check that each option adds height and the longest decides the width.
- **Each backend keeps the choice across new options** as the core does: the buttons already there keep their place on AppKit and GTK, with the new text; WinUI's items are replaced and the index set again; Qt's `Repeater` makes new buttons for a new model, so the backend reads the chosen one back first and sets it again.
  - AppKit: buttons with the same action in the same superview turn each other off, but only on a click, so the backend sets every button's state. A click on the chosen button sends its action again: the backend compares with the choice it knows.
  - GTK: `toggled` fires for programmatic sets (muted) and for the button turned off (ignored: only the one turned on reports).
  - Qt: `toggled` is the user's only; `perform` checks the button and emits it, as `Select` emits `activated`. The column hands its focus to the chosen button, or the first, so Tab reaches the group once. A layout sizes itself when polished, so `measure` polishes it first.
  - WinUI: `SelectionChanged` fires for programmatic sets too, guarded by `shown_index` as a select's is.
- **A radio group to assistive technology** (`Role::RadioGroup`), named by its label, holding a `Role::RadioButton` for each option, checked while it's chosen. The buttons are data, not nodes, as tabs are: they stand for the group, and the test kit's `click()` on one chooses its option (`A11yAction::SetValue`). Natively it's `NSAccessibilityRadioGroupRole`, GTK's `RadioGroup` role, `RadioButtons`' own peer, and a `Grouping` on Qt, which has no radio group role. It's one stop in the Tab order.
- **Left out:** side by side. WinUI's `RadioButtons` has `MaxColumns`, AppKit and GTK a horizontal stack or box, Qt a `RowLayout`. Tweaks get the container: the stack view, the box, the `ColumnLayout`, the `RadioButtons`.
- **The example's tweaks:** horizontal on AppKit and GTK, no spacing on Qt, a `Header` on WinUI.
- **Where it has run:** headless and natively on GTK and Kirigami (Arch Linux: GTK 4.22, libadwaita 1.9, Qt 6.11, Kirigami 6.30), `tests/radio_group.rs`, with the `radio_groups` story checked by eye on both. It has run on AppKit since (the story's capture on this machine), but these weren't checked there: that `NSStackView`'s `fittingSize` is the group's size when placed by frame (§13.16 has AppKit shrinking it to its fitting width), that `performClick:` on the chosen button leaves it on, focus with and without Full Keyboard Access, and how it looks. Only type-checked on WinUI; to verify there: that `RadioButtons` reports `SelectionChanged` when its items are replaced, that `ContainerFromIndex` has a button to focus before the group is first laid out, its measure, and how it looks.

### 13.8 Select

- **One option is always chosen, as with HTML's `<select>`:** the first, unless the app says otherwise; an index past the options chooses the first too. Only a `Select` without options has none. GTK forced this: `gtk::DropDown`'s selection autoselects and can't be cleared. `Prop::SelectedIndex` is `None` only when there are no options.
- **Sized as the platform sizes it:** `NSPopUpButton` for its widest item; `gtk::DropDown`, `ComboBox` and `QQC2.ComboBox` for the chosen one, so on those choosing can resize it (and relayout). The backends don't even that out.
- **Replacing the options keeps the chosen index** where it can, else chooses the first, as the core does; the core only sends the index when that changes it. Every backend replaces its items in one step and puts the index back itself: `NSPopUpButton`'s `removeAllItems`, a `StringList` splice, `ItemCollection.Clear` and a new QML model all lose the selection.
- **Options with the same text stay apart.** AppKit adds `NSMenuItem`s to the menu (`addItemWithTitle:` drops earlier items with the same title); XAML's items are `ComboBoxItem`s rather than boxed strings.
- **Choosing is what the pop-up does** (`select_option`, `A11yAction::SetValue`): AppKit performs the menu item's action (`performActionForItemAtIndex:`), GTK and XAML set the index and report it, and Qt sets it and emits `activated`, the user's signal. Opening the pop-up would start a modal loop, so tests never do.
- **Tab reaches it where the platform says:** macOS skips pop-up buttons unless Full Keyboard Access is on, as it skips buttons. The core puts selects in the focus order.
- **Kirigami opens its list in a window of its own** (`popupType: Popup.Window`), so it shows above a GPU surface (§13.26). That property is Qt 6.8's, and a binding to one Qt doesn't have fails to load, so the select sets it when it's made, if Qt has it (the property reads `undefined` before 6.8); on Qt 6.5 to 6.7 the list stays in the window, under a surface. Run on Qt 6.11 (Homebrew's, the QML alone); not on an older Qt.
- **Left out:** a borderless select for toolbars, the nearest, is AppKit's `bordered` and Qt's `flat` only: GTK's theme has no flat dropdown (GTK 4.14's `_common.scss` flattens dropdowns only inside `.toolbar`) and `GtkDropDown` has no API for one, and WinUI's `ComboBox` has no such style. Editable combo boxes are another widget on AppKit (`NSComboBox`) and missing from `GtkDropDown`.
- **The example's tweaks:** a borderless pop-up on AppKit (measured narrower), search in the pop-up on GTK (`enable-search`), `flat` on Qt, a `Header` on WinUI. Replacing the options runs the tweak again, like any prop change.
- **Where it has run:** every backend, checked by eye.

### 13.9 Slider

- **A step means what the platform's step means.** AppKit's is tick marks that the knob only stops at (`allowsTickMarkValuesOnly`); WinUI snaps drags of the knob to `StepFrequency` (not values set through automation or by the app) and steps by `SmallChange`; Qt's sliders only move by it from the keyboard (`snapMode` stays off unless asked). Without a step each keeps its default: WinUI's is 1, which it snaps to, and Qt's `increase()` moves by 0.1. Tests only check which way a step moves.
- **GTK snaps to its steps, as GTK apps do.** `gtk::Scale` has no stepped mode: the step is only a keyboard increment, and marks pull the knob in only within a few pixels. AppKit and WinUI stop on steps, so without this a `step(10.0)` slider on GTK would report whatever value a drag ends on. GTK apps round in `change-value`, so the backend does too (§1):
  - Only the user's moves snap. `change-value` carries drags, clicks, scrolls and keys; values the app sets pass through as given. `SetValue` emits `change-value` too, as a drag would, so it snaps. Without a step, GTK's is a tenth of the range, since it needs one to move at all.
  - A mark at each step, as AppKit draws tick marks for a step: `gtk::Scale::add_mark` below (or beside) the trough, redrawn when the step or range changes. `mitsuami::gtk::show_step_marks(scale, false)` in a tweak turns them off; since tweaks run after every prop, the setting lives on the scale (as object data) and marks are only redrawn when what they'd show changes. Marks make the scale taller, which it measures itself, and Adwaita draws the knob as a pin pointing at them rather than a circle, as in any GTK app with marks.
  - Marks only where steps are 24 px apart. While dragging, GTK holds the knob on a mark until the pointer is 12 px away (`MARK_SNAP_LENGTH`), and snapping always leaves it on one; with steps closer than 24 px it holds past the next step, and the knob jumps several at once (the example's 57 px slider with ten steps jumped four). At 24 px the hold is at most half a step, which snapping does anyway. The length the knob travels is the frame less the scale's CSS padding (12 px each side on Adwaita, read with the deprecated `StyleContext::padding`, the only API for it). Whether marks fit depends on the length and changes the thickness, so `measure` decides them for the length it's asked about, and `SetFrame` for the final one.
  - A wheel notch moves a page, as GTK scales do (`scroll_delta_to_value` in `gtkrange.c`), and the page is 10 steps, as `gtk_scale_new_with_range` makes it. Without a step a notch goes from end to end, and so does one with a coarse step (20 on 0–100). A smooth-scrolling touchpad moves by its distance instead. An app that wants a finer wheel sets the page in a tweak (`adjustment().set_page_increment`), as the slider example does; a step of 1 would do it too, but AppKit would draw 101 tick marks.
  - `stops_on_its_steps_where_the_platform_snaps` expects 80 for a move to 83 on GTK, where screen readers' moves snap too, and 80 or 83 elsewhere (WinUI keeps 83); `gtk_marks_its_steps_unless_told_not_to` checks the marks by the scale's height, and that dense or short sliders have none.
- **The value follows the range.** The platforms clamp the value to the range, so a range sent after the value would lose it: the core queues the value again after every range change.
- **A clamp while the range is set isn't the user's.** XAML reports it from inside the range's setter: a new `Slider` starts at 0 of 0–100, so a range of 120–480 moved it to 120 and WinUI reported that as a move, overwriting the app's value. The backend expects the clamped value before it sets the range, for `Slider` and `NumberBox` alike.
- **`orientation`**, the one semantic option every platform's slider has: AppKit `vertical`, GTK's and Qt's `orientation`, XAML's `Orientation`.
  - Up is more, everywhere. AppKit, Qt and WinUI put the minimum at the bottom; GTK puts it at the top, and GTK apps set `inverted` to turn that round, so the backend does too.
  - Length comes from the layout. Sliders have no natural width on AppKit (no intrinsic width), nor natural height when vertical: they're as long as the layout makes them, stretched in a column. The others measure theirs, but WinUI's slider measures only its thumb (18 wide): XAML apps stretch sliders or size them. Give a slider in a row `grow` or a width. Headless measures a vertical one 20 × 160.
- **Stepping and setting do what assistive technology or the keyboard does:** VoiceOver's increment, and the action as for a drag; GTK's step on the adjustment; UIA's RangeValue pattern; on Qt the keys' `increase()` or `decrease()`, then `moved()`, the user's signal.
- **Left out:** tick marks (not on Qt's), a drawn value (GTK only) and reversed direction (not on AppKit or Qt).
- **The example's tweaks:** a circular slider on AppKit (sized at its natural size), `draw-value` on GTK, `snapMode` on Qt, tick marks on WinUI (`TickFrequency`, `TickPlacement`).
- **Where it has run:** every backend, checked by eye; GTK's snapping on GTK and headless.

### 13.10 NumberInput

- **A spin box for a whole number,** where every platform has one: `NumberBox`, `gtk::SpinButton`, `QQC2.SpinBox`. AppKit has no single control, and its apps put an `NSStepper` beside a text field, so the backend does that (`mitsuami::appkit::NumberField`, a flipped view with both; the stepper holds the number, range and increment, and the field shows it). It came from 2ksbox's launcher (memory in MB, disk size in GB).
- **Whole numbers in an `i32`, because Qt's `SpinBox` holds an `int`:** a rule the core enforces since one platform makes the alternative impossible. Decimals typed or set are rounded: GTK with `digits` 0, Qt by its validator, WinUI in `ValueChanged` (`NumberBox` takes decimals; an emptied box, NaN, gets the last number back), AppKit when the field commits.
- **Typing reports when the edit is committed,** as every platform commits one: Return, or leaving the field (AppKit's cell `sendsActionOnEndEditing`). Buttons and arrow keys report at once. Text that isn't a number puts the number back.
- **What happens past an end is the platform's,** unless the app says with `wrap_around(value)` (`NSStepper.valueWraps`, GTK `wrap`, `NumberBox.IsWrapEnabled`, Qt `wrap`). By default AppKit's stepper wraps round and GTK, Qt and WinUI stop. Numbers typed past an end are clamped everywhere. `stops_or_wraps_at_its_ends_as_the_platform_does` expects each.
- **Held buttons repeat where the platform's do** (`NSStepper.autorepeat`, GTK's and Qt's buttons, WinUI's `RepeatButton`s), reporting each step. AppKit's stepper is left as AppKit makes it: on macOS 26 it repeats only inside an `NSSplitView` (a window with a `Sidebar`), faster the harder a Force Touch trackpad is pressed (driven by pressure events: about 100 ms between steps pressing normally, 33 ms harder, 16 ms harder still), and elsewhere steps once per click, in a bare AppKit app too. Filling the repeat in, by having the cell send its action on periodic events, stepped on every pressure event as well inside a split view, twice as fast as the stepper, and the showcase (a sidebar window) made it show. Found with small AppKit apps logging each step, `+[NSEvent stopPeriodicEvents]`'s callers (`NSStepperDidReceivePressureEvent`) and pressure events, held by hand; the test kit can't hold a native button down.
- **Inline spin buttons on WinUI.** `NumberBox` hides them by default; `Inline` is its documented spin-box mode, and a tweak can pick `Compact` or `Hidden`.
- **Sized as the platform sizes it:** GTK for its range's widest number, Qt for its text, WinUI by `Measure`, AppKit for the range's longest number in the field plus the stepper; so the range and number re-measure a `NumberInput` (`Prop::affects_measure` takes the kind). Headless measures one 96 wide.
  - WinUI sizes the box with room for its text box's clear button. The button shows only while the box has focus, in a column of its own; measured without it, the box left it over the value. While it's hidden, the backend shows it for its own measure and hides it again; while it shows, XAML's measure has it already. Checked by eye in captures on this machine: focused, re-measured while focused, and after focus moves on.
- **A spin button to assistive technology** (`Role::SpinButton`), named by its label, with the number as its value. On AppKit the field and the stepper both get the label, as VoiceOver finds them separately.
- **Left out:** decimals (Qt's holds an `int`).
- **Not tested: typing keys into one.** `synthesize` has no `NumberInput` path yet on any backend (each would drive the field inside); the suite uses assistive technology's `SetValue`, `Increment` and `Decrement`.
- **The example's tweaks:** a rounded bezel on AppKit's field, a vertical spin button on GTK (buttons above and below the number), `wheelEnabled` on Qt, `Compact` spin buttons on WinUI. The GTK tweak ran natively; AppKit's and Qt's are only type-checked.
- **Where it has run:** every backend and headless, checked by eye. `wrap_around` on AppKit and headless (`tests/number_input.rs`), only type-checked elsewhere.

### 13.11 Progress

- **A value from 0 to 1, or none** for work of unknown length: the platform's indeterminate, animated bar. It animates as the platform animates it: `NSProgressIndicator` and XAML's and Qt's bars on their own, GTK's by `pulse()` calls, which a 100 ms timer makes while the bar is indeterminate, as GTK apps do.
- **Leaving the indeterminate state rebuilds AppKit's bar.** On macOS 26, `startAnimation` gives the bar a layer (AppKit's Swift `ProgressIndicatorLayer`) that keeps drawing the indeterminate animation after `stopAnimation`, whatever the value or `indeterminate` say; removing its animations or redrawing doesn't help. Setting the style to spinning and back rebuilds the layer, so the backend does that when a bar gets a value after having none (`shows_its_value_after_being_indeterminate` goes there and back twice).
- **No natural width on AppKit** (no intrinsic width): a bar is as wide as the layout makes it, stretched in a column.
- **Left out:** orientation (GTK's only), paused and error states (WinUI's), a percentage label (GTK's). A circular style would be the one to share, but on GTK, Qt and WinUI a spinner is another control, and GTK's and Qt's show no value, so spinners are a widget of their own.
- **The example's tweaks:** a small bar on AppKit (measured thinner), `show-text` on GTK, `palette.highlight` on Qt (whether Breeze draws the bar with it isn't checked), `ShowPaused` on WinUI.
- **Where it has run:** every backend, checked by eye.

### 13.12 Spinner

- **A widget of its own, not a style of `Progress`:** every platform has a spinner for work of unknown length, but on GTK, Qt and WinUI it's another control than the bar, and GTK's and Qt's show no value.
- **`running`, on by default.** Stopped, every platform's spinner shows nothing (AppKit's with `displayedWhenStopped` off, which the backend sets) and keeps its size, so nothing around it moves; apps hide it with `Show` to give the room back.
- **A progress bar without a value to assistive technology** (`Role::ProgressBar`), as ARIA has it and as GTK and WinUI report theirs; AppKit reports a busy indicator natively. Named by its label; takes no focus.
- **Sized as the platform sizes it:** AppKit's regular spinner is 32 pt, GTK's 16 px, Breeze's two grid units, WinUI's 16 (its style's minimum); headless measures 16 × 16. A `ProgressRing` has no size until XAML loads it and applies its template, at the frame after it's added, so the backend asks the core to measure it again on `Loaded`, and waits for it in `settle`.
- **The example's tweaks:** a small spinner on AppKit, a 32 px size request on GTK, a larger implicit size on Qt, a determinate ring on WinUI (`IsIndeterminate`, `Value`), which only WinUI's spinner can show.
- **Where it has run:** every backend, checked by eye.

### 13.13 Separator

- **Every platform has one but WinUI:** a separator `NSBox`, `gtk::Separator`, `Kirigami.Separator`. XAML's separators are for menus and command bars (`MenuFlyoutSeparator`, `AppBarSeparator`), so WinUI's is a `Border` in `DividerStrokeColorDefaultBrush`, 1 epx across (`MinHeight` or `MinWidth` in its style), as Fluent apps and the Settings app draw dividers. It has no automation peer, so UIA doesn't list it, as it doesn't list those apps' dividers.
- **`orientation`, horizontal unless told otherwise,** always sent. GTK's separator has one and reads it back; AppKit's takes it from its frame's shape and Kirigami's has none (its frame is long one way), so those keep it on the node, as WinUI does.
- **As thick as the platform draws it; the layout gives its length.** It measures 0 along its length (1 on GTK and Kirigami, their minimum), so it spans a column that stretches its children, or a row when vertical, as the platforms' own separators span a box (`hexpand` on GTK, `Layout.fillWidth` on Kirigami). It isn't stretched when the app aligns the column's children otherwise; `align_self(Align::Stretch)` does. AppKit's is 1 pt unless its intrinsic size says more; headless measures 1. WinUI's 1 epx is 1.33 px at 150% (XAML rounds to whole pixels): the backend rounds it to the nearest pixel rather than up, which drew it 2 px thick.
- **A separator to assistive technology** (`Role::Separator`), with no name, as ARIA, GTK and Qt (`Accessible.Separator`) have it. It takes no focus and no actions.
- **The example's tweaks:** `transparent` on AppKit and the `spacer` class on GTK keep its room but draw nothing; Kirigami's light `weight`, as inside lists and cards. WinUI's has none of its own.
- **AppKit places it by its frame, not its alignment rect.** An `NSBox` separator's alignment insets follow its frame's shape, so a vertical one placed through them while still horizontal (as it's created) landed 2 pt past each end and 0 wide, and the mirror check failed on every vertical separator. It's a line with no bezel, so its frame is what the user sees.
- **Where it has run:** WinUI (tests and a capture, checked by eye) and AppKit (tests). Only type-checked on GTK and Kirigami; to verify: Kirigami's `weight`.

### 13.14 TextInput

- **`read_only`,** the one semantic option every platform's text field has: AppKit `editable` off (still `selectable`), GTK's `editable`, Qt's `readOnly`, XAML's `IsReadOnly`. The text can be selected and copied, and the app can still set it. The a11y tree gets `read_only`.
  - Focus is the platform's. GTK, Qt and WinUI keep read-only fields in the Tab order; AppKit's refuse keyboard focus (`acceptsFirstResponder` is false) unless Full Keyboard Access is on, and take it from a click. The core's focus order still lists them: AppKit skips views that can't become key.
  - Nothing can be typed into one. `synthesize` returns `ActionError::ReadOnly` for any key, before focusing, and `perform(SetValue)` too: AppKit can't deliver keys to a field it won't focus, and WinUI's backend edits through the selection, which `IsReadOnly` doesn't stop.
- **`input_purpose(…)`:** Email, Url or Phone, the purposes every platform has, used as the platform uses one (an on-screen keyboard, autofill, input methods): AppKit's `contentType`, GTK's `input-purpose`, XAML's `InputScope`, Qt's `inputMethodHints`. It checks nothing typed. Others one platform lacks were left out: AppKit has no number or digits type, Qt no name hint.
- **`Submit` is Return,** and only Return: not Tab, a click elsewhere or focus loss, though AppKit's field action fires for those too, which was a real bug.
- **Where typing goes after the app sets the text is the platform's.** A field focused when the window opened keeps its caret where the set left it: GTK's `set_text` leaves it at the start, AppKit at the end. The tests accept either.
- **Left out:** a length limit (AppKit needs a formatter), icons in the field (GTK's), a header (WinUI's). Secure entry is another control on AppKit and WinUI, so it is a widget of its own.
- **The example's tweaks:** a borderless field on AppKit (macOS 26 draws a rounded bezel as it draws the default one, so a bezel tweak showed nothing), a search icon on GTK, `maximumLength` on Qt, a header on WinUI.
- **Where it has run:** read-only on every backend, checked by eye. `input_purpose` on AppKit and headless (`tests/forms.rs`), only type-checked elsewhere.

### 13.15 PasswordInput

- **A widget of its own,** not a `TextInput` option: AppKit and WinUI make password fields from other controls (`NSSecureTextField`, `PasswordBox`), and GTK's `PasswordEntry` isn't a `gtk::Entry`. Qt uses `Kirigami.PasswordField`, KDE's own, a `QQC2.TextField` that echoes bullets. It has a text field's props and events: `Value`, `Placeholder`, `Enabled`, `Changed(Text)`, `Submit`. No read-only: `PasswordBox` has none.
- **Hidden as each platform hides it.** Bullets everywhere; a button that shows the text on Qt and WinUI, their default; GTK's peek icon is off, GTK's default. Showing it is up to the platform and the app's tweaks. AppKit has no reveal button, so that isn't shared.
- **A text field to assistive technology,** as every platform exposes one (AppKit's secure subrole, Qt's and UIA's password flag): role `TextField`, named by its label or placeholder, with `password` set and no value. `native_state` still reports the text, for the mirror check.
- **WinUI edits at the end.** `PasswordBox` has no caret or selection API, so synthesized keys append to or trim `Password`, where typing into a focused box goes, and it has no settable Value pattern, so `SetValue` sets `Password`.
- **The example's tweaks:** no bullets on AppKit (`echosBullets` off on the cell), the peek icon on GTK, `showPassword` on Qt, asterisks on WinUI (`PasswordChar`).
- **Where it has run:** every backend, checked by eye.

### 13.16 SearchInput

- **Each platform's search field:** `NSSearchField`, `gtk::SearchEntry`, `Kirigami.SearchField`, and on WinUI an `AutoSuggestBox` with `QueryIcon` Find, as the WinUI docs make a search box, with no suggestions (a list would open as a pop-up). A widget of its own: GTK's `SearchEntry` isn't a `gtk::Entry`, nor WinUI's box a `TextBox`. It has a text field's `Value`, `Placeholder`, `Enabled` and `Changed(Text)`; no read-only, and no `Submit`: Return is a search.
- **It searches when the platform does:** `UiEvent::Search(text)`, `on_search`, is the platform's own signal, with its own timing. AppKit sends its action once typing pauses, longer after shorter text (measured: 500 ms after one character, 400 after two, 200 from three on), and on Return; GTK `search-changed` 150 ms after typing (`search-delay`), at once when emptied, and `activate` on Return; Kirigami `accepted` 100 ms after (`autoAccept`, `Units.shortDuration`; 2 s with `delaySearch`), and on Return; WinUI has no pause: every user `TextChanged` searches, and `QuerySubmitted` on Return or the find icon. Headless searches at once, as WinUI. The core doesn't even the timing out. `on_input` still reports every edit. A search delay isn't shared: WinUI has none.
- **Clearing searches.** The clear button (AppKit's cancel button and Escape, GTK's clear icon, Kirigami's clear action, WinUI's delete button) empties the field, which reports `Changed("")` and a search for `""`. AppKit sends that search twice, before and after the change, as its own apps get it. Escape is the platform's: AppKit clears; GTK only emits `stop-search`, on which GNOME apps hide their search bar, and the text stays.
- **Text the app sets is never searched for.** AppKit sends no action for it, and WinUI's `TextChanged` is checked against `shown_text`. GTK and Kirigami search for any text change, after their delay, so their backends report a search only after a user edit (GTK: a flag set in the unmuted `changed`; Qt: `textEdited`), on Return, or when the clear action emptied it. Kirigami's clear action calls `clear()`, which isn't a `textEdited`, so its edit is reported as the search comes.
- **Assistive technology's edits search:** `SetValue` reports `Changed` then `Search` itself, and the delayed platform search that follows is dropped (GTK, Kirigami).
- **Role `SearchField`:** AppKit's search field subrole, GTK's search box, ARIA's searchbox; Qt and UIA expose a text field (Kirigami's `Accessible.searchEdit`; the edit inside WinUI's auto-suggest box). Named by its label or placeholder, its text as its value.
- **The platform's placeholder** shows unless the app gives one: "Search" on AppKit, "Search…" on Kirigami; GTK and WinUI have none.
- **About a text field's size:** 24 pt tall on AppKit, as a text field; the others measure theirs, WinUI's as a text box's (at least 200 wide).
- **WinUI edits through its template's `TextBox`,** found in the visual tree once loaded: keys go through its selection, as a `TextInput`'s, `SetValue` sets its text, and focus goes to it. A synthesized Return reports the search itself, since only a real key raises `QuerySubmitted`.
- **AppKit's tests fire due timers** (`limitDateForMode:` in the backend's `settle`) while a search field is shown, or its searches would never come: offscreen test windows get no run loop. Not always yet: tables and tab views have timers too, which change what their stories capture, and a radio group's stack view shrinks to its fitting width when AppKit lays it out (its frame then disagrees with the core's).
- **The example's tweaks** are each platform's timing: searching on every keystroke on AppKit (`sendsSearchStringImmediately`), a one-second `search-delay` on GTK, `delaySearch` on Kirigami, and a header on WinUI.
- **Where it has run:** AppKit and headless. Only type-checked on GTK, Kirigami and WinUI (the Kirigami QML linted with Homebrew's `qmllint` on a `QQC2.TextField`, without Kirigami).

### 13.17 TextArea

- **Each platform's text view, in its scroll view:** AppKit's `scrollableTextView` in a bezel, as Interface Builder makes one, plain text in the system font with undo; a `gtk::TextView` in a framed `gtk::ScrolledWindow`; XAML's `TextBox`, which scrolls itself; a `QQC2.TextArea` in a `QQC2.ScrollView`, which the desktop style frames as a field. It has a text field's `Value`, `Placeholder`, `ReadOnly`, `Enabled` and `Changed(Text)`, and no `Submit`: Return starts a new line on every platform.
- **Its lines wrap everywhere.** AppKit's text view wraps; GTK's, XAML's and Qt's don't by default, and GNOME, Windows and KDE apps turn wrapping on for a text area (GTK `WordChar`, XAML `TextWrapping.Wrap`, Qt `TextEdit.Wrap`), so the backends do too. Fluent's multi-line text box also shows its vertical scroll bar (`Auto`), which XAML's default style hides. GTK's text view gets 6 px margins, as GNOME apps give a framed one; GTK's own has none.
- **`line_wrap(false)`:** lines as long as their text, scrolling sideways. AppKit follows Apple's recipe for a horizontally scrolling text view (horizontally resizable, a container as wide as it likes, a horizontal scroller); the text view is fitted to its text when it's set, since the text system would only lay it out at the next display, and until then it stayed its clip view's width and clipped the lines. GTK `wrap-mode` `None` with an automatic horizontal policy; XAML `NoWrap` with an `Auto` horizontal scroll bar; Qt `wrapMode` `NoWrap`.
- **`lines` is its natural height** (3 unless set), which no platform has: the core's, like HTML's `rows`. Each backend measures that many lines of the area's own font inside the platform's insets and frame: AppKit from the font's ascender, descender and leading and the scroll view's frame for that content (3 lines of the body font are 50 pt); GTK from a Pango line, as the scrolled window's minimum content height; Qt from `FontMetrics` and the paddings; WinUI as an empty text box in the box's font holding that many lines (a hidden probe in each window's root: XAML measures text boxes only in a live tree; the area itself grows with its text, and a `TextBlock` spaces lines differently from a text box, 18.7 against 18.2 epx at 150%, so neither can stand in). It's a text field's width (200) wide, and doesn't grow with its text, which scrolls; the layout can size it otherwise.
- **Tab is the platform's.** AppKit's, GTK's and Qt's text views insert a tab; XAML's text box takes none and moves focus on. The tests say so; headless inserts one.
- **No placeholder on AppKit and GTK,** whose text views have none (AppKit's is private API). The prop is kept for the mirror check; it still names the area to assistive technology without a label.
- **Disabled on AppKit** is how AppKit apps disable a text view, which has no enabled state: neither editable nor selectable, in the disabled text colour. Read-only stays selectable.
- **Disabled, it shows no selection.** Disabling it collapses the selection to its start, on every backend (AppKit's `setSelectedRange:`, GTK's `place_cursor` at the insert mark, Qt's `deselect()`, XAML's `SelectionLength` 0), as disabling a field ends its editing. AppKit's text view went on showing the selection otherwise; the others weren't checked, and the test runs through each one's tweak.
- **A text area to assistive technology:** role `TextArea` (AppKit's text area, GTK's and Qt's multi-line text, UIA's multi-line edit), named by its label or placeholder, its text as its value.
- **Its lines end in `\n`.** XAML's text box ends them in `\r`, whatever it's given, so the WinUI backend reads text boxes' text back with `\n`.
- **Change signals:** AppKit's `textDidChange:` is the user's only; GTK's buffer `changed` is muted while commands apply, as an entry's is; WinUI's `TextChanged` is checked against `shown_text`, as a text box's is. Qt's `TextArea` has no user-only signal (`textEdited` is `TextField`'s): the QML marks the backend's own sets (`mitsuamiSetting`) and reports the rest as `mitsuamiEdited`.
- **The example's tweaks:** continuous spell checking on AppKit, monospace on GTK, `wrapMode` `WrapAnywhere` on Qt (lines broken inside words too), a `Header` on WinUI. The story's tweak is a fixed-pitch font on AppKit and GTK, and Qt's and WinUI's as the example's; those two are only type-checked.
- **Where it has run:** AppKit, WinUI and headless. Only type-checked on GTK and Kirigami (the Kirigami QML linted with `qmllint`). On WinUI, `read_only_and_enabled_follow_their_signals` fails: typed after it was made editable, the text isn't in the native box (not looked into yet). `line_wrap` on AppKit and headless only.

### 13.18 Image

- **A picture from a file or from pixels in memory,** in each platform's image view: `NSImageView`, `gtk::Picture`, XAML's `Image`, a Qt Quick `Image` (`Kirigami.Icon` is for themed icons: `Icon`). 2ksbox's shader preview is pixels the app renders, so pixels aren't a detour through a file.
- **Pixels are straight RGBA8, sRGB, with a scale** (pixels to a point), so an app can render at the window's scale factor and have each pixel shown as one. Each backend makes its own image from them: an `NSBitmapImageRep` retagged sRGB, a `gdk::MemoryTexture`, a `WriteableBitmap` (premultiplied BGRA, converted), and on Qt a `QImage` served by a `QQuickImageProvider` registered as `mitsuami`, Qt's way to give QML images from memory. Every set of pixels gets its own `image://` URL there, and `cache` is off, so QML never shows a stale one. `Pixels` holds an `Arc`, so props clone cheaply, and prints its size, not its bytes.
- **A file is read when it's set.** AppKit, GTK and Qt (`asynchronous: false`) decode it right away; WinUI decodes in the background, so its `ImageOpened` and `ImageFailed` send `UiEvent::Remeasure` and the core measures it again. A file that changes under the same path isn't read again: set other pixels, or another path. A missing or unreadable file shows nothing and measures zero. Headless reads a PNG's size from its header, and gives other files no size.
- **Its natural size is the image's in points:** pixels over their scale, or the file's size as the platform reads it (AppKit honours a PNG's resolution; the others count pixels).
- **Only the fits every platform has:** `ImageFit::Contain` and `Stretch`, sent only if the app picks one, since the defaults differ: AppKit shrinks proportionally but never enlarges, GTK and XAML contain, Qt stretches. Aspect fill is missing from `NSImageView`, and "only shrink" from Qt and XAML, so those are tweaks.
- **Smoothing isn't shared** (Qt's `smooth` is the only switch), so pixel-sharp scaling is a tweak on Qt; elsewhere, pixels made at the window's scale aren't scaled at all.
- **An image to assistive technology** (`Role::Image`), named by its label (GTK's `alternative-text`, Qt's `Accessible.Graphic`); without one, decorative. It takes no focus. No platform gives an image's source back, so backends keep it on the node for the mirror check.
- **`draws_what_it_is_given` checks the pixels on screen:** a capture of the window, blue and red where the fixture has them, so a mirrored or swapped-channel image fails.
- **The example's tweaks:** a photo frame on AppKit (`imageFrameStyle`), `content-fit` cover on GTK, `smooth` off on Qt, `UniformToFill` on WinUI.
- **Where it has run:** every backend and headless, checked by eye. An unpackaged WinUI app loads a `BitmapImage` from an absolute `file:///` URI; paths with spaces or `#` are unverified. WinUI's `settle` waits for files being decoded, as it waits for spinners to load: `expect` only retries while the app has tasks, and XAML's decoding isn't one.

### 13.19 Icon

- **An icon from the platform's own set, by its name there:** an SF Symbol in an `NSImageView` (or else an image AppKit has by that name), a themed icon in a `gtk::Image` or a `Kirigami.Icon`, a Segoe Fluent Icons glyph in a `FontIcon`, as WinUI's `NavigationView` items show them. Names differ per platform, so the app picks them with `platform!`. A shared set of names mapped per platform could come later; the names stay each set's own, so an app can use any icon its platform has. An empty name shows nothing. A name the set lacks shows the way the platform shows one: nothing on AppKit, GTK's missing-image icon, Kirigami's fallback icon, the font's fallback glyph on WinUI. Buttons (§13.2), tabs and sidebar items (§14) take the same names.
- **Each platform's own size** unless the app gives `icon_size` in points. On AppKit that's the symbol's point size, as a font's: a symbol's shape sets its frame (at the default size, a disc is 15 × 15 and a trash can 15 × 17). Elsewhere it's the side of a square: GTK's 16 (`pixel_size`, whole pixels), Kirigami's `iconSizes.small`, 16 at the default scale (what KDE's buttons and menus show inline), and XAML's `FontIcon` `FontSize`, 20 by default. Tests compare sizes, never a number.
- **Drawn in the colour the platform gives icons,** which follows dark mode: AppKit's image view draws a lone symbol in a secondary grey, while GTK, Kirigami and XAML give symbolic icons the text colour.
- **`color` takes the colours `Text` takes** (`Prop::TextColor`): a semantic one is the platform's own, so it follows dark mode, high contrast and the accent without the core sending it again; `Rgba` is fixed. Symbolic icons take it (SF Symbols, `-symbolic` theme icons, Fluent glyphs), and icons in full colour keep theirs.
  - AppKit: the image view's `contentTintColor`.
  - GTK: CSS `color`, which symbolic icons are drawn in. Semantic colours use the labels' classes (`dim-label`, `accent`, `error`, …). A fixed colour gets a class of its own, with its rule in one style sheet for the display, since an image has no Pango attributes.
  - Kirigami: `color`, bound as a label's is, to `Kirigami.Theme`'s colours. Without one it's `transparent`, Kirigami's default, which leaves the icon to the theme.
  - WinUI: a style whose `Foreground` setter is the colour's theme resource, as for text. It's kept on the node, since a resolved brush can't be told from another.
  - `draws_in_its_colour` checks the pixels in a capture: red where a red icon is, none where an icon has its own colour.
- **Symbol weights and rendering modes** (hierarchical, palette, multicolour) are AppKit's alone, so they're tweaks.
- **An icon to assistive technology** (`Role::Image`), named by its label; without one it's decorative. It takes no focus. AppKit's symbol images have no name to read back, so its backend keeps the name and size on the node; GTK, Kirigami and WinUI read them from the widget.
- **Where it has run:** every backend and headless (`tests/icon.rs`, the pixel test too).
### 13.20 Group

- **A box around related content, under an optional heading,** as each platform groups settings: an `NSBox` on AppKit (the title inside at its top), a `heading` label over a libadwaita `card` on GTK (as `AdwPreferencesGroup` lays out a group), a `QQC2.GroupBox` on Qt (Breeze draws the title inside its top), and on WinUI a `BodyStrongTextBlockStyle` heading over a card, as Windows 11's Settings has them (WinUI has no group box). Where the heading goes is each platform's.
- **The core lays out the content,** as a column's (`gap`, and any style), inside the platform's insets: `PlatformMetrics::group_insets`, or `titled_group_insets` with a heading, added to the app's padding. Backends measure them from a probe, as tab views' insets (§14.5). The group is at least as wide as its heading, which backends measure as they measure a tab strip: a group is one of the two containers the core measures.
- **The box is drawn behind the content,** in the same host: every backend's group is a layout host with the platform's box (and heading) as its first children, sized with it, so the core's children are placed as in any container.
  - AppKit: the `NSBox` follows the host with an autoresizing mask, and inserts count past it. A box made empty and grown keeps its content view's first frame, so the probe is made at its size. Its content margins are `NSBox`'s own, 5 points (17 at the top with a title on macOS 26), tight beside the others': an app that wants more adds `padding`.
  - GTK: content is 12 inside the card, as GNOME apps put it, and the heading is 12 above; its height comes from a throwaway label.
  - Kirigami: the insets are a probe `GroupBox`'s paddings (`topPadding` grows with a title), and its implicit size is the empty group's measure.
  - WinUI: the card is a `Border` drawn as the Community Toolkit's `SettingsCard` draws one (the card brushes, a 1 epx border, `ControlCornerRadius`, 16 of padding), and the heading is 6 above it, as the WinUI Gallery's settings page spaces them. The heading's height starts from its line height (20), and the first one that loads is measured, sending `MetricsChanged` if it differs, as tab views do for their bar.
- **A group to assistive technology** (`Role::Group`), named by its heading, around its content; it takes no focus. The platforms' own heading labels and boxes are hidden from assistive technology, so there's one group, not two.
- **Tweaks get the box** (the `NSBox`, GTK's card, the `QQC2.GroupBox`, WinUI's card `Border`), after the group's props and again when they change (`a_tweak_gets_the_box_after_its_props`). A tweak can move the heading or change the border, so after measuring a group the core asks the backend where that group puts its content (`Backend::group_insets`), and uses the metrics' insets only when it says nothing. AppKit reads a probe box set up as the group's (title, its position and font, the box type, border and margins), since the box itself may be too small to say: with the title moved to the bottom, the content moved up under the border and ran over the title (`a_tweak_that_moves_the_heading_moves_its_room`). GTK, Qt and WinUI say nothing yet, so their tweaks keep the metrics' insets.
- **Left out:** a collapsible group (AppKit and Qt Quick have none).
- **The example's tweaks:** the title at the bottom on AppKit (`titlePosition`), GTK's `activatable` class on the card, `flat` on Qt, square corners on WinUI.
- **Where it has run:** AppKit and headless (`tests/group.rs`, the `groups` story). Only type-checked on GTK, Kirigami and WinUI; to verify there: each platform's insets (the probes outside a window, WinUI's estimate and its re-layout); that the boxes stay behind the content and don't take its clicks; how they look.

### 13.21 ScrollView

- **The native scroll container scrolls; the core lays out its content** (§4). Scroll changes are reported for the user's scrolls and the app's alike (`Scrolled`).
- **`scroll_bars(false)`,** the one semantic option every platform's scroll view has: hide the bars and keep scrolling, by wheel, trackpad and touch, as a strip of photos does. AppKit turns its scrollers off, GTK's policy is `External`, Qt's scroll bar policy `AlwaysOff`, XAML's visibility `Hidden` (not `Disabled`, which stops scrolling). Shown, they're the platform's own, overlay or not. "Always shown" isn't shared: on macOS it's the user's setting. AppKit keeps the axes on the node, since hidden scrollers read as axes turned off.
- **Shift+wheel scrolls sideways on WinUI too.** AppKit, GTK and Qt do it, and so do Windows' own apps (Explorer, Edge), but XAML's `ScrollViewer` doesn't (microsoft-ui-xaml#8553, closed as not planned), so the backend fills it in (§1): a `PointerWheelChanged` handler on the scroll view's content, which sees the wheel before the scroll viewer, scrolls sideways by XAML's 48 px a notch when Shift is down and the content is wider than the view. The content gets a clear background, as a host with a tooltip does: a `Canvas` without one is only hit where its children are, and the wheel passed it by between them. Scrolls apply at XAML's next layout, so quick notches add up from where the last one sent the view. Not covered by tests, since wheel input can't be synthesized here: to be tried by hand in `examples/scroll_view.rs`.
- **Controls in a new WinUI scroll view are measured once it's in the live tree.** A `ScrollViewer` shows its content only after a layout pass applies its template, and XAML measures only elements in the live tree, so controls in it measured as if untemplated (in the todos example, whose list scrolls, Remove buttons were 0 wide and rows 19 px tall). After each batch the backend lays out any scroll view that's live while its content isn't (`UpdateLayout`), which connects the content before the core measures it. Found and run on WinUI (the todos test's layout); the other backends weren't checked for the same.
- **Left out:** elasticity and borders (AppKit's), classic scroll bars (GTK's), the wheel's step (Kirigami's), inertia and zoom (WinUI's). A border narrows the visible area, which the core doesn't know about: content can lose a point or two at its edges. On Kirigami the desktop style's scroll bars take room from the view the same way, so a vertical bar covers the content's right edge.
- **The example's tweaks:** a bezel border on AppKit, `overlay-scrolling` off on GTK, one line per wheel notch on Qt (`WheelHandler.verticalStepSize`), `IsScrollInertiaEnabled` off on WinUI.
- **Where it has run:** every backend, checked by eye.

### 13.22 List

The model is in §4: the platform realises rows and owns the selection, and the core mounts, keys and lays out what's in them.

- **Selection** is `selection_mode` (`None`, `Single`, `Multiple`), `selected` (bound to the app's keys) and `on_activate`. Rows read as list items named by their text.
- **Rows drag their files out** with `drag_files` (§13.31).
- **The selection mode can change while the list shows** (it takes a signal). The selection keeps what the platform keeps, at most what the new mode holds, and the rows let go are reported as the user's, as for removed rows:
  - AppKit's table keeps its selection when it stops allowing several rows, or any, so the backend keeps `selectedRow` (the row selected last) for Single and none for None.
  - GTK has no mode, only selection models to swap: the backend carries over what the new model holds (the first row for Single), as AppKit does. The swap also takes the focused row out of the view while the window keeps it as its focus, so keys went nowhere; the backend gives focus back to the list, as AppKit's table keeps it. Removing the focused row does the same (GTK doesn't move the window's focus off the row's item widget, and doesn't report it), so focus goes back to the list there too.
  - Kirigami's selection is the QML view's own, and keeps the first row.
  - XAML's `ListView` clears its selection on every mode change, even to one that holds more, and WinUI keeps that.
- **Selected rows that go are deselected by the core, before the rows change.** Platforms report `Changed(Rows)` when selected rows are removed, but that report arrives after whatever the app did next: a rename that replaced the selected row and selected the new one had the stale report clear the new selection, and the core's `Selected` then disagreed with the platform's. So when rows change, the core sends the selection that stays first (the platform then removes no selected row and reports nothing), then the rows, then the whole selection again, which also picks up keys selected before their rows existed. The selection is sent in row order, as platforms report theirs. Found by the `files` example.
- **`list_style`: how a list sits in its surroundings is a semantic choice,** like `ButtonStyle`: `Plain` (edge to edge: sidebars, main content) or `Framed` (a bordered box on the content background: a list in a form or dialog), each drawn the platform's way: the bezel border on AppKit, `ScrolledWindow`'s `has-frame` on GTK, Breeze's scroll view frame on Kirigami, a card's border on WinUI. `Automatic`, the default, is `Plain` everywhere. Where platforms' habits differ (KDE frames more lists than GTK or macOS), the app picks per platform: `.list_style(platform! { kde => ListStyle::Framed, _ => ListStyle::Plain })`. A frame takes room from the rows, which backends report with `RowWidth`.
- **Rows the platform keeps are its own.** Which rows it realises is its call, and the tests allow for it: AppKit prepares a few around the view, GTK about 200, and GTK keeps its cursor row and selected rows bound wherever it scrolls. Where rows go when rows are inserted above the view is its call too (GTK keeps the rows in view where they were).
- **AppKit:**
  - A view-based `NSTableView`, one column and no header, in an `NSScrollView`: plain style, no intercell spacing, row heights from `tableView:heightOfRow:`, the hosts' heights and, for rows not shown yet, the app's estimate or else the first row measured. The estimate stays put once known: the table keeps the heights it read, so a drifting estimate left rows above the view at stale heights.
  - Rows shown are the table's row views: `tableView:didAddRowView:forRow:` and `didRemoveRowView:` report `RowShown` and `RowHidden`. Each cell is a plain view that takes the row's host when it arrives. The data source and delegate only read the list's own data (keys, heights, hosts, cells) and emit, since the table calls them in the middle of `apply`.
  - Tables add row views in a layout pass, at the next display, which offscreen windows never get. The backend lays its lists out at the end of each `apply` and in `settle`, so rows a change or a scroll reveals are reported and built in the same run-loop turn, before anything is drawn. Scrolling doesn't mark the table as needing layout, so it's marked first.
  - Data changes reload, then reselect by key. The reload lays out right away, and only the difference in rows shown is reported: rows that stay keep their state. Height changes go through `noteHeightOfRowsWithIndexesChanged:` with animations off; a list scrolled to its end stays there when rows turn out taller than estimated.
  - Return activates the selected row (a table subclass's `keyDown:`), as it opens the selected item in Finder and Mail, only without ⌘, ⌥ or ⌃, which leave it to the list's keys (§13.29); double-click is the table's `doubleAction`. Home and End only scroll, as in every AppKit list.
  - Known gap: captures show no row selection. macOS 26's `NSTableRowView` sets its selection on its layer instead of drawing it, and `cacheDisplayInRect:` only runs views' drawing code. The selection itself is real (the tests check it natively); only baselines miss it.
- **GTK:**
  - A `gtk::ListView` in a `ScrolledWindow`, over a `gio::ListStore` of row keys, with `NoSelection`, `SingleSelection` or `MultiSelection` around it. A `SingleSelection` only takes `set_selected` (`set_selection` does nothing), and a splice that replaces items drops their selection even when the same keys come back, so the backend reselects by key after every data change. Data changes are one splice, of the part between the rows that stayed at the start and at the end.
  - Rows are the factory's binds. Each item's child is a box that takes the row's host when it arrives and is as high as the estimate until then: empty cells would be 0 px high, and GTK would bind every row. GTK binds and unbinds during layout, and unbinds and rebinds a row it keeps when the model changes, so the rows bound are compared with the rows reported once the main loop is idle, and only the difference is reported.
  - Rows it keeps but doesn't place (only the rows in view are allocated) are positioned from the nearest placed row and the heights of the rows between.
  - List views bind and place rows when allocated, at the next frame, and on a test display frames stall (§15, GTK), so `settle` allocates each changed list's scrolled window again at its frame, as its host does, which lays the list view out synchronously.
  - GTK 4 can't inject keys and its list keyboard handling has no signals to emit, so synthesized arrows, Home, End and Enter do what it does: move the selection and show it, or activate. `list.scroll-to-item` scrolls to a row (`ListView::scroll_to` needs GTK 4.12). Row padding is reset with CSS (`listview.mitsuami-list > row`).
- **Kirigami:**
  - A QML `ListView` over the row keys, with Qt Quick Controls' `ItemDelegate`s, so the style draws the rows' highlight. Each delegate holds its row's host (`mitsuamiHost`) and is as high as it, or the estimate until it arrives. No C++: the view's QML keeps the selection, activation and keyboard handling, and Rust sets properties (`mitsuamiKeys`, `mitsuamiSelected`, `mitsuamiScrollTo`, …) and listens to three argument-less signals.
  - Rows are the delegates. Delegates announce their creation and destruction with `Qt.callLater`, which coalesces them to once per event loop turn; Rust then compares the rows that have a delegate with the rows it reported. A data change resets the view and recreates its delegates, so without that a row would be hidden and shown again, and lose its state. The scroll position is put back after the reset.
  - The keyboard moves the current row, which is the selection (`keyNavigationEnabled`), with Home, End and Return (without Ctrl, Alt or Meta) added in `Keys.onPressed`.
  - Multiple selection is built in the QML, as KDE's apps do theirs: a QML `ListView` has no selection model, so the backend adds Qt's extended selection, which Dolphin and Qt's item views have: Ctrl-click toggles a row, Shift-click and Shift with the arrows select the rows from the anchor, and Ctrl+A selects them all. Rows are clicked through a `TapHandler`, because `clicked` doesn't say which modifiers were held. The tests can't hold modifiers, so this was checked with a `qmltestrunner` test over the list's QML.
  - The delegates keep the style's padding and insets at the sides, though rows don't use the padding: Breeze draws the highlight from both, and with zero padding it cut the highlight short at the right.
  - `ListView`'s content starts at `originY`, which moves as rows turn out taller or shorter than estimated: offsets and row positions are taken from it. At the end, a list stays there as rows are measured (`positionViewAtEnd()`, which also handles a scroll to the end over estimated rows). Qt keeps the heights of rows it has laid out; the estimate is the app's, or the first row measured.
  - List views place delegates when they polish, before a frame: `settle` polishes the windows, so rows are where the view says when tests look.
- **WinUI:**
  - A `ListView` whose `Items` are the row keys, boxed strings, so XAML's own collection takes the inserts and removes (one splice per data change, as on GTK). An item container style from markup takes the padding, margin and minimum height off `ListViewItem`s.
  - Rows are realised containers. `ContainerContentChanging` fires when a container is realised for a row and when it goes to the recycle queue (a reused container fires for its old row, then its new one). Each container's content is a `Canvas` cell that takes the row's host, as high as it or the estimate. The rows realised are compared with the rows reported once the dispatcher is free, and at the end of each `apply`, after `UpdateLayout` realises what a change brought into view.
  - Selection is `SelectedIndex` or `SelectedItems`; the selection set is recorded first, so the later `SelectionChanged` finds it reported. Double-clicks find their row up the visual tree to its container; Return activates the selected row (`PreviewKeyDown`), without Ctrl or Alt. The list's scroll viewer is its template's, found in the visual tree.
  - Rows are placed from the scroll viewer's content, not the view: XAML applies a scroll at its next layout, so right after one the view's transform plus the offset counted the scroll twice.
  - A cell is never 0 high. A row keeps its last measured height, or the estimate until it has one, including while a new host waits for its frame: rows above the view that shrank to 0 and grew back made XAML shift the offset to keep the rows in view still, a little further each layout pass, and a scroll to row 500 crept to the end of the list.
  - Focus is a row container's, so the list has it when the window's focus tracking says so. `settle` resyncs that tracking from `FocusManager.GetFocusedElement(XamlRoot)`, walked up to the nearest node, as `GotFocus` does, so a list, or a `NumberBox` whose focus is its text box's, keeps it.
- **Left out:** alternating row colours (AppKit's), separators (GTK's), single-click activation (GTK's and WinUI's), wrapping key navigation (Qt's), selection that doesn't follow focus (WinUI's), columns (a `Table`'s, §13.27).
- **Tweaks reach the list view,** not the scroll view the node stands for: the table on AppKit (its scroll view is `enclosingScrollView`), the `gtk::ListView`, the QML `ListView`, XAML's `ListView`. The example's: alternating rows on AppKit, separators on GTK (a line per row, which GTK's rows add to their padding), `keyNavigationWraps` on Qt, `SingleSelectionFollowsFocus` off on WinUI.
- **Where it has run:** every backend and headless: `tests/lists.rs` and the `contacts` suite (ten thousand rows, filtered by a search field, `examples/contacts`), checked by eye on each. The deselection before row changes has run on AppKit and headless (`tests/files.rs`, `tests/lists.rs`), and only type-checks on the others.

### 13.23 Tooltips

- **A prop, not a widget:** `.tooltip(text)` on any widget or container, sent as `Prop::Tooltip`; empty removes it. 2ksbox puts one on a status line cut off at one line, holding the whole text.
- **Shown as each platform shows them,** with its delay, placement and look: `NSView.toolTip`; `set_tooltip_text`; `ToolTipService` on WinUI; on Qt the attached `QQC2.ToolTip` (Breeze draws it), visible while hovered after `Qt.styleHints.mousePressAndHoldInterval`, as Kirigami apps do it. Qt Quick has no tooltip without that binding: controls use their own `hovered`, and labels, images and container hosts a `HoverHandler`.
- **On the view under the pointer:** a list's table or list view rather than the scroll view around it, and on AppKit a `NumberInput`'s field and stepper as well as their host.
- **Containers:** a WinUI host is a `Canvas` with no background, which never gets the pointer, so it gets a clear background while it has a tooltip (and takes the pointer over its empty areas meanwhile). A box's tooltip also shows over children without one of their own: on AppKit (tried by hand), on Qt (hover is passive), and GTK looks for a tooltip up the widget tree.
- **Read as the description** unless the app gave one: the core's tree does that, AppKit reads a tooltip as the view's help and GTK as its description on their own, Qt gets `Accessible.description`, WinUI `AutomationProperties.HelpText`.
- **Custom renders, drawn and native items on Kirigami** keep the tooltip on the node but don't show it: their QML is the app's.
- **Not tested: showing on hover,** since nothing here can rest a pointer on a native widget. The suite checks that each backend's native widgets and containers carry it, and the description; `examples/tooltip.rs` is for trying it by hand.
- **Where it has run:** every backend and headless, and tried by hand on each.

### 13.24 Context menus

- **A prop, not a widget, made of menus:** `.context_menu(entries)` on any widget or container takes what a `Menu` takes (items, separators, submenus, with reactive titles, states, `bind` and `radio`; §14.6), and sends it as `Prop::ContextMenu` whenever it changes; empty is none. The platform reports the item chosen as the node's `UiEvent::ContextMenuItem(id)`, so ids only need to be unique within the node's menu. Unlike menu bars they're not a service: each belongs to a native view, and the mirror check reads it back.
- **Shown as each platform shows them,** on its own gestures, with items built by the menu bar's code:
  - AppKit: `NSView.menu` (right-click, Control-click).
  - GTK: a `PopoverMenu` of a `gio::Menu` at the pointer (secondary click, long press on touch, Shift+F10 and the Menu key), its actions in a group inserted on the widget.
  - Kirigami: a `QQC2.Menu` of `Kirigami.Action`s, popped up at the pointer from a right-button `TapHandler` (on press, as KDE's menus open), a long press on touch, and the Menu key or Shift+F10 at the item's centre. Qt 6.9's `ContextMenu` attached type would do that, but the backend supports Qt 6.5.
  - WinUI: `ContextFlyout` of a `MenuFlyout`, which XAML opens on right-click, press and hold, Shift+F10 and the Menu key itself.
- **Children without a menu show their container's,** as a right-click does everywhere: AppKit passes the click up the responder chain, XAML's `ContextRequested` bubbles, GTK's gesture denies a click when the widget has no items, and Qt's handlers are off while an item has none. The test kit does the same: `choose_menu_item` acts on the nearest node up the tree with a menu.
- **AppKit's tables take right-clicks on their rows' labels:** over a label, the table's `hitTest:` answers the table (so that a click selects the row), which then shows its own menu, so a list row's menu showed only around its text. The table's `menuForEvent:` looks from the cell under the pointer (`rowAtPoint:`, `columnAtPoint:`, then the cell's own `hitTest:`) for the nearest view with a menu, and falls back to the table's own, the `List`'s. Found by hand: a `swiftc` probe sending right-clicks with `sendEvent:` hit the label instead, so a real click was traced.
- **Where a native menu is taken, the native one wins.** AppKit's pop-up button's menu is its options, and a right-click opens it, so a `Select` keeps its context menu on the node without showing it (GTK, WinUI and Qt show one). Text fields keep their Cut, Copy and Paste menu: GTK adds the app's items to it (`extra-menu`, as GNOME apps do), Qt keeps KDE's and doesn't show the app's, WinUI's `ContextFlyout` replaces XAML's while the app has items, and on AppKit the field editor shows its own while editing. A GTK spin button's + and − keep their right-click (jump to the ends), so a `NumberInput`'s menu is on its text there. Qt's custom renders, drawn and native items keep it on the node, as they do tooltips.
- **Shortcuts are shown, not bound:** a context menu's items show their shortcut, as menus do, but only the menu bar's shortcuts work from the keyboard. Qt would match an action's shortcut with its menu closed (ambiguous between rows), so its actions have one only while the menu is open. XAML runs a flyout item's accelerator while focus is inside its owner; that's left as XAML does it.
- **Check and radio items** are drawn and toggled as in menu bars (§14.6).
- **Qt's open menus sit in the window's overlay.** An item that changes a list's rows resets the view, and a row's host leaves the window between delegates; a popup whose parent changes window shows itself again in the new one, and KDE's menus are still visible during their exit fade, so Duplicate and Sort By stayed open. While it's open, the menu's parent is the window's `Overlay.overlay`, and it goes back to its item once closed. Right-clicking a KDE text field logs `Shortcut: Only binding to one of multiple key bindings` for each item: qqc2-desktop-style's own text field menu gives its actions `StandardKey`s, which its `MenuItem` binds with `sequence`. Every Kirigami app logs it, and we leave it.
- **Choosing without opening:** `A11yAction::ContextMenuItem(id)` chooses an item as a screen reader does once it has shown the menu, through the item's own path: AppKit's `performActionForItemAtIndex:`, GTK's `activate_action`, Qt's `trigger()`, WinUI's automation peer (Invoke, else Toggle, else what `Click` does). Disabled items and disabled controls refuse it: disabled controls show no menu (GTK, XAML and Qt don't deliver them input).
- **Updated in place when only enabled and checked states change** (`MenuBarData::same_structure`, the entries as one menu), so an open menu stays open; GTK refills the same model otherwise, WinUI and Qt rebuild. GTK's popover (or text widget) lets go of the model while it's refilled: refilled under it, a `PopoverMenu` adds the new submenus' pages to its stack before the old ones go, and warns about their duplicate names (a title that changes, e.g. Start/Stop, rebuilds). A one-item GTK popover has extra space under its item; a bare GTK 4.22 `PopoverMenu` has it too, so it's left as GTK draws it. XAML's radio group names are shared across the thread, so groups are named by their window or node as well as their first id: two windows' bars, or a bar and a context menu, could uncheck each other's items.
- **Not yet:** Finder outlines the row a right-click is on (a table's `clickedRow`), and Files and Dolphin select it; here neither happens.
- **Where it has run:** every backend and headless (`tests/context_menu.rs`), with right-clicks tried by hand in `examples/context_menu.rs` on each (on GTK also through Xwayland with `xdotool`), and checked by eye on each.

### 13.25 Dropping files

- **A prop of hosts, not a widget:** `Container::file_drop` and `Group::file_drop` take a `FileDrop` (extensions, or any file, and whether folders); `on_drop` gets the paths, and `on_drop_hover` says when files it takes are over it, for the app's own highlight: platforms show only that they'll copy. 2ksbox's disc library takes .iso and .cue files and folders.
- **The backend filters, not the core,** because the platform decides while the files are over the node whether the drop is welcome (the copy cursor), before any drop reaches the core. Every backend filters with the core's `FileDrop::accepted`, which asks the file system whether a path is a folder, and reports only what it keeps, in the drag's order.
- **Not reachable without a pointer:** no platform has an accessible drop, so apps should offer another way in (an open dialog, as the examples do).
- **Each platform's drop target:**
  - AppKit: the host view registers for file URLs and is the `NSDraggingDestination`, reading the URLs from the dragging pasteboard.
  - GTK: a `gtk::DropTarget` for `gdk::FileList` with `preload` on, since GTK hands a drop's files over only once they're read: until then it offers to copy, and once they're in, a drag with nothing the node takes is refused (`reject`). Remote URIs without a local path aren't taken.
  - Kirigami: a Qt Quick `DropArea` over the host, after its children (Qt Quick hands drags only to items that take drops). On `entered` the backend filters the URLs and the area accepts the copy only if something is kept, so a refused drag gets no `exited` or `dropped`. A drop ends the hover itself, since Qt sends no `exited` after one.
  - WinUI: the host's `Canvas` with `AllowDrop` and XAML's drag events, and a clear background so its empty areas are hit. XAML gives a drag's files only asynchronously (`GetStorageItemsAsync`), so entering starts reading them; until they're read the host takes the drag on its format, and from then on only if the filter keeps one of them. A drop before the read finishes reports when it does; leaving ends the drag, so a late read is dropped.
- **Tests drag through each backend's own handling** (`SyntheticInput::DragFiles`, `DragLeave` and `DropFiles`; the test kit's `drag_files`, `drag_leave` and `drop_files`), with real files and folders in a temporary folder (`tests/file_drop.rs`). What they skip is reading the paths from a real drag, which nothing here can start: `examples/file_drop.rs` is for trying it by hand from the file manager, and the icon example's library takes dropped discs too.
- **Where it has run:** AppKit and headless. Only type-checked on GTK, Kirigami and WinUI; to verify: reading a real drag's paths on every platform (AppKit's pasteboard included); GTK's preload during hover and `reject`; WinUI's read finishing during a drag from Explorer, and its canvas getting drags over empty areas; Kirigami's `drag.urls` and `keys` for drags from Dolphin.

### 13.26 GpuSurface

A surface the app presents to itself, from its own thread, as it would present to a window of its own. It was built for 2ksbox's player, the emulated machine's picture. With full screen, the minimum size and resizing (§14.2), it replaces what the player asked of winit (`set_fullscreen`, `set_cursor`, `set_min_inner_size`, `request_inner_size`, `Focused`, `DeviceEvent::MouseMotion`).

**The surface.**

- **`GpuSurface::new().on_ready(…).on_resize(…)`.** `on_ready` gets a `SurfaceHandle`, which implements `raw-window-handle`'s traits (so `wgpu::Instance::create_surface(handle.clone())` takes it) and is `Send + Sync`; `on_resize` gets its `SurfaceSize` (pixels and scale), which `handle.size()` also has for a render thread. The backend reports `UiEvent::SurfaceReady` once the native surface exists, then `SurfaceResized` whenever its pixel size or scale changes. `examples/gpu-surface` draws moving bands and a circle from a render thread with `Fifo`.
- **The handle keeps the native surface alive,** after the widget is gone too: a GPU surface made on it must never outlive it (Wayland and Vulkan treat that as an error). When the node goes, the surface stops showing and reporting, and is freed with the last handle, on the UI thread wherever that's dropped: AppKit releases the view on the main queue, WinUI posts `WM_CLOSE` to its child window, and Wayland objects may be destroyed from any thread.
- **No natural size:** it's as large as the layout makes it. It reads as an image (`Role::Image`) named by its label, and takes no focus unless it takes input.
- **The size follows the frame at once,** so the app never draws its next frame at the old size: GTK reports it when the area is allocated (`resize`), and its `settle` allocates surfaces at their new frame, as it does lists and header bars; Qt reports it when the item's width or height changes, which `SetFrame` sets right away; WinUI's `settle` lays out and places each surface itself, rather than waiting for XAML's next frame (`follows_its_frame`).

**Each platform's route.**

- **AppKit:** a layer-backed `NSView` whose backing layer (`makeBackingLayer`) is a `CAMetalLayer`, sized with the view by AppKit. Its contents stay at the top left while it resizes (`layerContentsPlacement`): AppKit's default for layer-backed views stretches the last frame to the new bounds, so every frame drawn at the old size showed stretched until one at the new size arrived. wgpu takes the view's layer as it is, and must be given the handle on the main thread (it reads the view there), so `on_ready` is the place. It's part of the view hierarchy, so views can draw over it.
- **GTK and Kirigami on Wayland:** a `wl_surface` of our own, a desync subsurface of the window's surface, over a widget (`gtk::DrawingArea`) or item that keeps the space, sized with `wp_viewport`, with an empty input region, on the toolkit's own Wayland connection (`mitsuami-linux`; libwayland-client is loaded at run time, as the toolkit loaded it). It's placed after each of the window's frames (GTK's frame clock's `after-paint`, Qt's `afterAnimating`), so it follows the widget wherever layout or scrolling moves it; a move takes effect with the window's next commit, so it asks for a frame. Without its subsurface role it isn't shown: hiding the widget, or destroying the node, takes the role away. A window's surface is new each time it's mapped, so the role is made again then. Qt's window surface comes from its platform native interface (`qpa/qplatformnativeinterface.h`, under Qt's versioned include directory).
  - Why this route: 2ksbox's spike (`spikes/player-gtk` on its `track/player-gtk-spike` branch) measured two on GTK. `GtkGraphicsOffload` fed dma-bufs waits for GTK's frame clock, about one refresh more than a winit window (47.6 against 32.8 ms from a frame's publish to its presentation at 60 Hz). A desync subsurface, placed over the widget, sized with `wp_viewport` and given an empty input region so GTK keeps the pointer, matched the winit window. Qt's own routes weren't taken: a `QQuickRhiItem` renders in the scene graph's frame, and a child `QWindow` is destroyed and made again with its window.
- **GTK and Kirigami on X11:** a child window of the window's, on an xcb connection of our own (`mitsuami-linux`, x11rb with libxcb loaded at run time), since xcb is thread-safe and the app presents from its own thread (the handle is `Xcb`). It has no background (what was there shows until the app presents) and an empty input shape, so the pointer goes to the toolkit's window. It's placed in the window's pixels ((widget point + GTK's surface transform) × scale; Qt's scene point × device pixel ratio, the window manager's frame being outside the client window), stacked above its siblings, and moved under the root window when its node goes, so the toolkit's window doesn't destroy it with itself. It's made with the toolkit window as its parent, so it has that window's visual (with a compositor, GTK's is ARGB: what the app presents with alpha shows through).
- **WinUI:** a child window (HWND) over a `Canvas` that keeps the space, placed before each of XAML's frames (`CompositionTarget.Rendering`); without input it answers `WM_NCHITTEST` with `HTTRANSPARENT`. That passes the pointer to windows under it in the same thread, but XAML's content island (`DesktopChildSiteBridge`) never gets it: XAML sees no pointer over the child window (found by injecting real mouse input over it, and seeing XAML's events come once it was hidden). When its node goes it's hidden and moved under `HWND_MESSAGE`, so the XAML window doesn't destroy it with itself. A `SwapChainPanel` would compose with XAML (wgpu has it for Direct3D 12 only); a child window works with any GPU API.
  - With Vulkan on NVIDIA (an RTX 3090, driver 32.0.16.1664), fast resizes sometimes leave the surface at 2 frames a second: each acquire waits about 500 ms on a fence in the driver, and reconfiguring doesn't recover it. That happened in about 1 in 6 runs of 1,500 scripted resizes, and never with Direct3D 12 (0 of 23), nor with Vulkan in a plain winit window, drawing to the window itself or to a child window like ours. The example uses Direct3D 12 on Windows.
- **A GLib main loop on another thread** (2ksbox runs QEMU's in its process) needs a GLib of its own: GTK, and Qt's GLib event dispatcher, run on GLib's global default `GMainContext`, so that library's loop would otherwise dispatch the toolkit's sources on its thread.
- **Where platforms differ:** on GTK, Kirigami and WinUI the surface sits above the window's own content, so nothing the toolkit draws can be over it (an overlay, or a menu in full screen, needs the platform's own layering); on AppKit it's a view like any other. A GTK 4.10 window's scale is a whole number, so a fractional display gets pixels at the next whole scale, as GTK draws its own. The size in pixels is rounded as each platform rounds it; the tests allow a pixel.
- **Popups over the surface.** What Qt Quick draws in the window's overlay goes under the surface: a `Select`'s list opened behind the example's surface on Kirigami, while GTK's popover, a popup surface of its own, showed above. Kirigami's select opens its list in a window of its own (§13.8), and it showed above the surface under Sway. Context menus, tooltips and the global drawer are still drawn in the window: context menus as windows were misplaced (the menu moves to the window's overlay to pop up, and showed in the window's corner), and drawers can't be windows.
- **The Linux tests' private displays have no surfaces.** GTK's tests run on Broadway and Qt's on its offscreen platform, neither Wayland nor X11, so there the backends make a `NoSurface` (`mitsuami-linux`): the app is told of it and its size, as headless tells it, with a handle that has no window or display handle, and a lock or grab ends at once. `TestApp::has_surface_handles` says where handles come (not headless, nor on the private displays); the tests that check them read it. With `MITSUAMI_SHOW_WINDOWS=1` the tests run on the session's display, where they check the real surfaces.

**Input** (`on_input`, `Prop::TakesInput`): keys and the pointer, as 2ksbox's player needs them for its machine.

- **Keys come by where they are on the keyboard** (`KeyCode`, the W3C `code` names, from macOS virtual key codes, Linux evdev codes and Windows scan codes, with the platform's code as `native`): down, up and the platform's repeats. **The pointer** in points, every button, scrolling in lines (a wheel) or points (a trackpad), positive towards the end. A click or Tab focuses it.
- **It gets every key the window doesn't take first** for its shortcuts, in each platform's order, Tab too (a game's view or a terminal takes Tab); Control+Tab leaves it on AppKit, GTK and Kirigami, as it leaves a text view. Input comes through the widget under the surface, the toolkit's own events, since the surface takes none (the pointer on WinUI aside):
  - AppKit: the view is a responder like a game's: `acceptsFirstResponder`, `keyDown:` after the menus' key equivalents, `flagsChanged:` for modifier keys (each side's device flag; Caps Lock, whose flag is the lock's, is reported down and up per press), a tracking area for moves and leaving. A local event monitor, while it's the first responder, takes every `keyUp` (AppKit sends none for a key let go while Command is held). A modifier key's `flagsChanged:` event must not be asked whether it repeats, which only key downs and ups answer: AppKit raised an exception, its run loop swallowed it, and no modifier key was ever reported, so Control+Option, which lets go of the example's capture, showed a frozen cursor and left the lock and grab on. Found by posting a click and the modifier keys to the app's own event queue (`NSApp.postEvent`, which needs no Accessibility permission and goes through local monitors as real input does); `takes_modifier_keys` sends the view a real flag-change event.
  - GTK: the area's own controllers (a key controller, motion, `EventControllerLegacy` for every button, scroll). mitsuami's window shortcuts are a bubble-phase `ShortcutController`, which GTK would run after the focused widget; so a key with Control, Alt or Super is left to the window when one of its shortcuts matches it exactly (`ShortcutTrigger::trigger`), as application accelerators in GTK's capture phase would win. Plain keys (Escape in a dialog too) go to the surface.
  - Kirigami: a C++ item filling the item that keeps the space, overriding Qt's key, mouse, hover and wheel handlers (`nativeScanCode` − 8 is the evdev code, `nativeVirtualKey` the keysym). Qt reports a held key as release and press pairs with `isAutoRepeat`: the releases are dropped. The backend's own Tab filter lets Tab through to a focused surface. Qt Quick focuses nothing in a new window, so the backend focuses the first control in the window's order, once, if nothing has focus (§15, Kirigami).
  - WinUI: keys through the `Canvas` itself (WinUI 3's `UIElement` has `IsTabStop` and `Focus`). The pointer over the surface never reaches XAML (above), so the child window reports it from its own messages (`WM_MOUSEMOVE`, the buttons', the wheels', `WM_MOUSELEAVE` through `TrackMouseEvent`), holds it while a button is down (`SetCapture`), sets the app's cursor (or the arrow) on its `WM_SETCURSOR`, and focuses the canvas on a press. The wheel goes to the focused window when Windows' "scroll inactive windows" is off: XAML then finds the canvas, which has a clear background for it. XAML runs accelerators on the focused element's path before its `KeyDown`, but those elsewhere (the menus') only if `KeyDown` goes unhandled, so keys with Control or Alt are reported and left unhandled, and a menu's shortcut still fires ("Keyboard accelerators", Input event priority, on Microsoft Learn).
- **Keeping keys on WinUI.** Keys reach the surface only while XAML's focus is on it, and three things left it elsewhere, for every window: XAML gives a `Page`'s first focusable element focus when it loads, but the window's content is a `Grid`, so a window opened with nothing focused; after another app's window took activation for a moment (NVIDIA's overlay), XAML's own restore left focus on its root `ScrollViewer`; and a press on the `TitleBar` control reached that `ScrollViewer`, which focuses itself. So once XAML has handled a window's activation (a dispatcher item queued from `Activated`), and when a window is first shown (it's activated when made, before its content and before the handler exists), the backend focuses the control that had focus, or the first in the core's Tab order, when XAML's focus isn't on a node; and the title bar marks presses handled and isn't a tab stop, as a Windows caption leaves focus alone.
  - Win32 focus matters too: keys go to the window that has it, which XAML keeps on its content island. A few seconds after the example starts presenting, with NVIDIA's overlay loaded into it (`nvspcap64.dll`), Win32 focus moves from the island to the XAML window itself, with no input, on Direct3D 12 and Vulkan alike, and before the child window took the pointer too; the text input example, which presents nothing, keeps it. Keys then reach nothing until the window is activated again (Alt+Tab twice), when XAML gives the island focus back. Found by watching the example's UI thread from outside (`GetGUIThreadInfo`). So each frame, a surface whose window has Win32 focus itself gives it to the island (`InputFocusController.TrySetFocus` for `XamlRoot.ContentIsland`), which gives it back to its focused element. XAML raises `LostFocus` on the canvas when the island loses Win32 focus, though the canvas stays its focused element, so that no longer ends the keyboard grab (it ended the example's capture, the lock with it, each time the overlay showed). A stand-in for the overlay (a topmost window that activates and closes) no longer loses focus, and a click on the status text leaves it on the surface. The maintainer tried the example with the real overlay: the surface has key focus from the start, keys keep coming when the overlay shows (the watcher's log showed Win32 focus staying on the island), and the capture survives it.
- **Remapped keys on Linux.** Under Sway with Caps Lock and Control swapped, the surface reported the keys where they are, not what the swap made them: the swap is the keymap's (`ctrl:swapcaps`), applied above the evdev code, where Windows's scan code map remaps below the code it reports. So on GTK and Kirigami, keys that type no character (modifiers, Caps Lock, Escape, arrows, function keys) go by their keysym (`KeyCode::from_keysym`): on GTK the key's unshifted keysym in the event's layout (`Display::map_keycode`), on Qt `nativeVirtualKey`, which has the shift level applied, so `Meta_L` (Shift+Alt on most layouts) is left out. Letters and digits keep their place, as a game's WASD needs, and `native` stays the evdev code. Both backends' native suites give the same results as before, and the maintainer tried the fix in the example on GTK and Kirigami under Sway.
- **Keys held are let go** when the surface loses focus or its window stops being the active one: they're reported released right then, since their releases go to another window, and the app would think them held (2ksbox's player lifted every key on winit's `Focused(false)`). AppKit when focus leaves, and on `NSWindowDidResignKeyNotification` while it's the first responder, since the first responder stays when the window stops being the key one; GTK on focus `leave` and on the window's `is-active` going false; Kirigami on losing active focus, which Qt Quick takes away when the window deactivates, and on `activeChanged`; WinUI on `LostFocus` and on `Window.Activated` (deactivated), since XAML's focus doesn't move then. A release never seen going down is dropped. On GTK and Kirigami a key is let go as the code its press was reported as, which for a remapped key is its keysym's. `tests/gpu_surface.rs` holds a key down on AppKit with a real key event and posts the notification.
- **Synthesized input** (tests): a click focuses the surface and reports the primary button, a key goes down and up, a scroll is reported in points. AppKit sends real `NSEvent`s through the view's own methods and Kirigami real Qt events for clicks and keys; GTK, WinUI and every scroll report what the platform would. On GTK a synthesized key asks `is_focus` (focus within its window), since `has_focus` is false while a test's window isn't the active one; Kirigami asks its window's focus item.

**Pointer lock** (`pointer_lock`): the cursor hidden and held, its moves reported as `SurfaceInput::Motion` in points, accelerated as the cursor would be. Only in the active window; the platform ending it, or the window stopping being the active one, reports `PointerLockEnded`, which sets the app's signal back (2ksbox locks again on the next click). None of the toolkits has one:

- AppKit: what games do: `CGAssociateMouseAndMouseCursorPosition(false)` and `NSCursor.hide`, the cursor warped to the surface's middle; the mouse's moves still come as events, with their deltas. It ends when the window resigns key.
- Wayland (GTK and Kirigami): `zwp_locked_pointer_v1` (one-shot) on the window's surface with `zwp_relative_pointer_v1`, on a seat and pointer of our own (the compositor sends a client's pointer events to each of its pointers). Their events are dispatched on a thread of our own (libwayland lets several threads read a connection, as GTK and Qt do) and sent to the UI thread: GTK with `MainContext::invoke`, Kirigami through a queue drained when Qt's loop wakes. The cursor is hidden with the toolkit's own cursor (`none`, `Qt::BlankCursor`).
- X11 (GTK and Kirigami): a pointer grab on the child window, confined to it, with a blank cursor; each move is reported as its distance from the middle, then the cursor is warped back. A click that asked for the lock holds the toolkit's implicit grab until its button is let go, so the grab is retried for two seconds. The grab takes the buttons and the wheel from the toolkit, so the lock's thread reports those too.
- WinUI: `ShowCursor(FALSE)`, `ClipCursor` to the canvas, and `SetCursorPos` back to its middle after each move. It ends when the window is deactivated. Hiding the canvas while locked reports `PointerLockEnded` there; the other backends release it silently (Kirigami locks again when shown).

**Raw motion** (`SurfaceInput::RawMotion`): while locked, each move comes twice: `Motion` in points, accelerated as the cursor would be, then `RawMotion` in the device's counts, before the host's acceleration. 2ksbox's player gives a PS/2 guest the raw one (winit's `DeviceEvent::MouseMotion`): the guest accelerates it itself, and would accelerate the host's acceleration. Counts aren't points, so they're a variant of their own.

- AppKit: GameController's `GCMouse` (`mouseMovedHandler`, macOS 11), whose deltas Apple documents as "not affected by mouse sensitivity settings", with no permission (IOHID would need Input Monitoring). A small Swift program checked it by hand against `NSEvent`'s deltas while locked: slow moves gave `GCMouse` 3 to 4 times as much, fast ones half as much, so the ratio followed the speed, as acceleration does. Up is positive there, so y is turned round. Its handlers run on the main queue; a mouse has one handler, so the last surface to lock has it. Mice connecting while locked get it too.
- Wayland: `zwp_relative_pointer_v1`'s `dx_unaccel` and `dy_unaccel`, in the same event as the accelerated move.
- X11: XInput 2's `XI_RawMotion`, which only the root window gets and only for a client that said it speaks XInput 2: the lock's thread asks for 2.0 and selects it on the root, on our own connection, while the lock holds. Without XInput 2 there's no `RawMotion`; the lock and its `Motion` still work.
- WinUI: Raw Input, the mouse registered to the surface's child window while the lock holds (no `RIDEV_INPUTSINK`: the lock only holds in the foreground window), `lLastX` and `lLastY` from `WM_INPUT`. Absolute moves (remote desktop, tablets, some virtual machines' mice) have no counts, so they give only `Motion`. A process has one Raw Input registration per device kind: ours replaces any other while locked, and removing it drops one the app made itself.

**Keyboard grab** (`keyboard_grab`): focuses the surface, then gives it every key, the window's shortcuts and as many of the system's as the platform lets an app take. It ends (`KeyboardGrabEnded`) when the surface or its window loses focus, or the platform lets go.

- AppKit: the key monitor takes every key before AppKit dispatches it (menus' key equivalents, Command-`), and the app's presentation options turn off process switching (Command-Tab) and hiding; AppKit allows the first only with the Dock hidden or hiding itself, so the Dock hides while the keyboard is grabbed. Mission Control, Spotlight and the Spaces shortcuts stay the system's: only an event tap, which needs an Accessibility permission, or private API takes those.
- GTK: `gdk::Toplevel::inhibit_system_shortcuts` (the Wayland shortcuts inhibitor, a keyboard grab on X11), and a capture-phase key controller on the window that gives the surface every key; `shortcuts-inhibited` going false ends it.
- Kirigami: on Wayland `zwp_keyboard_shortcuts_inhibitor_v1` of our own (Qt has no API; its `inactive` ends it), on X11 `QWindow::setKeyboardGrabEnabled`; the item accepts `ShortcutOverride`, so the window's shortcuts don't fire.
- WinUI: a `WH_KEYBOARD_LL` hook, while the window is the foreground one and the canvas has focus, swallows every key and reports it itself: Alt+Tab, the Windows key, Alt+F4 and the app's accelerators. Control+Alt+Delete stays the system's.

**The cursor over it** (`GpuSurface::cursor`, `Prop::Cursor`): the platform's own, none, or the app's image at its scale with its hotspot in points, e.g. the one a machine gives its pointer. It's set on the widget under the surface, whose pointer it is. `Hidden` is a blank cursor, not the platform's "hide the cursor", which is global and counted (`NSCursor.hide`). The lock's hidden cursor wins while it holds. No platform gives an image back, so the backends keep the prop.

- AppKit: a cursor rect over the view (`resetCursorRects`), shown while the window is the key one; none is a clear 1 × 1 image.
- GTK: the drawing area's `set_cursor`: `none` hides it, as GTK's video widget does, and an image is a texture. GTK 4.10 shows a texture one pixel per point, so an image at another scale is resampled to its size in points (GTK 4.16's `Cursor::from_callback` would keep the detail).
- Kirigami: the input item's `setCursor`: `Qt::BlankCursor`, or a `QCursor` from a pixmap with its device pixel ratio.
- WinUI: `WM_SETCURSOR`, in subclasses of the XAML window's windows: over the surface they set the app's cursor instead of letting XAML set the arrow. An `InputCursor` can't be made from pixels, and `ProtectedCursor` is only a subclass's. The image is redrawn at the display's scale (nearest pixel) as a 32-bit `HCURSOR` (`CreateIconIndirect`); none is a null cursor.

**Not yet:** text input (typed characters, input methods) on the surface, and captures of what the app presented (AppKit's `cacheDisplayInRect:` doesn't read a Metal layer; the story shows where the surface sits).

**Where it has run.**

- **The suite** (`tests/gpu_surface.rs`) on AppKit and headless (headless gives a handle without window handles, and sizes at its scale factor; the AppKit run checks the view's layer is a `CAMetalLayer`), and on WinUI. On GTK and Kirigami, on their private displays and under Sway on Wayland and on X11 (Xwayland: `GDK_BACKEND=x11`, `QT_QPA_PLATFORM=xcb`); Kirigami on X11 takes about 15 seconds, and once `takes_keys_while_focused` got other keys there, which didn't happen again in five runs. Input runs on AppKit and headless too (clicks, keys, Tab into the surface, scrolls; raw motion headless, one count to a point; the lock and grab ending, headless only, since a test run's window may not be the key one; natively the test checks the props say what the platform did).
- **The example, by hand.**
  - AppKit: it presents from its render thread, and follows resizes without stretching (it draws in points, with a circle that would show as an oval). The capture works (Control+Option letting go was fixed after, and is covered by `takes_modifier_keys`); locked, the circle follows the raw moves, one count to a point, which don't feel accelerated (moved by `Motion`, it still did, as it should). From a terminal whose windows macOS says are hidden, wgpu skips every drawable, so it can't be checked from there.
  - GTK and Kirigami under Sway: presenting and resizing, and remapped keys and the first focus.
  - Raw motion is only type-checked on Wayland, X11 and WinUI.
  - WinUI: resized from a script, with its child window above XAML's content island at the canvas's place and size; then driven by injected input (`mouse_event`, `keybd_event` with scan codes) and tried by hand: the circle follows the pointer, a click captures, locked moves move it, keys come with and without the grab, Control+Alt lets go, and the wheel scrolls. Injected keys without a scan code come as `Unidentified`, as the scan code is what's reported.
- **Unverified until they run with real input** (the tests synthesize it, and a test window may not be the active one):
  - X11: the child window's position with client-side decorations and `QT_SCALE_FACTOR`, and a Qt window's XID when first shown; that an X server delivers raw events to our root selection while our own client holds the grab (XInput 2.0 may not), and whether libinput's Xorg driver already scales them.
  - Wayland: that relative motion and the lock's `unlocked` arrive on our own thread, and GTK's `invoke` and Qt's wake are quick enough for motion.
  - GTK: the controller order (a capture-phase controller added to the window after GtkWindow's own, the legacy controller getting every button, `shortcuts-inhibited` notifying on both display servers).
  - Qt: `ShortcutOverride` reaching the focused item before the shortcut map, an accepted Tab stopping Qt Quick's focus chain, `nativeScanCode` being the XKB code on xcb and Wayland, `setKeyboardGrabEnabled` taking the window manager's shortcuts; Qt's `frameMargins` with its own decorations (Sway decorates Qt's windows itself).
  - WinUI: a handled Tab stopping XAML's focus navigation, `KeyDown` coming for Alt and the Windows keys, `ShowCursor(FALSE)` hiding the cursor (a screen capture doesn't show it), that `WM_INPUT` reaches a child window registered as the target, and that `SetCursorPos`'s warps give no raw input.
  - The cursor and the key releases are only type-checked on GTK, Kirigami and WinUI: texture cursors' size on HiDPI Wayland and X11; the hotspot's scale on Qt, and whether an item's cursor applies over the input item when it takes no input; on WinUI, that the child window's `WM_SETCURSOR` shows the app's cursor over a surface that takes input, and over one that doesn't, that the island's input window gets `WM_SETCURSOR` in a subclass before XAML, that XAML doesn't set the cursor again on pointer moves, that a straight-alpha 32-bit cursor shows right, and that `EnumChildWindows` finds the island's window when the surface is attached. On AppKit the cursor's image, none and the arrow were checked through `native_state` only.

### 13.27 Table

A table is a `List` whose rows are cells under column headers: the platform's table control, which virtualises, draws the header and the selection, and lets the user resize and sort by the columns. Its data, selection, activation, keys, scrolling, style and handle are a list's (the core shares the machinery); what's new is columns, cells and sorting. Its rows drag files out as a list's do (§13.31).

- **Columns** are `TableColumn`s: a title, the view each row shows in its cell, the width it starts at, `expand` (it takes the room left, as a file's name does) and a `sort_key`. They're data (`Prop::Columns`), like a sidebar's items: the platform builds its own columns and header from them, and the user resizes them as the platform lets them.
- **Cells, not rows, are mounted.** For each row the platform shows, the core mounts a host per column (a `Container` with `Prop::Cell`), in one scope for the row, and the table's native children are those hosts in row, then column order. Every platform's table asks for a view per cell (AppKit's `viewForTableColumn:row:`, a `ColumnViewColumn`'s factory, a `TableView` delegate), so a cell's host goes where the platform puts that cell. The platform reports the width each column gives its cells (`ColumnWidths`), and the core lays each cell out at it, as high as its content; the platform makes a row as high as its highest cell and at least its own row height, and centres the others in it, as native tables centre a cell's content.
- **Sorting is the app's; the header is the platform's.** `Table::sort` binds a `Signal<Sort<S>>` (a column's sort key and an order) both ways: the header shows it (`Prop::Sort`), and pressing a sortable header sorts as the platform does (the same column the other way round, another ascending) and reports it (`Changed(Sort)`), which sets the signal. The app sorts its data by it; the table never reorders rows itself. Sort keys are `Any` so columns don't carry the key's type, and a key of another type than the signal's panics when the table is built.
- **Accessibility:** a `Table` role with its headers (`ColumnHeader`s, the sorted one's value its order), then `Row`s named by their cells' text and selected as their row is, holding `Cell`s. A row is selected or activated through any of its cells, so tests find a row by one cell's text (`get_by_role(Role::Cell, "notes.txt").select()`), and a header is pressed by its title (`get_by_role(Role::ColumnHeader, "Size").click()`, the `PressHeader` action).
- **In `view!`**, `each` and `key` come first, as a `List`'s, and columns are attributes: `<Table each=files key=|f: &File| f.id column=name column=size sort=sort/>` (or `columns=vec![…]`). Columns aren't tags: a column's cell closure needs the item's type, which a child tag can't know before the table's.
- **AppKit:**
  - The list's `NSTableView`, with a column per `ColumnData` (named by its index) and its own header, style and intercell spacing, in an `NSScrollView` that also scrolls sideways. The uniform autoresizing style shares the room left among the columns that autoresize, which are the ones that expand; every column is user-resizable. Column widths are reported as a column's width less the intercell spacing, when the table is sized and from `tableViewColumnDidResize:`.
  - A sortable column has a sort descriptor prototype, keyed by its index, so a header click sorts as AppKit does and `tableView:sortDescriptorsDidChange:` (a data source method) reports it. The core's sort is set as the table's sort descriptors, muted, and the header's indicator image (`NSAscendingSortIndicator`, `NSDescendingSortIndicator`) is set with it. `PressHeader` does what a click does: the first sort descriptor reversed if it's that column's, else its prototype.
  - Each cell is a `HostView` that centres its host in its height, now and whenever the row's height changes (`resizeSubviewsWithOldSize:`). A row's height is its highest cell's, at least the table's `rowHeight`.
  - The header insets the clip view: its bounds start the header's height above the rows. Scroll offsets are reported, and taken by `ScrollTo`, from where the rows start (the bounds' origin plus the clip view's `contentInsets`), and a cell's rect is its place in the scroll view plus that offset, so the core's scrolled content lines up however the rows are inset.
- **WinUI:** a header row of column buttons above the list view, as File Explorer builds one. With multiple selection, `ListViewItem`s show a check box before their content: the backend measures where a realised row's cell starts in its container, and moves the header past the check box and takes it from the room the columns share (a list's rows are as much narrower). The header has nothing above the check boxes (File Explorer has a check box there that selects every row).
- **Left out:** hiding and reordering columns, a column's alignment of its header's title, and sort orders a column starts with (Finder sorts dates newest first). Each platform has them; they're the next options.
- **Where it has run:** AppKit, WinUI and headless (`tests/tables.rs`, the `table` story, `examples/table.rs`). On WinUI, `cells_are_as_wide_as_their_columns` and `only_the_rows_near_the_viewport_are_mounted` fail.

### 13.28 What no widget has

From the survey of the widgets' native options, each missing on one platform, so each a tweak or the app's to build: window minimize and maximize buttons and a maximum size (GTK 4), middle truncation (WinUI), a link button (AppKit), a text field's length limit (AppKit, which needs a formatter), a password's reveal button (AppKit), a search delay (WinUI), an editable combo box (GTK's is deprecated), slider tick marks (Qt Quick), decimals in a spin box (Qt's holds an `int`), an image's cover fit (AppKit's image view), tabs on other sides (WinUI's selector bar), a table (WinUI 3, built as its apps build one, §13.27), a collapsible group (AppKit, Qt Quick), a toolbar's overflow (GTK's header bar).
### 13.29 Keys

- **A prop of containers and lists, not a widget:** `on_key(shortcut, handler)` on a `Container` (`Column`, `Row`, `Grid`), `Group`, `List` or `Table` takes a key (a `Shortcut`, §14.6: a key and modifiers) while it, or a control inside it, has keyboard focus: Finder's Space on its list (Quick Look), Delete on a folder's view. The core sends a node's keys as `Prop::Keys`, and the backend reports one pressed as the node's `UiEvent::Key(shortcut)`. A window's commands stay menu items with shortcuts (§14.6).
- **The focused control first, then up, the platform's way.** A key goes to the focused control, and only one it doesn't use goes up to the nearest node that takes it. Which keys a control keeps is the platform's, never overridden:
  - AppKit: the responder chain. A view passes a key it doesn't use to its next responder, its superview, and a host view's `keyDown:` takes its node's keys. A table passes keys on to a responder of ours put right after it (`ListKeys`, ahead of its clip view), so a list's own keys only see what the table left. Measured: `NSTableView` keeps ↑ and ↓ with any modifiers (so Finder's ⌘↑ and ⌘↓ are menu key equivalents), Home, End and letters (typing to select); it passes on the page keys, ← and →, Delete, Backspace, Space, and ⌘ with a letter. A key is read with `charactersIgnoringModifiers` if it types nothing and `charactersByApplyingModifiers:` with none otherwise, so Shift+[ is `[` with Shift: the layout gives keys that type nothing control codes there.
  - GTK: a `gtk::ShortcutController` (local scope, bubble phase) on the node's widget, a list's or table's on the scrolled window around its view. GTK's list and column views keep the arrows, Home, End and the page keys (alone or with Ctrl, Shift or both), Space (alone or with Ctrl or Shift: it selects the focused row) and Ctrl+A and its kin; a row keeps Return, and a column view's cells ← and → (with Ctrl too).
  - Kirigami: an event filter from the shim on the node's item (a list's or table's on the scroll view around its view), since Qt Quick sends a key the focused item doesn't accept up its parent items. QML's `ListView` keeps ↑ and ↓ with any modifiers, except past its ends; our list QML adds Home, End, Return (without Ctrl, Alt or Meta), Ctrl+A in multiple selection and the Menu key; an empty `TableView` stops every key. Qt names a key with Shift by its shifted character, so a real Shift+[ doesn't match `[` with Shift there.
  - WinUI: a bubbling `KeyDown` handler on the node's element, marking what it takes handled, with the modifiers read from `GetKeyState`. Not keyboard accelerators: XAML invokes an element's accelerators before its `KeyDown`, so a list's would take its arrows first. XAML's `ListView` keeps the arrows, Home, End and the page keys (with Shift or Ctrl too), Space (it selects or toggles the focused row) and Ctrl+A in multiple selection.
  - Tab and Shift+Tab move focus: AppKit, Qt and WinUI do before a node sees them, GTK's window only after (its binding is the window's own), so don't take them.
- **Menus and a node's keys:** for a key that's both a menu shortcut and a node's, AppKit's key equivalents and Qt's `Shortcut`s come before the focused view (the menu wins), while GTK's window shortcuts and XAML's accelerators only get what the focused control and nodes left (the node wins). Each platform's order is kept, so give a command one or the other.
- **Nothing shows them,** as a menu shows its shortcuts, so the command needs a menu item or a button too. Pick the keys the platform's own apps use (`platform!`): the files example's Space shows the preview on macOS only, since GTK's and XAML's lists keep Space.
- **Tests press keys on the focused control** (`press(Key::F(2))`, `press(Shortcut::primary(Key::Backspace))`; `SyntheticInput::Key` and `SyntheticInput::Shortcut`). AppKit sends a real key event to the window, the control made first responder; Qt sends a real key press with its modifiers. GTK 4 can't inject keys, so its backend runs the shortcut controllers from the focused widget up, in GTK's order (capture, target, bubble), and activates the first match, GTK's own bindings included; key-press controllers, input methods and mnemonics aren't run. XAML can't be sent keys, so WinUI's backend has a table of the keys each control keeps, then walks up the nodes. Headless keeps a list's arrows, Home, End and Return, and a text field's typing. A key nothing takes is `Unsupported`, where a platform beeps. Keys with modifiers aren't simulated in text fields.
- **Not yet:** typing to select a list's row, which AppKit's table keeps letters for, and GTK's, Qt's and XAML's lists don't do (Nautilus and Dolphin build it); keys on other widgets, and on windows, whose commands are the menu bar's.
- **Where it has run:** AppKit and headless (`tests/keys.rs`, and the files example's `the_preview_key_shows_and_hides_the_preview`); `examples/keys.rs` is for trying them by hand. Only type-checked on GTK, Kirigami (the shim only syntax-checked) and WinUI. To verify there: which keys each platform's list keeps (from their sources and docs, not tried; WinUI's table of the keys its controls keep is from how XAML's controls behave as recalled), that XAML's `KeyDown` brings the keys the focused control left, and GTK's simulated press against a real keyboard.

### 13.30 FileIcon

- **A file's or folder's own icon, as the platform's file manager shows it** (`FileIcon::new(path)`): an app's own icon, a folder's (a custom one too), a document's by its type. The `files` example drew it with a custom widget of its own (native on AppKit and GTK, a built-in icon by kind elsewhere); every app listing files would write the same, and only the platform knows a file's icon, so it's a widget. Not a source for `Icon`: WinUI shows glyphs and bitmaps in different elements (`FontIcon`, `Image`), and a file's icon needs the file's path and a thumbnail flag that named icons don't have.
- **A square, always,** as big as the platform's small icons unless `icon_size` says otherwise: 16 points everywhere (Finder's list, GTK's normal icon size, Kirigami's `iconSizes.small`, Explorer's details view). Its size never depends on the file, so it's measured without reading it, and a thumbnail arriving later fits in the same square (proportionally, as file managers draw them).
- **`thumbnail(true)`** shows a preview of what's in the file (a picture's, a PDF's first page) in place of its icon, where the platform makes one; the icon until then, and where it makes none. It's passed through, and platforms without thumbnails ignore it:
  - AppKit: `QLThumbnailGenerator` with `iconMode` on, as Finder draws thumbnails in place of icons, at the window's (or main screen's) scale. Its handler runs on another thread and hops to the main queue; the view waits for it by address, with a ticket so a late thumbnail for a file no longer shown is dropped. `settle` runs the run loop until pending thumbnails are shown, as WinUI's waits for images being decoded, so captures have them.
  - GTK: the thumbnail the desktop has cached (`thumbnail::path` when `thumbnail::is-valid`), as GTK's own file chooser shows. GTK makes none itself: Files does, through GNOME Desktop's thumbnailers, which aren't GTK's.
  - Kirigami: none. Dolphin's thumbnails are KIO's (`KIO::PreviewJob`), which a Qt Quick app doesn't have; Qt has no thumbnailer. The flag is kept for reading back.
  - WinUI: the shell's image with `SIIGBF_RESIZETOFIT` (a thumbnail, or its icon where there's none) instead of `SIIGBF_ICONONLY`.
- **Each platform's icon:**
  - AppKit: `NSWorkspace.iconForFile`, sized to the square, in an `NSImageView` that scales proportionally. A type IconServices hasn't rendered before draws a dashed placeholder at first and the real icon a moment later, by itself, as in Finder; a story's first capture on a machine can show the placeholder.
  - GTK: GIO's `standard::icon` for the file (its content type's, from the icon theme, read from its name and first bytes), in a `gtk::Image` at `pixel_size`, as Files shows it.
  - Kirigami: the icon of its MIME type (`QMimeDatabase::mimeTypeForFile`, by name and content), with its generic icon as `fallback`, in a `Kirigami.Icon`: what Dolphin shows without a preview.
  - WinUI: `IShellItemImageFactory::GetImage` at the square's pixels (the window's scale), read on a thread of its own (the shell may read the file), then shown as a `WriteableBitmap` in an `Image`, with a ticket so only the latest load shows. It's premultiplied BGRA, as XAML takes it; a thumbnail with no alpha at all is made opaque. `settle` waits for the load.
- **Read when it's set,** as an image file is: a file that changes under the same path isn't read again (until its size or thumbnail changes, which reads it again), and a path that doesn't exist shows what the platform gives for one: a generic document on AppKit, the icon for its name's type on Kirigami, nothing on GTK and WinUI (there's no file to ask about).
- **An image to assistive technology** (`Role::Image`), named by its label; without one it's decorative, as it usually sits beside the file's name, which says it already. It takes no focus. No platform gives the path back, so backends keep it on the node for the mirror check (Kirigami in a property of its own).
- **Tests** (`tests/file_icon.rs`): a square of the size given, larger or smaller than the default, following a new file and size, decorative without a label, taking no focus, something drawn in its square (a capture), and on AppKit and WinUI a picture's thumbnail showing its blue and red halves. The story (`file_icons`) uses the crate's own files, so every machine has them.
- **Where it has run:** AppKit and headless, checked by eye (the story's capture: a folder, a `.toml` document, a PNG, and the PNG's thumbnail). Only type-checked on GTK, Kirigami (the shim compiled against Homebrew's Qt) and WinUI. To verify: GTK's icons and cached thumbnails; Kirigami's MIME icons in Breeze (`inode-directory` for folders); on WinUI, the shell's bitmap's orientation and premultiplied alpha, the thread's COM apartment, and that `GetImage` makes thumbnails for pictures and PDFs.

### 13.31 Dragging files out

- **A list's or table's rows, not any node:** `List::drag_files(|item| Option<PathBuf>)` (and `Table`'s) makes each row carry its item's file when it's dragged out of the app, to the file manager, the desktop, Mail or another app, as a file manager's rows are. That's where apps list files, and each platform's list control has its own row drag, which drags the selection when a selected row is dragged; a drag from any host would skip that. A host of its own (a proxy icon) isn't done yet.
- **The files are a list prop, not the rows':** rows are virtualised, and a dragged selection can hold rows with no node mounted, so the core sends every row's file as `Prop::RowFiles` (only if the app gave `drag_files`), before `Prop::Rows`, whenever the data changes. A row whose item has no file doesn't drag; in a selection, only the rows with one are carried.
- **A copy, always.** Apps that aren't the file manager offer their files to copy (Mail's attachments, an editor's documents), so a destination on the same disk that would move them (Finder, Files) copies, and the app's files stay where its listing says. The drag says nothing back: nothing changed.
- **Each platform's own drag:**
  - AppKit: `NSTableView`'s, through its data source's `tableView:pasteboardWriterForRow:`, a file URL per row, which Finder and other apps take as the file. The table asks for each row it drags: the selected rows when a selected one is dragged. Its operation mask is Copy, in and out of the app (by default a table offers nothing outside it).
  - GTK: a `gtk::DragSource` on each row's cell (a table's cells each), as Nautilus's rows have: `prepare` gives a `gdk::FileList` of the row's file, or the selection's when the row is selected, as a copy; nothing without a file. The row itself is the drag icon (a `gtk::WidgetPaintable`).
  - Kirigami: a `DragHandler` in each row's delegate (a table's cells) drives the attached `Drag` with `Drag.Automatic`, a platform drag, with a `text/uri-list` of `file://` URLs QML makes from the row's file or the selection's, offering `Qt.CopyAction`. Mouse only, so a touch drag still scrolls, and the view can't take a drag over once it's begun.
  - WinUI: the `ListView`'s own item drag (`CanDragItems`, on once there are files), whose `DragItemsStarting` has the rows XAML drags (the selection when a selected row is dragged). The files go as storage items, which Explorer takes; loading them is asynchronous (`StorageFile::GetFileFromPathAsync`, and `StorageFolder::GetFolderFromPathAsync` for folders), so the data package gets a provider (`SetDataProvider`), which loads them on a thread of its own when the drop target asks, with the request's deferral, as the Files app does. The requested operation is Copy; a drag with no files is cancelled.
- **Tests ask the backend** (`dragged_files`, `TestHooks::dragged_files`): nothing here can start a drag, so each backend runs its drag source's own code up to where the platform takes the files: AppKit calls the table's data source through its selector for each dragged row and reads the URLs; GTK the function its `prepare` uses; WinUI the one its `DragItemsStarting` turns rows into files with, choosing the rows (the selection, or the row) itself, since only XAML knows which it drags; Kirigami reads the URLs QML made for the row's delegate. `tests/file_drag.rs` checks a row, a selection, rows without files, data that changes, and a table; the `files` example's rows drag too, and dropping them back on their own folder leaves them there (`files` suite).
- **Not reachable without a pointer,** as dropping isn't: apps should offer another way out (the example's Copy Paths).
- **Where it has run:** AppKit and headless, through the tests; a real drag to Finder is untried (`examples/file_drag.rs` is for trying it by hand). Only type-checked on GTK, Kirigami (the QML never loaded) and WinUI. To verify there: a real drag to the file manager on each (GTK's `FileList` through the portal in a sandbox; Qt's `Drag.Automatic` from a `DragHandler` in a `ListView` delegate, and whether the view's own mouse flicking gets in the way; XAML's deferred storage items and the provider's thread).

---
## 14. Windows and the app shell

### 14.1 Windows

- **`App::window(title, size, content)`** makes the windows open at startup. **`Window` is a view**, declared anywhere in the tree next to the state that opens it, and shown while its `open` value is true, like `Show`: that's how 2ksbox's launcher drives its windows (a model's `open` flag). Its content is built when it opens and disposed when it closes, so each opening starts fresh; it closes with the scope that declared it, so a window declared in another window's content closes with that one. In the tree it's a fragment that takes no room; the native window is top-level. **`App::open(|| window)`** opens a `Window` at startup, made by a closure run in the app scope once the app starts, so its title and props can use stores (the Files example's title is its store's folder) and the signals it makes live as long as the app, for what only a `Window` has (full screen, a minimum size, a title that changes): 2ksbox's player window is one. The app ends when its last window closes, whichever kind.
- **The close button asks, and the app decides.** `bind(flag)` closes a window by clearing the flag; `on_close_request` lets the app keep it open (to ask about unsaved changes, as `examples/windows.rs` does); without either the close button does nothing, since the flag is the app's. Platforms deliver the request their own way (`windowShouldClose:`, `close-request`, `closing`, `AppWindow.Closing`), always vetoed, and the core destroys the window when the app closes it.
- **In `view!`**, the title is an attribute and the children are the content: `<Window title="Machine" bind=editing>…</Window>`. `Window::new` takes the title; the tag starts without one, and the title is a type parameter, `()` until set, as `#[component]` does for required props. A window's content is one view; several children go in a `Column`.
- **`on_open` runs at each opening, before the content is built,** to start a form from what's saved: `<Window title="Advanced" bind=open @open=move || draft.set(…)>`. The builder API can do it in `content`'s closure, but `view!`'s children are views, so it needs a handler.
- **A window's size and modality are read at each opening** (`.size(value)`, `.modality(value)`), since each opening creates the window anew. The windows example needs it: a dialog follows its content's height, while a plain window has a fixed size, because macOS can open a plain window as a tab of another (always in full screen, or as the user's "Prefer tabs" setting says), and a tab takes its window's frame: following its content's height shrank the whole showcase window (`its_size_applies_at_the_next_opening`).
- **Modal windows are the app's choice of two** (`.modal(Modality::…)`): `Window` blocks the window it's declared in, `Application` the whole app. The maintainer chose to offer both rather than pick for macOS, where they look different:
  - AppKit: `Window` is a sheet on its window (the macOS idiom for a dialog about a window), `Application` a separate window centred on its owner, run in `NSApp.runModalForWindow` (what Qt does for 2ksbox's dialogs today). A sheet has no close button, so its content must offer a way out: a `ButtonRole::Cancel` button, which Escape presses. The modal loop can't start inside a tick (it's nested), so AppKit schedules it on the main run loop after the tick that shows the window; the UI keeps ticking inside it, since the app's observer runs in the modal panel mode too. Destroying the window ends its sheet (`endSheet:`) or its loop (`abortModal`, the call for code outside event handling, with an empty event to wake the loop).
  - GTK: `modal` with `transient_for`, which blocks the whole app either way (GTK has no window-modal; GNOME attaches modal dialogs to their parent).
  - Kirigami: Qt's `WindowModal` and `ApplicationModal` with a `transientParent`. Qt ignores a modality set on a window already shown, and `Kirigami.ApplicationWindow` shows itself when created (`visible: true`), so the backend's windows start hidden and show after their first layout. The tests read the modality back, which Qt keeps even when it ignores it, so this was found by hand and checked against `QGuiApplication::modalWindow()`.
  - WinUI: an owned window with `OverlappedPresenter.IsModal`, which disables the owner, plus the app's other windows disabled for `Application`, as Win32 apps do. Windows are activated hidden at creation, for a live XAML tree, so it's applied when the window is shown: `GWLP_HWNDPARENT` to the owner, centred on it, not minimizable or maximizable, then `IsModal`; for `Application` the app's other windows are disabled with `EnableWindow`, leaving alone those another modal window already disabled, and enabled again on destroy, before the owner comes back to the front.
- **Only modal windows belong to a window:** the one they're declared in, found through `CurrentWindow`, which each window's scope provides (`App::window`, `Window`, the test kit's mount). A window declared inside a modal window belongs to that one. One declared outside every window (rare) has no owner, so a `Window` modality becomes `Application`: Qt's window-modal blocks nothing without one. Plain windows belong to none, so they aren't kept above another window, as owned windows are on Qt, GTK and WinUI. Every `Window` builds in a scope of its own, even when always open, so its `CurrentWindow` doesn't leak to what's declared after it.
- **Nested dialogs are windows declared in a window's content,** which is how the tests and `examples/windows.rs` write a cancellable dialog: "Advanced…" in a machine window opens a `Modality::Window` dialog on it, with a draft started in `on_open` that OK applies to the machine window's form and Cancel, Escape or the close button drop.
- **Escape asks a modal window to close,** through the close request, as every platform's dialogs close on Escape (AppKit's Cancel button, `QDialog`, `GtkDialog`, Win32's `IDCANCEL`); plain windows ignore it, as they do everywhere. Each goes through the platform's own path, so a focused control that uses Escape (an open pop-up, completion) gets it first: on AppKit a key equivalent (a `ButtonRole::Cancel` button takes it), then `cancelOperation:` up the responder chain to the window's delegate, which a field editor passes on; on GTK a bubble-phase `ShortcutController` calling `close()`, as `GtkDialog` has; on Qt a `Shortcut` on `StandardKey.Cancel`, as 2ksbox's launcher has; on WinUI a `KeyboardAccelerator`, which fires only when the focused control didn't handle the key. Synthesized Escape takes the real path on AppKit and Qt (a key event); GTK 4 can't inject key events and WinUI's backend drives controls, so theirs run the window's shortcut or accelerator directly, and a focused control can't take Escape first in their tests.
- **The focused window is the one that counts.** Alerts and file dialogs without a window go on the focused one, and Escape closes only the focused modal window. Qt's `active` is true for the focused window's transient parents, and so for their other transient children: a modal window's owner and its other dialogs all read as active. Taking the first active window put alerts on the owner, which the modal window blocks, and Qt matches window shortcuts by `active`, so two modal windows' Escape shortcuts were ambiguous and neither fired. The shim sets each window's `mitsuamiFocused` from `QGuiApplication::focusWindow()`, which alerts and the Escape shortcut use; synthesized keys go through the shortcuts first (`qt_sendShortcutOverrideEvent`, as QTest does), after activating their window. WinUI takes the active window (`GetActiveWindow`), since every XAML window keeps a focused control while inactive; without an active window (the app in the background), a window that isn't disabled, focused if one is. The test kit answers alerts itself, so this was checked by hand in the windows example.
- **`TestHooks::close_window`** clicks the close button through the platform, so the request goes the way a user's does: `performClose:`, `gtk::Window::close`, `QQuickWindow::close()` (which sends nothing to a window never shown; test windows are shown by then), and on WinUI a posted `WM_CLOSE`, where the close button and Alt+F4 end up, since `Window.Close()` closes without raising `Closing`. `TestApp::close_window` and `TestApp::window_titled` use it; locators search every window.
- **Known gap: GTK's window-modal windows aren't tied to their window.** They're separate modal windows, `transient_for` their owner, which block the whole app. Only GNOME's compositor attaches them, so they move with their owner there; on Sway they move on their own, as any GTK app's do. GNOME apps (Text Editor) now use libadwaita's dialogs, drawn in the parent window, which move with it everywhere and block only that window, like a macOS sheet. `Modality::Window` as an `adw::Dialog` and alerts as `adw::AlertDialog` would close the gap, at the cost of a dialog clipped to its owner (a bottom sheet in a narrow window), title, size and close requests wired through the dialog, and libadwaita 1.5 over the 1.4 floor. Left for later.
- **Where it has run:** every backend and headless, with sheets, modal loops and Escape tried by hand in `examples/windows.rs` on each, since tests never show windows: they check the modal prop the backends carry, not the sheet or the loop. The size read at each opening on AppKit and headless; only type-checked on GTK, Kirigami and WinUI, though the change is in the widget, not the backends.

### 14.2 Window size and state

Full screen, maximizing, a minimum size and resizing each come as a `Window` prop the app sets and, for what the user changes too, an event the core absorbs into it, so the app's signal always says what the window shows.

- **Full screen is each platform's own** (`Window::full_screen(signal)`, `Prop::FullScreen`): AppKit's `toggleFullScreen:`, a Space of its own with its animation (what winit's borderless full screen does on macOS too); `gtk::Window::fullscreen`; Qt's `WindowFullScreen` state; WinUI's full-screen presenter. The user changes it too, the platform's way (the title bar's green button, Escape and the menu bar on macOS, the window manager's key), which the backend reports as `FullScreenChanged`. A refusal is reported the same way.
  - Every platform reports its own changes too, and later: AppKit's `windowWillEnterFullScreen:` comes inside `toggleFullScreen:` (a small Swift program showed it, and that a toggle during the animation is ignored), GTK's `notify::fullscreened` when the compositor configures the window, Qt's `windowStateChanged` inside `setWindowStates`. So each backend keeps what the app asked for and reports only what differs from it; muting while applying commands wouldn't do.
  - A window takes full screen once it's shown on AppKit and WinUI: the Swift program showed a hidden `NSWindow` goes into full screen unseen. Tests never show windows, so natively the mirror check compares what the window will show. AppKit applies a request made during a transition when it ends. A sheet can't have full screen (AppKit refuses), nor a dialog on WinUI (an owned, modal presenter): the app hears `false`.
  - Full screen keeps the window's size for when it leaves. AppKit resizes a window in full screen when told to, so every backend ignores `SetWindowSize` while it's in full screen; GTK's default size would apply when it leaves, and WinUI's kept `OverlappedPresenter` gives its settings back. The Fluent title bar is collapsed meanwhile on WinUI: the full-screen presenter has no caption.
- **`Window::maximized(signal)`:** the platform's maximize (AppKit's zoom, GTK `maximize`, Qt's `WindowMaximized` state, WinUI's presenter `Maximize` and `Restore`), which the user changes too (`MaximizedChanged`). AppKit reports a zoom from `windowDidResize:`, comparing `isZoomed` with what it last knew, so a resize that undoes a zoom counts. GTK's `notify::maximized` and Qt's window states come later for the app's own, so they're compared with what the app asked, as full screen's are; WinUI reads the presenter's state when `AppWindow.Changed` says the size changed, and maximizes only once the window is shown (`Maximize` would show it) and out of full screen. Headless fills a work area (1280 × 740), and keeps sizes the app sets meanwhile for when it's restored, as GTK does.
- **`Window::resizable(value)`:** AppKit's `resizable` style, GTK `resizable` (which `HeightFollowsContent` turns off too: either keeps a GTK window from resizing), WinUI's presenter `IsResizable`, and on Qt a minimum and a maximum at the window's size, moved with every size the app gives it, as KWin holds KDE's fixed-size dialogs (Qt has no flag for it on Linux). The app still sizes it; `TestHooks::resize_window` doesn't.
- **A minimum content size** (`Window::min_size`, `Prop::MinSize`): AppKit's `contentMinSize`, a size request on GTK's content host (the header bar's above it), Qt's `minimumWidth` and `minimumHeight` with Kirigami's toolbar added, WinUI's `PreferredMinimumWidth` and `PreferredMinimumHeight`, which are the whole window's in pixels (the title bar, menu bar, toolbar and frame added). A window smaller when it's set grows to it: GTK does that itself, the others' backends do it, since 2ksbox's player had to ask for it (winit's minimum alone doesn't grow a window on every platform). Qt and WinUI apply it again when the chrome above the content changes.
  - On WinUI, Windows grows the window to a new minimum itself as it's set, before XAML lays the root out again, so the resize that follows uses the client insets (the resize border inside the client area) measured before: measured after, off a root at the old width, they came out as their 8 px cap, and the window was 8 px too wide (`grows_to_its_minimum_size`).
  - **It goes no larger than the screen:** capped at the content of a window filling its screen's visible area, and applied again when the window moves to another screen, since a machine's mode can be larger than a laptop's screen (the player capped it at `current_monitor`'s size), and AppKit would make a window as large as the minimum asks (a small Swift program gave a 5000 × 5000 window, in full screen too). `native_state` reports the app's minimum while the platform holds the capped one. AppKit caps at its screen's `visibleFrame` and again on `windowDidChangeScreen:`; GTK at the window's monitor less the header bar (GTK 4 has no work area on Wayland, so panels aren't taken off), again on `enter-monitor`; Kirigami at the screen's `availableGeometry` less the window's frame and the toolbar, again on `screenChanged`; WinUI at the monitor's work area (`GetMonitorInfoW`'s `rcWork`) less the frame and the chrome, again after `AppWindow.Changed`'s `DidPositionChange` (not yet on a scaling change on the same display). Headless caps at its 1280 × 800 screen.
- **The app resizes a window** with `Ui::set_window_size(window, size)`, finding it through `CurrentWindow`. The platform may refuse, and gives no less than the minimum, so the core lays the content out at the size the platform reports (`WindowResized`) rather than the one asked for. AppKit's `setContentSize:` goes below `contentMinSize`, so its backend clamps, and its test hook does too, as a user's drag can't go below it. GTK sets its content's size at once only before the window is first mapped. The core's size is the one it asked for, and the backend reports what the window gets once it has it: GTK allocates it at the next frame, so its `settle` waits for it (at most two seconds, as for a user's resize, about a second on Broadway), and so does Kirigami's until Qt has laid it out (`the_app_resizes_it`).
- **A window's height can follow its content** (`WindowSize::FollowHeight`, `FollowHeightUntilResized`, §4), as 2ksbox's Clone dialog does (its `height`, `minimumHeight` and `maximumHeight` bound to the content's): a note or a progress bar comes and goes, and the window grows and shrinks with it. `FollowHeightUntilResized` stops on a user's resize, as WPF's `SizeToContent` does.
  - No platform does all of this on its own (GTK 4 non-resizable windows take their content's size, WinUI 3 has no `SizeToContent`, AppKit and Qt apps resize by hand), so the core does it the same way everywhere: whenever the content or the width changed, it lays the window out at max-content height, sends a new height through the ordinary `SetWindowSize`, then lays the content out at the height the window has, so a minimum taller than the content leaves room below it. A window in full screen doesn't follow; it catches up when it leaves. `Ui::set_window_size` gives a `FollowHeight` window only a width, and ends following for the others.
  - Whose height changed is the core's guess, since only AppKit tells a user's resize apart: a reported height that's one the core asked for and the platform hasn't reported yet (GTK and Qt apply them later, so several can be under way), grown to the minimum, is the core's; any other is the user's. A window too tall for its screen isn't capped: an app whose content can grow that far gives it a scroll view.
  - The user can't change a `FollowHeight` window's height (`Prop::HeightFollowsContent`, which the core sets): a minimum and a maximum at the height it has, moved with it, on AppKit (`contentMinSize` and `contentMaxSize`, so the resize cursors only go sideways), Qt (`minimumHeight` and `maximumHeight`) and WinUI (`PreferredMinimumHeight` and `PreferredMaximumHeight`, the outer size in pixels as for the minimum). GTK 4 can't hold one side of a window, so there it isn't resizable at all (`set_resizable(false)`), as GNOME's content-sized dialogs are; its default size still sets its size (GTK's `toplevel_compute_size` reads it for windows that aren't resizable, and doesn't remember the user's). The platform's minimum then holds the locked height, so `native_state` reports the app's minimum from what the backend kept. AppKit's `setContentSize:` keeps a window's top edge where it is (checked with a small Swift program), so it grows downwards, as the others do.
- **Where it has run:** full screen, the minimum and resizing on AppKit and headless (`tests/windows.rs`: headless simulates the user's change; AppKit's full screen isn't shown in tests). GTK and Kirigami ran them natively on their private displays (Broadway, Qt's offscreen platform), and WinUI natively, before the screen cap and following the content's height were added: those two, maximizing and `resizable` ran on AppKit and headless only. `examples/gpu-surface` has a full-screen checkbox, a minimum size and `App::open`, and the windows example's machine window has "Maximized" and "Resizable" switches and a dialog that follows its content, for trying by hand. The test kit can't click a title bar's button, so the user maximizing a window is simulated headless only. Unverified until they run: whether Broadway does full screen at all (a request it ignores stays pending, and the mirror check wouldn't see it), whether a compositor that refuses full screen is ever heard on GTK (it sends nothing), whether Broadway before GTK 4.16 grows a mapped window for a new minimum without presenting it again, whether Qt's offscreen platform gives the window its old size back, and on WinUI that the minimum follows a DPI change (nothing applies it again yet), that `AppWindow.Changed` fires for our own `SetPresenter`, and that the kept presenter gives the window its size and position back.

### 14.3 Toolbar

- **What every platform shares:** a bar across the top of the window that shows its title (the unified `NSToolbar`, GTK's header bar, Kirigami's page toolbar, WinUI's `CommandBar`), with arbitrary items at its trailing end (`NSToolbarItem.view`, `pack_end`, a `Kirigami.Action`'s `displayComponent`, an `AppBarElementContainer`). Leading items and the title's place differ too much (Kirigami has no leading items; GTK centres the title), so only trailing items are built. The title stays the window's title, shown as each platform shows it.
- **`Toolbar` is a view, declared anywhere in a window's content,** like SwiftUI's `.toolbar`: it finds its window through `CurrentWindow` and puts its items there; they go with the scope that declared it, so `Show` around it takes them away. So it works for `App::window` and `Window` alike, with no new window API.
- **Each child is one item,** a `ToolbarItem` host: a native child of the window, after its content (the core sorts them last, so the content's indexes don't move). The core lays each out on its own at its natural size (`layout_toolbar`), outside the window's layout box; the platform places it and spaces the items, and the core reads where from `native_state`, as for list rows. So an item's frame is in the content's coordinates, above it (y < 0), and `visible_rect` clips what's in an item by the item, not by the content.
- **An empty item is hidden:** a `Show` that shows nothing leaves an item with an empty frame, which the backends hide (AppKit takes it out of the toolbar: `NSToolbarItem.hidden` is macOS 15's). 2ksbox's download progress shows this way only while a download runs.
- **Items that don't fit are the platform's too:** AppKit and WinUI move them to an overflow menu; the backend then reports the item at `Rect::ZERO`, and the core takes it as hidden. GTK's header bar makes the window wider instead, and Kirigami keeps them (`KeepVisible`).
- **The content keeps its size.** `SetWindowSize` is still the content area; the bar is added to the window. An item taller than the bar grows the window.
- **Known gap: Tab doesn't reach the toolbar.** Toolbar controls aren't in the core's Tab order, which is the content's. That's right for AppKit, whose toolbar items are outside the key view loop. On GTK, Qt and WinUI, whose own Tab chains reach the bar (the titlebar's widgets, QQC2 buttons, a `CommandBar` as one Tab stop), the backends' Tab handling walks only the core's order. 2ksbox's toolbar has no controls. Letting Tab leave the content for the platform's own chain at its ends would close the gap.
- **AppKit:** the toolbar is made with the first item, with a flexible space ahead of the items; each item's view is its host, sized by Auto Layout constraints, as toolbars size views. A window never shown (as in tests) doesn't make its toolbar's views until asked, so the backend lays out windows with toolbars after each batch and in `settle`. Tests turn off the toolbar's animation, whose frames they'd otherwise read on the way. On macOS 26 each item gets a glass capsule drawn tight around its view, and the system's own views pad themselves; ours don't, so the backend insets each host 10 pt from its capsule's sides, as far as the system's image items have their image (chosen by the maintainer from variants side by side; 7 pt top and bottom as well looked the same). Before macOS 26 there's no capsule and no inset. Adjacent capsules still come close enough that the tests only check the items' order.
- **GTK:** items go in the window's own header bar, packed at its end in order (packed again from the last one, since `pack_end` fills inwards), with GNOME's main-menu button still last; the header bar spaces them. Each host is `valign` centre: the header bar stretches its children to its height, and a host places its content from the top, so a lone `Text` sat at the top of the bar where a `GtkLabel` would be centred. An empty item is `set_visible(false)`. On a display nobody watches, frames stall, so the header bar wouldn't place an item shown again until the next one; `settle` allocates header bars that need it where they are, as it does lists.
- **Kirigami:** items are page actions: a `Kirigami.Action` with `displayHint` `KeepVisible` whose `displayComponent` holds the host, which is how KDE apps put search fields in the page toolbar. The toolbar shows them after the title with its own spacing; an empty item's action is hidden. When the content host's height changes without the window's (an item taller than the bar), the backend measures the toolbar again and resizes the window.
- **WinUI:** a `CommandBar` in a row of its own under the title and menu bars, collapsed while no item shows, with each host in an `AppBarElementContainer` among its primary commands; its defaults (overflow, trailing commands) are kept. XAML measures only what's in a live tree, and a collapsed bar keeps its items out of it, so a button in an item measured nothing, got an empty frame and stayed hidden (text needs no template, so it showed): each new item is laid out once with the bar shown, then collapsed until its first frame. A container is the bar's height and puts its content at the top, so it centres it (`VerticalContentAlignment`), as the bar centres its own buttons. The bar's trailing "More" button spaces itself from the window's edge, and with no secondary commands it doesn't show, which left the items against the edge: the bar has a trailing margin of 16, as far as the title bar insets the title from the leading edge (`TitleBarLeftPaddingWidth` 2 + `TitleBarLeftHeaderPaddingWidth` 14), a margin since the bar's padding only reaches its content area, and the bar has no background. The window grows by the bar, as with the menu bar. The windows' `TitleBar` control would put items beside the title (`RightHeader`), as on AppKit and GTK; `CommandBar` was chosen as WinUI's toolbar control, and `TitleBar`'s areas sit in the drag region.
  - XAML lays a window's content out at a new size only at its next frame (a new window's at the size it opened at, wider than the one set), so `settle` waits until each window's content is at the size last reported, or the items would be read at the old edge. And `resize_client` aims with the title bar and resize border as XAML last laid them out, which at the new size are a pixel off (the content 599.33 high for 600): once XAML has laid the content out at the new size, the backend corrects the window by the miss, if it's a pixel or two.
- **Where it has run:** every backend and headless, checked by eye.

### 14.4 Sidebar

- **What every platform shares:** a list down the window's leading side that picks what the window shows, as the system's own settings apps have: items with an icon and a title, in sections with an optional heading, one of them chosen, and the window's content beside it. How it collapses in a narrow window differs too much to share, so it's each platform's: AppKit's split view collapses the sidebar item as it does (dragging its divider away), libadwaita's split view becomes a stack of the two pages below 400sp (a choice shows the content, whose header bar has a back button), Kirigami's page row shows one page at a time, and WinUI's `NavigationView` in `Auto` shows only icons, then a menu button.
- **`Sidebar` is a view declared anywhere in a window's content,** like `Toolbar`: it finds its window through `CurrentWindow`, and goes with the scope that declared it. `Sidebar::new(selection)` takes a signal of the app's own type; each `SidebarItem::new(title, value)` gives it a value, and choosing an item sets it. A value no item has chooses none. Titles are reactive; icons are names in the platform's own set (§13.19). Items outside a `SidebarSection` next to each other are a section without a heading.
- **One node, its items as data.** The node (`WidgetKind::Sidebar`) is a native child of the window after its toolbar items, with `Prop::Sections` and `Prop::SelectedIndex` (the index across sections), and reports `Changed(Index)`. Items aren't nodes: every platform builds them from data, as menus are, and a native list needs no layout from the core. The a11y tree still shows them, as list items under headings; they stand for the sidebar node, and the test kit selects one by its title (`A11yAction::SetValue`), as a `Select`'s option is chosen.
- **The window's content is what's beside it,** so the core's window is the content: its size is the content's, and the window is larger by the sidebar, as with the toolbar. The sidebar's frame is the platform's, read back like a toolbar item's, beside the content (at negative x); a collapsed one is empty and hidden. It's first in the Tab order.
- **Toolbar items go between the content and the sidebar,** so a toolbar declared after the sidebar (on one of its pages, as the showcase's Toolbar page is) is inserted before it (`a_toolbar_can_come_after_it`).
- **The content's page is titled after the item chosen** on GTK and Kirigami, whose panes have header bars of their own (as GNOME and KDE settings do); the sidebar's page takes the window's title. AppKit and WinUI leave the window's title as it is.
- **`Sidebar::shown(signal)`,** which the user changes too (`SidebarShownChanged`): AppKit collapses its split view item and watches its `collapsed` (KVO), which the toolbar's toggle, the divider and a narrow window change; WinUI opens and closes the `NavigationView`'s pane (`PaneOpened`, `PaneClosed`); Qt takes the sidebar's page out of the page row, and puts it back ahead of the content's. libadwaita's `NavigationSplitView` always shows its sidebar while it isn't collapsed, so on GTK hidden only means something collapsed: the content's page shown (`show-content`), which the back button and a choice change too; wide, it's kept for when the split collapses. The tests don't look at a hidden sidebar's frame on GTK for that reason.
- **AppKit:** the window's content view becomes an `NSSplitViewController`'s (`contentViewController`), with the full-size content view the sidebar needs to reach under the title bar, as on macOS 11 and later; the host is in the content item, under the title bar and toolbar (its safe area). The window gets a toolbar with the sidebar's tracking separator first, so the title and toolbar items are over the content. The table is in the source-list style at AppKit's own row size (the user's sidebar icon size), with a fixed slot for icons so titles line up. Its width is AppKit's default (a fraction of the window's, within limits: 140 at 800 wide), and macOS 26 floats it, inset 8 pt from the window's edges. A toolbar's identifier is unique per toolbar: a window that got a new one while the old one wasn't freed yet (a sidebar shown again) had AppKit keep the two in sync, and assert. Offscreen captures can't draw the glass the sidebar sits in (nor, then, its rows), so window captures show the content.
- **GTK:** `adw::NavigationSplitView` in an `adw::BreakpointBin` (360 × 294, GNOME's smallest window) with the collapsing breakpoint; the window's header bar moves to the content page, and the window's title bar gives way to the pages' (a hidden one, as `AdwWindow` has). The list is a `gtk::ListBox` in GTK's `navigation-sidebar` style with headings for titled sections and lines between untitled ones, as GNOME Settings had before libadwaita 1.9's `AdwSidebar`, which needs a newer floor than Ubuntu 24.04's 1.5. The window's extra width follows libadwaita's sidebar width (a quarter of the window, 180 to 280, taken as points). Every page has a title before it's realized, which libadwaita checks (a shown window realizes the split as it's added): the content's is the item's or the window's, and an untitled page takes the app's name, which its header bar doesn't show. With libadwaita, the appearance goes to `AdwStyleManager:color-scheme`, not GTK's `gtk-application-prefer-dark-theme`, which it warns about.
- **Kirigami:** a `Kirigami.ScrollablePage` ahead of the content's page in the window's page row, as System Settings has its categories, at the row's default column width; `ItemDelegate`s, with `ListSectionHeader`s for titled sections, and a header's line alone between untitled ones (not before the first). `PageRow.insertPage` pops the pages from its position on first, which took the content's page away, so the sidebar is pushed after it and moved ahead (`movePage`). The page is made apart from the window, where Kirigami's `applicationWindow()` isn't defined: the window gives it its page row. A list view shows every section delegate it makes, so a header is hidden inside one. The user's choice (a click, the arrow keys) is a signal of the page's; the app's isn't reported. Qt's offscreen screen is 800 × 600 by default, too narrow for a sidebar beside a 500 pt minimum, so Kirigami's tests give it 1920 × 1080 (the platform's `configfile`).
- **WinUI:** a `NavigationView` takes the content host's row, with the host as its content, no Settings item and no back button. The window's extra width is the pane's as `Auto` shows it at that width (open from 1008, icons only from 641). Icons only needs every item to have an icon, as Fluent's guidance has it (an item without one showed its title cut down to the strip): while one hasn't, the compact threshold is the expanded one, so `Auto` goes from open to the menu button, and the extra width follows the view's thresholds. A closed pane in a wide window would show as that strip too (the app's `shown(false)`, or the pane's own button): without icons for all, it's hidden whole (`IsPaneVisible`), and the menu button goes to the window's `TitleBar`, as Task Manager has it, and shows the pane again, open; the view's own would sit over the content's corner. Open in a wide window (Expanded), the pane has no menu button, as in Windows' Settings: the user can't close it there, and it closes as the window narrows. The button stays in icons-only mode, which it opens, and for a pane the app closed in a wide window.
  - Which buttons show is decided once the view is done with a new mode, from the dispatcher: `DisplayModeChanged` fires before `Auto` opens the pane, which it does only while the pane is visible, so hiding it then kept a wide window's pane hidden (the view also passes through Compact, its template's state, before it has a width). The view doesn't report the app's closing, so `shown` places the buttons itself. Unloaded, the pane reads closed, so an app's `shown(false)` then was no change and `Auto` opened it in a wide window: it's opened first, so that closing counts as the app's (`Auto` keeps a pane the app closed).
  - A navigation view takes focus on its selected item, and its selected item is compared with the items by COM identity (the `IUnknown` pointers).
- **The showcase** (`examples/showcase`) is the examples in one window, a sidebar item each: each page is its example's `page()` (or named view), the example file included as a module, so the two can't drift apart. It's a crate of its own, outside the workspace, as `examples/gpu-surface` is, since it shows that example too (so wgpu isn't built with the workspace's tests). Leaving a page drops it and its state, as closing its window would: the GPU surface page's render thread stops (a flag its scope's cleanup sets), and its full-screen checkbox is the showcase window's. Releases build it alone, for each platform (`release.yml`); its `kde` feature, with `--no-default-features`, builds the KDE one without GTK. The sidebar's own toggles are the platform's, so the showcase has none.
- **Where it has run:** every backend and headless (`tests/sidebar.rs`, the `sidebar` story; `examples/sidebar.rs`, whose button shows and hides the sidebar, for trying by hand). `shown` and toolbar items after the sidebar on AppKit and headless; only type-checked on GTK and Kirigami. WinUI's pane and buttons run on Windows: the showcase opened wide, went to the title bar's button narrow, and closed and opened from both buttons; the native sidebar tests pass but for sizes 2 px short at 150% scaling (600 → 598, frames at y −1.33), which `the_minimum_size_is_the_contents` now meets too, as its pane opens and collapses. The test kit can't drag a divider, so the user hiding a sidebar is simulated headless only.

### 14.5 Tabs

- **A view switcher, not document tabs:** a fixed set of pages, one shown, with a tab for each, as in a settings window. Every platform has one: `NSTabView`, libadwaita's inline view switcher, Kirigami's `NavigationTabBar`, and WinUI's `SelectorBar` (its `TabView` is for documents, closable and reorderable). It's `Tabs` rather than GNOME's "view switcher", which names only the switcher in a header bar, not the pages.
- **`Tabs::new(selection)` with `Tab::new(title, value)`s,** as `Sidebar` takes its items: the signal holds the value of the page shown, and picking a tab sets it. Every platform's tab view always shows a page, so a value no tab has shows the first and leaves the signal as it is (as a `Select` shows its first option). Titles are reactive. A `Tab` is its page's column: `.padding`, `.gap` and the other style setters go on it. In `view!`: `<Tabs selection=page>` with `<Tab title="General" value=Page::General>…</Tab>`s.
- **Every page stays mounted,** as every platform's tab view keeps its pages: what's typed in one, and its scroll position, are still there when it's picked again. The pages the platform doesn't show are out of the a11y tree and the Tab order, and have an empty frame; which page shows is the core's to say (`SelectedIndex`), so an empty page shown still counts as shown.
- **One node, its pages as native children.** `WidgetKind::Tabs` has `Prop::TabTitles` and `Prop::SelectedIndex`, and reports `Changed(Index)`; its children are page hosts (`Container`s), as a list's are row hosts. Tabs aren't nodes: in the a11y tree they're `Role::Tab`s of the `Role::TabGroup`, before the page shown, and the test kit picks one by its title (`A11yAction::SetValue`), as a sidebar's item.
- **The core sizes the pages, the platform places them.** The tab view is a one-cell grid holding every page, so it's as big as its biggest page, as a notebook or `StackLayout` is, and doesn't change size from page to page; grown, its pages fill it. Its padding is where the platform's page area is (`PlatformMetrics::tab_insets`: the strip on top, the border elsewhere), measured from a real tab view by each backend; its own padding is ignored. Its minimum is its strip, which the backend measures once it exists: at the start of layout, which resolves styles again if a strip changed. It and `Group` are the only containers the core measures. The pages are where the platform put them, as rows and toolbar items are, at the core's size.
- **`TabsStyle` is GNOME's and KDE's.** Each has navigation tabs (the default) and a tab bar, named for what they are rather than for a toolkit's widget. The other platforms have one tab view; they keep the prop and report it. The tabs example switches between the two on GNOME and KDE.
- **`Tab::icon(name)`,** a name in the platform's own set as `Icon`'s: a view stack page's `icon-name` on GTK, whose inline switcher is told to show labels and icons (its `display-mode`, set through its enum's nick, since the switcher is looked up at run time); the tab actions' and buttons' `icon.name` on Qt; a `FontIcon` as a `SelectorBarItem`'s `Icon`. AppKit's `NSTabView` draws no icons: on macOS 26 its tabs are a segmented control that ignores `NSTabViewItem.image` (found with a small `swiftc` app listing the tab view's subviews), which is for a tab view controller's toolbar style, so AppKit keeps them on the node, as GTK's notebook does.
- **AppKit:** an `NSTabView` with its tabs on top. Each host is in a plain flipped view of its own as its item's view, which the tab view sizes to its page area, at the view's top-left. Its insets are measured on one (27 on top and 3 elsewhere from its alignment rect on macOS 26: the frame's page area, less the alignment insets). Its minimum width is AppKit's `minimumSize`, at which it truncates its tabs' titles, as AppKit does. The delegate's `tabView:didSelectTabViewItem:` is called for the backend's own selections too: they're muted.
- **GTK:** libadwaita's `InlineViewSwitcher`, centred above an `adw::ViewStack` (12 pixels apart, GNOME's spacing between groups), as GNOME apps switch panes inside a window; the pages are the stack's, titled. The switcher is new in libadwaita 1.7, above the 1.4 floor (§11), so the backend builds against 1.4 and looks up the switcher's type when it runs (`dlsym` of its type getter); with `TabsStyle::TabBar`, or an older libadwaita, it's a `gtk::Notebook`, a `gtk::Label` for each tab.
  - The view is in a box that stands for the node: a new style makes the other view in it and moves the page hosts across, with their titles and the page shown. A focused text field that moves to the new view is focused again: GTK keeps the focus on it, but until it's focused again the window never finished drawing (`a_new_style_keeps_the_focus` captures the window after the switch).
  - The two measure differently, so each tab view returns its own insets from `Backend::tab_insets(id)`, a throwaway view's page bounds for each kind (an even border all round if they can't be read); the metrics' are the default's.
  - Either view stretches its pages over their area, so each host is in a layout host of its own at its top-left. A view stack only appends, so a page inserted before others takes them out and puts them back after it, keeping the page shown. `visible-child` (the notebook's `switch-page`) is muted while commands apply. A view that switched pages is allocated again at `settle`, so the new page's place can be read.
  - CI's GTK runner is Ubuntu 24.04, whose libadwaita is 1.5, so CI tests the notebook, and the switcher only runs on newer systems.
- **Kirigami:** Kirigami's `NavigationTabBar` over a plain `Item` holding the pages, the shown one visible (a `StackLayout` would size the pages itself). It's the bar Kirigami apps switch views with, a row of large tabs across the top, closer to libadwaita's view switcher, WinUI's `SelectorBar` and AppKit's tabs than Qt Quick's `TabBar`, which is there with `TabsStyle::TabBar`: both bars are in the item, the one shown is its `mitsuamiStrip`, and the pages go below it.
  - Only a tab's action's `triggered` (the navigation bar's) or `TabButton.clicked` (the tab bar's) reports the user's choice. Setting the navigation bar's `currentIndex` triggers the tab's action, so the backend checks the tab's button instead.
  - Neither bar has arrow keys, and Tab reaches only the selected tab, so the backend adds Left and Right (mirrored for right-to-left) to both, as `QTabBar` and the other platforms' tabs have; the navigation bar's buttons are its own `NavigationTabButton`s, made by the backend's delegate for that.
  - Its strip is sized when it's measured: the navigation bar's buttons size themselves when Qt polishes them, so the backend polishes the whole strip first. Its natural width is its buttons, all as wide as the widest, as it lays them out; its own implicit width makes room for five, however many there are. Its height isn't the metrics' `tab_insets` (a probe tab bar's), so the backend returns each tab view's from `Backend::tab_insets(id)`, as groups return theirs.
  - KDE's desktop `TabBar` is as high as its first tab, and read it while a repeater had yet to make one (a QML `TypeError` whenever a tab view was made), so the first tab is always in the bar, hidden without titles, and the repeater makes the others.
- **WinUI:** a `Canvas` holding a `SelectorBar` (an item for each title) and the page hosts under it, the shown one `Visible` and the others `Collapsed`. The bar's height isn't known until one is in a window: `tab_insets` starts from 48 (worked out from its template), and the first bar that loads is measured and sends `MetricsChanged` if it differs. `SelectionChanged` is guarded by the page shown, and a null selection (the selected item removed) is ignored.
- **Left out:** tabs on other sides (WinUI's selector bar has only the top).
- **Where it has run:** every backend and headless (`tests/tabs.rs`, the `tabs` story, a `view!` case in `tests/macros.rs`; `examples/tabs.rs` for trying by hand). Both GTK views pass natively on Linux (the switcher on Arch Linux's libadwaita 1.9, the notebook through `TabsStyle::TabBar`). Tab icons on AppKit and headless (the `tabs_with_icons` story); only type-checked on GTK, Kirigami and WinUI.

### 14.6 Menus

- **Menus are a service, not widgets.** Every platform builds its menus from data and calls back with the item chosen, and on macOS the bar isn't in any window. So a `MenuBar` is data (`MenuBarData`) the core sends again whenever it changes, not nodes in the tree: menus have no frames, no Tab order and nothing for the mirror check to read back. Context menus (§13.24) and menu buttons (§13.4) take the same items, as props.
- **What's there:** submenus; check items (`MenuItem::bind` toggles a signal, `checked` only shows it) and radio items (`radio((signal, value))`, a pair so `view!` can write `radio=(zoom, Zoom::Large)`); reactive titles, visibility (`visible`, on items and menus) and lists of items (`Menu::children_with`); roles (`MenuRole::{About, Settings, Quit}`); shortcuts (a `Key` and modifiers: `Shortcut::primary` is Command on macOS and Ctrl elsewhere, `Shortcut::new` the key alone, with `shift` and `alt` added); and a window's own menus, a `MenuBar` in its content, written in `view!` (`<MenuBar>`, `<Menu title=…>`, `<MenuItem>`, `<MenuSeparator/>`). `MenuItem::new` takes the title, with `on_select` for the handler, as `Button` has `on_click`.
- **Each platform's bar:** the global `NSMenu` bar on macOS (the app menu, the app's menus, Edit, the rest; a window's own menus while it's the main window); GNOME's primary menu button in each window's header bar, one labelled section per app menu, then Quit (Ctrl+Q); a `MenuBar` in each WinUI window; a `Kirigami.GlobalDrawer` shown as a menu (`isMenu`) in each Kirigami window. The platform's standard menus stay, and their enabling is the platform's. GTK's text widgets have their own Cut, Copy and Paste context menus, so there's no Edit menu there. Shortcuts are installed in every window.
- **A shortcut's key is a character or a key that types none** (`Key::Up`, `Key::Backspace`, `Key::Delete`, `Key::F(2)`…), since file managers' shortcuts are those (⌘⌫ and ⌘↑ in Finder; Delete, F2 and Alt+↑ in Nautilus, Dolphin and File Explorer). A letter is lower case, with Shift apart. Each platform names keys its own way, and context menus read them back the same way: AppKit's key equivalents are characters, the function keys' `NSUpArrowFunctionKey` and so on, and Backspace is `NSBackspaceCharacter`, as Finder's Move to Trash has it (key events carry `NSDeleteCharacter`, which a menu matches too); GTK's triggers use keyval names (`<Alt>Up`, `BackSpace`, `F2`); Qt's `QKeySequence` its portable names (`Del`, `PgUp`); WinUI's accelerators virtual keys, the punctuation keys by their US `VK_OEM_*` codes, which XAML's `VirtualKey` doesn't name. Which keys an app gives which items is its own choice per platform (`platform!`): the files example gives Finder's on macOS and the others' elsewhere.
- **Radio items next to each other form a group,** as a separator ends one on every platform. The core keeps a group exclusive: choosing an item sets the signal, and every item's check comes from it. `MenuData::radio_groups` names each group by its first item, for the platforms that group natively.
- **Check marks are the platform's:** AppKit's item `state` (a check mark for radio items too, as AppKit's menus show a choice); a boolean stateful action on GTK, and for a radio item an action of its own holding its id while chosen, with the id as the item's target, which GTK draws as a radio; `checkable` actions on Kirigami, each radio group in one exclusive `QQC2.ActionGroup`; `ToggleMenuFlyoutItem` and `RadioMenuFlyoutItem` (`GroupName` from the group's first id) on WinUI. XAML and Qt toggle an item themselves when it's clicked, so their backends put back the app's state before reporting the choice; only the user's click reports one.
- **XAML leaves room for a check mark on every row** of a menu with a toggle or radio item, checked or not, so a menu whose one check item is off shows an empty column. Explorer avoids it with plain items that show a check mark icon while on (XAML only makes room for icons some item has), but those read as plain menu items to screen readers, without a checked state. The toggle items stay: they're XAML's own control for this, and the column is how XAML draws it.
- **Ids are unique across bars and stay put while the structure does:** each bar gives the item at each position the same id on every rebuild. So when only enabled and checked states change (`MenuBarData::same_structure`), GTK, WinUI and Kirigami update the items in place, and an open menu stays open. Any other change rebuilds that window's menus. AppKit rebuilds its bar every time.
- **A window's menus are shown with the app's** (`MenuBarData::merged`): a window menu titled like an app menu joins it after a separator, and the others follow. On GTK, WinUI and Kirigami that's what the window shows. On macOS, where the bar is the app's, a window's menus are there while it's the main window (`NSWindowDidBecomeMain`/`ResignMain`), the window menu commands act on. AppKit posts those while a window closes, which the backend does while applying commands with its state borrowed, so the bar follows them once the run-loop turn is over. A window's menus reach the services while its content is built, before the backend has applied the window's `Create`, so backends keep them by `NodeId` and use them when the window is made; Kirigami builds its drawer inline then, as it needs to (§15, Kirigami). A `MenuBar` outside any window is the app's, as `set_menu` is; installing the app's menus again replaces them.
- **Dialogs show only their own menus** (`MenuBarData::for_window`): dialogs on Windows, GNOME and KDE have no menu bar of the app's, nor its Quit. A window's modality comes with its `Create`, so a dialog is never built with the app's menus: Kirigami couldn't take its drawer away without the binding-loop warning (`dialogs_show_only_their_own_menus`). On macOS the bar stays the app's while a dialog is open, as it does for every Mac app: its modal loop disables what doesn't apply.
- **Roles move items only where the platform has a place for them** (`MenuBarData::take_role` takes them out, with separators and menus left empty). AppKit puts About, Settings and Quit in the app menu with its own titles and shortcuts ("About <app>", "Settings…" ⌘,, "Quit <app>" ⌘Q), as Qt's menu roles do. GTK puts them in the primary menu's last section in GNOME's order (Settings, About, Quit), Settings getting Ctrl+, if the app gave it none. Kirigami ends the drawer with them (Settings with KDE's Ctrl+Shift+,, About, Quit, with their theme icons), as KDE apps do. On Windows there's no standard place, so they stay where the app put them. An app's Quit replaces the platform's (AppKit's `terminate:`, GTK's and Kirigami's "ask every window to close"), titled and bound as the platform's is; on Windows it's an ordinary item.
- **Known gap on Kirigami:** a window shows no menu button when the app has no menus of its own, so Quit (Ctrl+Q) needs one.
- **Where it has run:** every backend and headless (`tests/menus.rs`, and the AppKit services test, which posts the main-window notifications itself since test windows are never main); `examples/menus.rs` is for trying them by hand, and they were checked by eye on each. Shortcuts on keys that type none (`context_menu::shortcuts_name_any_key`, the files example's menus, and `examples/shortcuts.rs` for trying by hand) have run headless and on AppKit; on GTK, WinUI and Kirigami they only type-check.

### 14.7 Quitting

- **The platform's own Quit goes to the app** (`Ui::request_quit`): the app's Quit item, as if chosen, if it has an enabled one, otherwise a close request to every window, as by its close button. The app decides either way, and ends when its last window closes. 2ksbox's player needed it: a guest killed while it writes leaves its disk dirty, so the player asks first when it's closed from the keyboard, and on macOS it had routed the Dock's Quit to its window's close request itself.
- **AppKit:** the app delegate's `applicationShouldTerminate:`, which every `terminate:` asks: the app menu's default Quit, the Dock's, logging out and restarting. The app's handlers run in that call (a tick), since the answer is due when it returns. With windows left, it's cancelled, and macOS says the app cancelled the log out, as it does for TextEdit with unsaved documents; with none, the process ends there, so code after `App::run` doesn't run then, as for any AppKit app the system quits. AppKit's default Quit asks the windows too, as GTK's and Kirigami's do. A small app whose window refused the first close request was quit twice from outside (`NSRunningApplication.terminate`, the Dock's quit event): it stayed, then ended.
- **GTK** doesn't use `GtkApplication`, so it takes the routes `GtkApplication` takes, over GIO's D-Bus: outside a sandbox it's a client of `org.gnome.SessionManager`, whose `QueryEndSession` asks the app; with windows left it answers no with a reason, so GNOME lists the app in its log out dialog with "Log Out Anyway". `EndSession` is answered yes, since the session ends whatever the app says by then, and `Stop` ends the run. In Flatpak, where the session manager isn't reachable, the portal's session monitor does the same, holding a logout inhibit while the app keeps windows. Without a GNOME session (Sway) nothing asks, and the session ends as it would.
- **Kirigami:** `QGuiApplication::commitDataRequest`, since Qt 6 closes no windows then; a window left cancels the logout (`allowsInteraction`, then `cancel`), as KDE apps with unsaved work do. If it comes while a tick runs, the app keeps the session: it hasn't been asked. Qt's session management is XSMP, which KWin likely doesn't give Wayland clients, so on Wayland it probably never comes.
- **WinUI:** `WM_QUERYENDSESSION`, in a subclass of each top-level window. The app is asked once per session end, and every window gets that answer; with windows left it's no, with `ShutdownBlockReasonCreate`, so Windows lists the app on its "preventing shutdown" screen, as it lists Notepad with unsaved changes, and the user can end it anyway. A query that comes while the app is being asked, or in a pump inside a `Ui` call, says no with the same reason. `WM_ENDSESSION` forgets the answer, and the reason if the session end was cancelled.
- **Where it has run:** AppKit. Only type-checked on GTK, Kirigami (the shim compiled against Qt 6.11's headers) and WinUI; to verify: that gnome-session shows the reason and lets go when the process ends, the portal's session handle in Flatpak, ksmserver granting interaction, that `WM_QUERYENDSESSION` reaches our subclass while our loop waits, and that Windows lets the app finish once it closes its windows.

### 14.8 The app's id, name and icon

- **`App::id`, `App::name` and `App::icon`** (an `AppInfo` in the core, `Ui::set_app_info`, `Backend::set_app_info`), set before the first window. The id is reverse DNS (`org.example.Player`), the one Linux desktops and Windows want; the icon is an image file or its bytes (`AppIcon::bytes(include_bytes!(…).as_slice())`), PNG everywhere, an `.ico` file too on Windows. There's one icon for the app, not one per window: macOS has no window icons, and Wayland shells show the app's.
- **Each platform takes what it has a place for, and a packaged app's own wins.**
  - macOS: the bundle's id and icon (an asset catalog has the sizes and the dark and tinted variants one image hasn't), so the icon is set (`applicationIconImage`) only when the bundle has none, as for `cargo run`, and the id is ignored. The name goes in the app menu's About, Hide and Quit; the menu bar's bold title is the bundle's or the process's, which only AppKit sets.
  - Linux apps are known by their id: the Wayland app id and X11 class match the `.desktop` file, and the icon theme has the app's icon under that name, which its package installs. Without a `GtkApplication` GTK takes both from the program name, so the backend sets it (`glib::set_prgname`), in `run` before GTK starts (GDK's X11 backend reads the class then) and again in `set_app_info`, and gnome-session's registration sends it. GTK 4 windows only show themed icons, so GNOME apps set the default icon name to their id and so does the backend; the app's image is ignored, and an app run from its build shows no icon, as an uninstalled GNOME app doesn't.
  - Kirigami does as KDE apps do (`KAboutData`): the desktop file name, the display name, which Qt adds after each window's title ("Home — Dolphin"; not when the title already ends with it), and the window icon from the theme by the id, with the app's image when the theme has none.
  - Windows: the AppUserModelID (which groups the app's windows on the taskbar) is set only when the app isn't in a package, whose id is its own, and the name is left to the executable's version resource or its shortcut. Each window gets the icon with `AppWindow.SetIcon`, as a Win32 app's windows get their class's: an `.ico` file's path (with its sizes), or else an `HICON` made from the PNG (`CreateIconFromResourceEx`, which takes PNG data; an `IconId` is that handle, as a `WindowId` is a window's). The `TitleBar` control shows no icon (its `IconSource` isn't set).
- **Tests read it back** (`TestHooks::app_info`, `tests/app_info.rs`): what the platform shows, `None` where it has no place. AppKit keeps the name (only its menus show it), and reports the Dock icon's size in points: it keeps a snapshot of the image at the screen's scale, 40 × 20 pixels for the 20 × 10 PNG on a Retina display.
- **Where it has run:** AppKit and headless. Only type-checked on GTK, Kirigami and WinUI, with Kirigami's shim compiled against Homebrew's Qt; `examples/windows.rs` sets all three, for trying by hand. To verify: that `AppWindow.SetIcon` gives the window the big icon `WM_GETICON` reads back, and that `CreateIconFromResourceEx` makes an icon of the PNG's own size; that GTK updates windows already open for a new default icon name; that `QWindow::icon` falls back to the app's on Qt's offscreen platform.

### 14.9 Alerts and file dialogs

- **Alerts** (`alert(Alert { title, message, buttons, style })`) never block: they reply with the button chosen. AppKit's `NSAlert` is a sheet on the parent window; GTK's `gtk::AlertDialog`, where Escape chooses the last button, and which has no alert styles, so `AlertStyle` is ignored there; Kirigami's `Kirigami.PromptDialog` in the window's overlay; WinUI's `ContentDialog`, one at a time per window. An alert without a parent goes on the focused window (§14.1).
- **File dialogs** (`open_file`, `save_file`) open or save files, and open folders (`OpenFile::directories`): AppKit's `NSOpenPanel` and `NSSavePanel` as sheets, GTK's `gtk::FileDialog`, Qt Quick's `FileDialog` and `FolderDialog` (Plasma's own through its platform theme), and on WinUI the Windows App SDK's pickers, made for the window.
- **`start_folder`** opens the dialog in a folder, as 2ksbox's path fields do (the folder of the file a field names, else the last folder browsed): its users know where their disk images are, and a dialog that starts elsewhere makes them walk there every time. Each backend drops a folder that isn't there, with the core's `services::existing_folder`, so the platform chooses, as with none. AppKit's `directoryURL`, GTK's `initial-folder`, Qt Quick's `currentFolder` (the save dialog's `selectedFile` goes in it when there's a name), WinUI's `SuggestedFolder` (`IFileOpenPicker2`, `IFolderPicker2`; the save picker's is on `IFileSavePicker`). The platform's own memory of the last folder is what applies without one. WinUI also has `SuggestedStartFolder`, which gives way to that memory; it isn't used, since the app asked for this folder.
- **`FileFilter::all(name)`,** a filter with no extensions, lets every file through, offered after a filter by type ("All files"), so a file the types miss can still be chosen: a `.Cue` when the filter lists `cue` and `CUE` (GTK and Qt match case-sensitively on Linux), or a file with an extension the app didn't list. GTK adds the pattern `*` to the filter; Qt reads `All files (*)`; WinUI's open picker takes named choices (`FileTypeChoices`, as the save picker's) when there are filters, the flat `FileTypeFilter` with `*` when there are none. AppKit's panels have no filter menu, only the types they allow, so a filter that lets every file through allows every file. WinUI's save picker leaves it out: a save choice is the extension the name gets.
- **Known gap on Kirigami:** `FolderDialog` picks one folder.
- **On GTK's test display portals are off,** so file dialogs don't open on the real desktop (§15, GTK).
- **Where it has run:** the start folder and filters on GTK and Kirigami natively (each backend's `tests/services.rs`: the dialog's folder and its filters, read back from GTK's `GtkFileChooser` and Qt's `currentFolder` and `nameFilters`, without the portal on GTK) and headless (`tests/services.rs`). Only type-checked on AppKit and WinUI. To verify: that `FileTypeChoices` on the open picker shows as a choice of filters and ignores `FileTypeFilter`, which stays empty then; that `SuggestedFolder` wins over the picker's memory on both pickers; that the portal's dialog (GNOME's, KDE's) honours `initial-folder` and the `*` pattern.

### 14.10 The trash

- **`trash(paths)`** moves files and folders to the user's trash, as the platform's file manager does, so its Put Back or Restore brings them back. It's a service (`Services::trash`), not something an app should build: each platform's trash has its own format, and only its own API keeps what restoring needs. The `files` example did it by hand before; it now calls this.
- **Items go one after another,** and the reply is the first failure, if any: those before it stay in the trash, so an app checks what's left. WinUI queues them all in one `IFileOperation`, as Explorer does with a selection, so a failure or a "no" there covers the batch, and some may have moved before it. **`ServiceError::Unavailable`** means the item's disk has no trash (some network and removable disks), where Finder and Files offer to delete right away, as the `trash` example does after asking; **`ServiceError::Cancelled`** means the user said no to a question the platform asked.
- **Each platform's own:**
  - AppKit: `NSFileManager.trashItemAtURL`, which Finder's Put Back knows. It's synchronous, a move within the disk. `NSFeatureUnsupportedError` is `Unavailable`.
  - GTK: `g_file_trash_async` (`gio::File::trash_async`), one item at a time, as Nautilus does; `G_IO_ERROR_NOT_SUPPORTED` is `Unavailable`.
  - Kirigami: `QFile::moveToTrash`, the freedesktop.org trash KIO uses (with the `.trashinfo` Dolphin restores from, and a disk's own `.Trash-$uid`). Qt doesn't say when a disk has no trash, so every failure is `Failed`, with Qt's reason.
  - WinUI: the shell's `IFileOperation`, as Explorer's Delete (`FOF_ALLOWUNDO` and `FOFX_RECYCLEONDELETE`), owned by the window. The shell asks first when the Recycle Bin's settings say so, offers to delete what's too big to recycle, and shows its progress and errors itself; saying no is `Cancelled`. It never replies `Unavailable`, since the shell's own offer to delete stands in for it. It runs its own message loop, so it starts from the dispatcher queue, outside the `Ui`'s call.
- **Tests answer it** (`app.services().take_trash()`): the scripted services move nothing, so the test does what the platform would have (`tests/trash.rs`, and the `files` suite moves items to a trash folder of its own). The real trash is checked in the backends' `tests/services.rs`: a file of the test's own goes to the user's trash and is deleted from there (AppKit: `~/.Trash`, which this terminal can't list, but a known name can be removed; GTK and Kirigami: `Trash/files` and `Trash/info` under the data folder, where the file is made so it's on the trash's disk).
- **Where it has run:** AppKit and headless (`tests/trash.rs` natively too, and AppKit's services check). Only type-checked on GTK, Kirigami (the shim compiled against Homebrew's Qt) and WinUI. To verify: GTK's and Kirigami's services checks; that `IFileOperation` recycles from a WinUI app without asking by default, that `GetAnyOperationsAborted` or the returned `HRESULT` reports a "no", and that its progress dialog doesn't disturb the XAML window it's owned by.

### 14.11 Opening files and links in other apps

- **`launch(path)` and `launch_url(url)`** (`Services::launch` with a `Launch`) open a file in the app set for its type, a folder in the file manager, an app itself, and a URL in the app set for its scheme (the browser for `https:`, the mail app for `mailto:`): what the platform's file manager does on a double-click. Running `xdg-open` or `explorer` as a process, as the `files` example did on KDE and Windows, skips the platform's own handling (its "which app?" questions, the portal in a sandbox, the window to attach them to).
- **The platform handles "no app":** macOS says so and offers to choose one, GNOME and Windows ask which app. Dismissing that is `ServiceError::Cancelled`; `Unavailable` is no app at all. The reply comes once the platform has handed the file over, not once the app has it open.
- **Each platform's own:**
  - AppKit: `NSWorkspace.openURL(_:configuration:completionHandler:)`, whose handler runs on another thread: the reply waits in a thread-local by ticket, and the handler sends the ticket and the result to the main queue. A string `NSURL` can't parse is `Failed`, without asking the system. Launch Services' `kLSApplicationNotFoundErr` is `Unavailable`, `userCanceledErr` and `NSUserCancelledError` are `Cancelled`.
  - GTK: `gtk::FileLauncher` for paths and `gtk::UriLauncher` for URLs, attached to the window, through the OpenURI portal where there is one. `GtkDialogError`'s dismissed and cancelled are `Cancelled`, `G_IO_ERROR_NOT_SUPPORTED` is `Unavailable`.
  - Kirigami: `QDesktopServices::openUrl` (`QUrl::fromLocalFile` for paths): kde-open on Plasma, the portal in a sandbox. Qt only says whether it worked, and what fails is nearly always that no app opens it, so a failure is `Unavailable`; Plasma shows nothing then, so the `files` example says so itself.
  - WinUI: `ShellExecuteExW` with the default verb, owned by the window, as Explorer's double-click: Windows asks "How do you want to open this?" when no app is set. `ERROR_NO_ASSOCIATION` is `Unavailable`, `ERROR_CANCELLED` `Cancelled`. It may wait on the app and show UI, so it starts from the dispatcher queue, as the trash does.
- **Tests answer it** (`app.services().take_launch()`, `tests/launch.rs` and the `files` suite): nothing is opened. No backend check opens a real app, since that would open windows on the machine running it: `examples/launch.rs` is for trying it by hand, with a text file, a web page, a type no app opens, a folder and a link.
- **Where it has run:** headless, and AppKit through the scripted services; the real call on AppKit is untried (the `files` example used the synchronous `openURL` before). Only type-checked on GTK, Kirigami (the shim compiled against Homebrew's Qt) and WinUI. To verify, with `examples/launch.rs` on each platform: that each opens in its app, that a type with no app is reported as the platform does (and what Launch Services returns after its own dialog), and GNOME's app chooser, dismissed, is `Cancelled`.

---
## 15. Backends

What each backend taught us beyond single widgets. Some of it became part of the contract, and [BACKENDS.md](BACKENDS.md) has the rules; this is the why.

Three rules came from AppKit and now hold everywhere:

- **Native views start with a zero frame.** The core only sends frames that differ from the last one it sent. AppKit controls come with frames of their own, and Qt Quick items follow their implicit size until one is set, so the backends zero them on creation. The mirror check caught this for a `display: none` label.
- **The core owns the Tab order.** AppKit's automatic key view loop orders controls by position on screen, which is wrong for right-to-left layouts and absolute positioning, so the core sends the order (`SetFocusOrder`). The conformance tests use three controls on purpose: with two, wrap-around would make any order pass. They were confirmed to fail with AppKit's and GTK's own orders.
- **Focus requests wait for the structure.** `Ui::focus` right after building a node (a composed field focusing itself) would reach the backend before the node was in a window, where no toolkit can focus it (headless doesn't mind), so focus commands go at the end of the batch's structure.

### AppKit

- **Presses go through `accessibilityPerformPress`,** the path VoiceOver uses. For windows that aren't on screen it returns `NO` even after pressing, so the result is ignored.
- **Typing goes through the field editor** (`insertText:`, `deleteBackward:`, `insertNewline:`), so the delegate and action paths are the real ones. Focusing a field selects all of its text, so the backend puts the caret at the end before typing, as clicking past the end would.
- **Focus** is reported through KVO on `NSWindow.firstResponder`. While a text field is being edited, the first responder is the window's field editor, and its delegate isn't set yet when focus moves, so the backend walks up from the responder through its superviews to the nearest known view. The mirror check compares native focus with the core's on every settle.
- **The Tab order** is a `nextKeyView` loop, with `autorecalculatesKeyViewLoop` off.
- **Scroll views** are an `NSScrollView` with our content view as its document view. Scroll changes are observed through the clip view's bounds-change notifications, including programmatic scrolls, so `Scrolled` fires for both. `NSScrollView` insets its content for the title bar on its own (`automaticallyAdjustsContentInsets`), which showed once a fit-height window shrank (content drawn a title bar's height up while the offset still said 0), so scroll views and lists turn it off: the core places them.
- **Custom widgets:** `NativeRender` views, and a flipped `DrawnView` that rasterizes display lists.
- **Tests run in offscreen windows** with the appearance forced (light, or the story's variant), for comparable captures (`MITSUAMI_SHOW_WINDOWS=1` shows them). The test app is never the active app, so its windows are never key or main, even when shown. AppKit captures therefore show the **unfocused-window look**: default buttons are grey instead of the accent colour, and controls use their inactive colours. A grey Submit button in an AppKit baseline is expected, not a regression. Captures lay the window out first (`layoutSubtreeIfNeeded`).
- **`settle`** lays out tables and toolbars, which make their views only in a layout pass, and fires due timers while a search field is shown (§13.16).
- **The run loop:** `-[NSApplication stop:]` waits for an event, so stopping from an observer posts an empty application-defined event.
- **Its real services** have their own checks (`crates/mitsuami-appkit/tests/services.rs`): a private pasteboard, the real `NSMenu` bar, an alert sheet answered by clicking, a cancelled open panel, and a file moved to `~/.Trash`.
- **Small `swiftc` apps** answer what tests can't reach here: posting real events needs an Accessibility permission, and AppKit's tracking loops read the physical mouse button. The stepper's repeat, full screen's notifications and the tab view's icons were found that way (§13, §14).

### GTK

- **Tests run on a private Broadway display.** The runner starts `gtk4-broadwayd` (bound to localhost) and points GDK at it, unless `MITSUAMI_SHOW_WINDOWS=1`. GTK needs a real, mapped window to capture and to track focus, and on the session display a tiling compositor would override window sizes. The display prints its address, so tests can be watched in a browser. Broadway's frames stall without a browser after a paint, until something new is drawn, so nothing may wait for two frames in a row; that's why `settle` allocates lists, header bars and surfaces itself.
- **The test display has portals off** (`GDK_DEBUG=no-portals`): otherwise file dialogs open on the real desktop, and its dark mode and fonts leak into tests.
- **Windows get an explicit header bar** (libadwaita's, as the titlebar). GTK's default size includes the titlebar; with a header bar of our own, its height is known, and the content gets exactly the size the core asks for. `adw::init` gives the app libadwaita's style.
- **Layout hosts allocate each child at its core frame,** and ask for exactly their own frame (window content hosts ask for nothing, so windows can shrink). Frames live in one map shared by all hosts, so a child keeps its frame when it moves to another parent (the core only sends frames that change). Leaves with an empty frame are hidden from GTK's allocation: controls can't be allocated smaller than their padding. A frame narrower or shorter than the child's minimum (a percentage width, a stretch) is allocated at the minimum and overflows, as in a GTK box: GTK warns otherwise ("Trying to measure GtkGizmo for width of 147, but it needs at least 152").
- **The Tab order is the window content host's `focus` vfunc.** Tab and Shift+Tab walk the core's order and wrap around; arrow keys keep GTK's geometric behaviour.
- **Programmatic changes are muted.** GTK emits `toggled`, `notify::active` and `changed` for `set_active` and `set_text` too, so events are dropped while the backend applies commands. Custom events are muted the same way.
- **Input is synthesized with keybinding signals:** GTK 4 can't inject key events, so keys become the signals they're bound to, on the widgets that handle them: `insert-at-cursor`, `backspace` and `activate` on an entry's text widget, `move-focus` on the window for Tab. Button presses call `clicked` directly: `gtk_widget_activate` would click only after the press animation, asynchronously. Drawn widgets' clicks emit the click gesture's own `pressed` and `released`.
- **Focus is tracked on the window** (`notify::focus-widget`), resolved to the nearest known node: an entry's focus sits on its inner text widget.
- **Scroll views are a `ScrolledWindow` around a `Viewport`.** Content hosts measure as their frame, so the viewport learns the content size. The adjustments are updated as soon as frames arrive, because a `ScrollTo` in the same commit needs the new range before GTK allocates.
- **Min-content text works here:** a wrapping `GtkLabel` reports its longest word as its minimum width.
- **Captures** use the Cairo renderer, whichever renderer the display uses, so baselines don't depend on the GPU, and reply from the frame clock (`after-paint` of the next frame). The content host carries the `background` style class, so captures include the window background. `TestHooks::settle` was added to the contract for this: platforms that complete work asynchronously catch up there.
- **Custom widgets** follow AppKit's shape: `mitsuami_gtk::NativeRender` (a `gtk::Widget` per render, measured by GTK unless the render measures itself), `NativeView::gtk(factory)`, and `mitsuami::gtk::gtk` for the bindings. Drawn widgets are a `DrawingArea` rasterized with Cairo, in the theme's named colours (`accent_color`, `borders`, …) with Adwaita's values as a fallback, so they follow the theme. Native views' accessibility actions do what GTK's do: `activate` for Activate, and a step for spin buttons and ranges.
- **Only real platform controls count as native.** GTK has no rating control, so the example's rating is built ad hoc there, from flat buttons with `starred-symbolic` icons like GNOME Software's. GTK's own widget in the example is `GtkLockButton`, deprecated since GTK 4.10 and gone in GTK 5, but in every GTK 4. It shows a `GPermission`, which gtk4-rs can't subclass, so the example registers a small one through GIO's C API: it reports what the props say, and acquiring or releasing just succeeds. The button's click is the request the app answers, like any controlled widget.
- **No `GtkApplication`:** `run` drives a plain GLib main loop, ticking from an idle source at `HIGH_IDLE`, ahead of GTK's layout and drawing. What `GtkApplication` would give, the backend does the same way: the app id as the program name (§14.8), and the session's end (§14.7).
- **Not done yet:** a reduced GTK 4.8 mode, and single instance.

### Kirigami

The spike (`spikes/kirigami`) settled the route; the backend is `crates/mitsuami-kirigami`.

- **A C++ layer, not bindings.** Qt has no maintained Rust bindings for driving arbitrary QML items (`cxx-qt` exposes Rust objects to QML, the other way round). `cpp/shim.cpp` is a C API over `QObject*`: items are created from one line of QML each (`QQC2.Button { }`, compiled once per text), properties go through `QObject::setProperty`, and every signal reaches Rust through one callback with a key naming a Rust closure, dropped with its object. `build.rs` finds Qt with pkg-config and runs moc on the one header that declares QObjects. The crate builds empty without its `qt` feature, so the workspace builds where Qt isn't installed.
- **Picked by a feature.** `mitsuami`'s `kde` feature swaps GTK for Kirigami (and wins if `gtk` is on too); the test kit has its own `kde` feature; `platform!` has `kde` and `gtk` arms, settled when `mitsuami` is built.
- **Windows are a `Kirigami.ApplicationWindow` with one page,** whose title shows in Kirigami's toolbar; the page's content item is the layout host. The toolbar's height is only known once Kirigami's page stack is laid out, which Qt does when it polishes before a frame. Resizing the window until the content matched overshot (the host lags a polish behind) and shrank the window to a pixel, so the backend polishes the window's items first, measures the toolbar, and checks again at the first frame; windows are sized in whole pixels.
- **Breeze outside Plasma.** Controls use the `org.kde.desktop` style, which draws with the app's QStyle. On Plasma, the KDE platform theme picks that style; anywhere else (Sway, GNOME, the tests), Qt falls back to Fusion: square buttons, arrowed scroll bars, mnemonics always underlined, and no default buttons shown. KDE apps like Kate then pick Breeze themselves (`KStyleManager`), and so does the backend, unless `QT_STYLE_OVERRIDE` names a style. Plasma's look needs the Breeze widget style installed.
- **Measuring is synchronous:** `implicitWidth`/`implicitHeight`, even for the desktop style's QStyle-drawn controls. Wrapping text measures its height by setting `width`; min-content is `Text.WordWrap` at width 1, which leaves the longest word as `contentWidth`.
- **User-only signals.** `toggled` and `textEdited` fire only for the user, so built-in controls need no muting. But a `SpinBox` stepped by a screen reader (`Increase`) changes its value without `valueModified`: native views observe `valueChanged`, and the backend mutes its own updates, as on WinUI.
- **Accessibility actions are Qt's own:** `Press`, `Toggle`, `Increase` through `QAccessible`, as Orca's would. They also focus the control, as a click does, unlike AppKit's press; the focus change is reported like any other. Disabled controls accept them and do nothing, so the backend checks `enabled` first.
- **Real input.** Qt can inject key and mouse events: typing, Backspace, Return, Tab and clicks on drawn widgets are real `QKeyEvent`s and `QMouseEvent`s.
- **The Tab order is an event filter** on the window, since Qt Quick's chain follows item order within each parent; popups (dialogs, menus) keep Qt's own chain. Qt Quick focuses nothing in a new window, where AppKit focuses its initial first responder and GTK its first control, so the first order a window gets focuses its first control that can take focus, if nothing has it.
- **Custom widgets:** renders are QML items, and drawn widgets a `QQuickPaintedItem` painted with `QPainter`. Qt's own controls in the example: the lock is a `DelayButton` and the pager a `PageIndicator`, both native; the rating is built ad hoc from tool buttons with Breeze's star icons, as Discover does.
- **Tests run on Qt's offscreen platform:** no display server, exact window sizes on a 1920 × 1080 screen, software rendering, so captures don't depend on the GPU. The desktop's settings stay out: no platform theme, a private `kdeglobals`, Plasma's default font. Light and dark are Breeze Light and Breeze Dark, switched the way KDE apps' colour scheme menus do (`KDE_COLOR_SCHEME_PATH` and a palette change). The private `kdeglobals` turns animations off (`AnimationDurationFactor=0`): captures caught a switch's knob mid-slide.
- **Teardown.** A backend can be dropped with the thread-locals that hold it as the process exits, after Qt's thread data or KDE's icon loader has gone: destroying a window then crashed. Dropped backends post their windows' deletion instead, a new backend flushes what earlier ones posted (their controls still held Kirigami's Alt-key mnemonics, which moved the underlines in the next test's capture), and at exit a C++ thread-local sentinel drops what is still queued.
- **Qt logs to the systemd journal when stderr isn't a terminal,** so QML warnings vanish from piped or captured output: set `QT_FORCE_STDERR_LOGGING=1` to see them (the tests don't).
- **Kirigami wants popups whole from the start.** A global drawer created without a parent reads the parent it doesn't have yet, and one attached to a finished window (or given its actions late) makes the hamburger button report a binding loop. Windows get their menu drawer in their own QML (menus are set before the commit that creates windows); a menu whose structure changes later still gets a drawer created in the window's overlay, and Kirigami's warning. A `PromptDialog` opened before its window's first frame loops over its position, so alerts wait for that frame. Kirigami's dialog binds its `y` to its `height`, and setting `y` lets Qt resize a popup that doesn't fit or whose implicit height has changed (`QQuickPopupPositioner::reposition`), so the binding loops whenever the dialog's implicit height changes as it slides in or out (a window shrinking, text rewrapping). Alerts bind `y` to the implicit height instead, which Qt places the popup by and never writes, and keep it inside the window. Checked in a QML reproduction that flips the implicit height while the dialog opens and closes (Kirigami's binding and the previous one loop, this one doesn't); the warning seen in `examples/menus.rs` wasn't reproduced directly.
- **Type-checking from macOS:** the shim compiles (without linking) against Homebrew's Qt 6, and QML is linted with Homebrew's `qmllint`, without Kirigami's modules.

### WinUI

**The spike** (M0.5, `spikes/winui`, not a workspace member) built a window, a `Canvas` host, a `Button` and a wrapping `TextBlock`, placed them at our frames, measured them, clicked through UI Automation, and captured the root to a PNG. What it settled:

- **Bindings.** `windows-bindgen` 0.100 generates everything from three sources: the `Microsoft.WindowsAppSDK.WinUI` and `.InteractiveExperiences` NuGet metadata, plus the built-in Windows metadata. The output was about 2.3k lines, against reactor's 29k. `--compose` (needed for `Application`) requires `--minimal`, so filters list members (`IUIElement::{Measure, get_DesiredSize}`), and calls go through `cast::<IUIElement>()` and friends rather than inherited methods. Struct fields are snake_case (`Size { width, height }`).
- **Bootstrap.** Framework-dependent bootstrap works the way `windows-reactor` does it: `TryCreatePackageDependency` and `AddPackageDependency` on `Microsoft.WindowsAppRuntime.2_8wekyb3d8bbwe`, with a minimum version of 2.4, which resolves to any installed 2.x at or above it (2.5.1 on the dev machine). The composed `Application` must merge `XamlControlsResources` and forward `IXamlMetadataProvider` to `XamlControlsXamlMetaDataProvider`, or controls have no templates.
- **Measure needs the live tree.** Before insertion, a `Button` measures `0 × 19` because its template isn't applied yet, and still does once inserted, until the window has loaded. Once the root has loaded, any element appended to a live `Canvas` measures right away, before its own `Loaded`.
- **Our frame size wins over content.** `SetFrame` is `Canvas.SetLeft/Top` plus `Width`/`Height`, and XAML's `Measure` then returns that explicit size, so `measure` sets `Width`/`Height` to NaN (Auto), measures, and restores the frame. With that, measuring after a content or text change is synchronous and correct, with no layout pass in between.
- **Text:** wrapping under a definite width works (at width 120 a label measures 115.3 × 56); at width 0 it wraps per character (0 × 782), so min-content text needs its own approach, such as measuring the longest word.
- **Units:** measured values are logical and snapped to device pixels at the rasterization scale (at 1.5, a width of 77.33 is 116 px). `AppWindow` sizes (`ResizeClient`) are physical pixels and must be scaled.
- **Events:** handlers are Rust closures (`IButtonBase::Click(|_, _| …)`), and the returned `EventRevoker` unsubscribes on drop, so the backend owns the revokers for each node. `IInvokeProvider::Invoke` on the element's automation peer raises `Click` synchronously, which is what `Activate` needs. `DispatcherQueue::TryEnqueue` is the run-loop flush hook.
- **Capture:** `RenderTargetBitmap::RenderAsync(root)` and `GetPixelsAsync`. Completions arrive on the UI thread, but `IAsyncAction::when` wants `Send`, so XAML objects travel through a thread-local. `PrintWindow` yields black, because the content is drawn by composition. The capture follows the system theme, so tests force the light theme, as on AppKit.
- **Shutdown:** all XAML references must be released before `Application::Exit`. Dropping them later, from a thread-local destructor, fails fast with `STATUS_STACK_BUFFER_OVERRUN`.
- **Toolchain:** windows-rs 0.100 links through `raw-dylib`. On `x86_64-pc-windows-gnu` that needs `dlltool`, which rustc doesn't find (the toolchain ships one under `self-contained/`, but rustc doesn't look there), and `windows-reactor-setup` rejects plain gnu anyway. So the backend targets MSVC (`x86_64-pc-windows-msvc`, or `aarch64-pc-windows-msvc`). Cargo can't require a target per host (`rust-toolchain.toml` pins only the channel, `build.target` applies to every OS, `forced-target` is nightly-only), so `mitsuami-winui` pulls its Windows dependencies only for `target_env = "msvc"`, and other Windows toolchains get a `compile_error!` that names the fix: `rustup set default-host x86_64-pc-windows-msvc`, or `cargo +1.96-x86_64-pc-windows-msvc`.

**The backend** (`crates/mitsuami-winui`):

- **Bindings are generated once and checked in** (`src/bindings.rs`, about 17k lines), by the tool in `crates/mitsuami-winui/bindgen/` from the member list in its `filter.txt`. The crate needs neither network access nor `windows-bindgen` at build time. They're public as `mitsuami_winui::bindings`, the escape hatch for native access. Adding a type or member means adding it to `filter.txt` and running the tool.
- **No `Application::Start`.** The backend composes the `Application`, then calls `WindowsXamlManager::InitializeForCurrentThread` (in that order: the other order fails), and pumps messages itself. `run()` is a plain `PeekMessage` loop that ticks before `MsgWaitForMultipleObjectsEx` sleeps until the next timer, like AppKit's run-loop observer. Ticks are also scheduled on the `DispatcherQueue`, which keeps running inside modal loops (live resizing) that bypass ours. Tests use the same pump.
- **Windows are live before anything is measured.** Creating a window activates it right away, layered and nearly transparent, and waits (pumping) for its content's `Loaded`. The window becomes visible after the first layout. Test windows stay that way: alpha 1 rather than 0, because the compositor skips fully transparent windows and XAML's rendering (captures) stalls. They're click-through and hidden from the taskbar.
- **XAML reports asynchronously; the core expects prompt reports.** `GotFocus`, `TextChanged`, `ViewChanged`, `SizeChanged` and `Checked` arrive after the call that caused them. The backend reports what it causes itself right away: focus it moves, values it sets for tests, scrolls it applies with `UpdateLayout`, and window sizes it sets. The late XAML event then finds its value already reported and is dropped. Focus moves XAML makes on its own (a focused control is disabled) are picked up in `settle`, which pumps messages.
- **The title bar.** The Win32 caption ignores the app's theme, so windows extend their content into the title bar and host WinUI's `TitleBar` control, with the caption buttons set to follow the theme (`AppWindowTitleBar.PreferredTheme`). The content size then needs care: `ResizeClient` sizes the area below the caption strip while `ClientSize` and XAML's root include it, and Windows keeps a 1 px resize border inside the client area. `resize_client` corrects by what `ClientSize` reports and by the border measured off the live root.
- **The Tab order.** `TabIndex` is scoped to each container, so it can't express a window-wide order across nested hosts. The window root handles Tab in `PreviewKeyDown` and moves along the core's order itself.
- **Theme resources come from markup.** The window root (the title bar, menu bar and toolbar rows, the content host, the background) is built with `XamlReader::Load`, so `{ThemeResource …}` follows the element's theme (tests force light) and live system theme changes. A brush looked up in code follows the app theme instead.
- **Completions and threads.** XAML operations (`RenderTargetBitmap`, `ContentDialog`) complete on the UI thread. The Windows App SDK's file pickers and the clipboard complete on worker threads, so they hop back through the `DispatcherQueue`. `IAsyncOperation::when` wants `Send` closures, so replies wait in a thread-local. The trash (`IFileOperation`) and launching (`ShellExecuteExW`) run their own loops and UI, so they start from the `DispatcherQueue`, outside the `Ui`'s call (`later::park`, `later::on_ui`).
- **Custom widgets and native views** follow the same shape: `mitsuami_winui::NativeRender` (any XAML element, from `mitsuami::winui::bindings` or bindings of the app's own), `NativeView::xaml(factory)`, and `WinUiCx` with an `Emitter`. `cx.keep(revoker)` ties a subscription to the node.
  - Native renders and native views sit in a `Border` that carries the core's frame, with the control inside. Many XAML controls size themselves (`RatingControl` sets its own `Width` once its template applies), which desynced frames when the frame went on the control. Accessibility props, focus and UI Automation go to the control inside.
  - Observe properties, not change events. `RatingControl.ValueChanged` isn't raised for values a screen reader sets through UI Automation, so a rating set that way never reached the app. `WinUiCx::observe` registers a dependency-property callback instead: it sees every change, and it runs synchronously, so the backend's own prop updates are muted (events raised asynchronously, like `PipsPager`'s `SelectedIndexChanged` after creation, slip past muting).
  - Drawn widgets are XAML shapes built from markup: the display list becomes a canvas of `Path`s loaded with `XamlReader`, and semantic colours become `{ThemeResource …}` brushes (`TextFillColorPrimaryBrush`, `AccentFillColorDefaultBrush`, …), so they follow the element's theme live. Every shape is a path, so strokes are centred on the outline as on the other platforms. Pointer presses and releases on the canvas become `Pointer` events, and `SyntheticInput::Click` emits them directly.
  - Native views' accessibility actions go through the control's UIA patterns: Invoke or Toggle for Activate, RangeValue for Increment and Decrement, Value (or RangeValue) for SetValue.
  - WinUI's own controls in the example: `PipsPager` and `RatingControl`, so the rating is native on Windows as well as on macOS. WinUI has no lock button, so the lock is composed there.
- **Known gaps.** Self-contained deployment (`windows-reactor-setup`) isn't wired up: apps need the Windows App Runtime 2.4 or later installed.

---

## 16. Plan

mitsuami was started to replace the Qt Quick launcher of 2ksbox, a Windows 98/XP emulator (`launcher-qt/qml` in that repository), and its player's winit window. What comes next is driven by what they need (§13), and only what every platform has a native control for.

| Milestone | Scope | Done when |
|---|---|---|
| **M0: core and test harness** ✅ | Workspace; `mitsuami-reactive`; node tree; styles and units; Taffy; the `Command` protocol; the headless backend; `mitsuami-test` basics (runner, a11y queries, actions, settle, snapshots) | A counter and a flex/grid form pass headless integration tests written with the public test API; the reactive suite passes |
| **M0.5: WinUI spike** ✅ | Throwaway, using windows-rs 0.100: a window, a `Canvas`, a `Button`, click and measure, driven imperatively (`spikes/winui`) | Integration route chosen (§18) |
| **M1: AppKit** ✅ | Window, container hosts, Text, Button, TextInput, Checkbox, Switch; run-loop flush; measure; resize and relayout | The M0 tests pass natively on macOS; the conformance suite passes; `Backend::capture` works and a first visual baseline exists |
| **M2: GTK 4** ✅ | The same widget set | The same tests and conformance suite pass natively on Linux |
| **M3: WinUI 3** ✅ | The same widget set | The same tests and conformance suite pass natively on Windows |
| **M4: escape hatches** ✅ | `platform!`, `NativeView`, `CustomWidget` and `NativeRender`, with drawn and composed fallbacks | One screen for every platform, with three custom widgets, each native where the platform has the control and built ad hoc, drawn or composed elsewhere |
| **M5: ergonomics** ✅ | `#[component]`, `view!`, stores, resources | The examples rewritten with macros |
| **KDE Plasma** ✅ | Qt Quick and Kirigami backend (`mitsuami-kirigami`, the `kde` feature), after a spike (`spikes/kirigami`) | The same tests and conformance suite pass natively on Kirigami |
| **M6: visual review** ✅ | Stories, light and dark variants, perceptual and layout diffs, `cargo mitsuami visual review`, CI on macOS, Windows, Linux and KDE | A change to a widget shows up as a reviewable visual diff on every platform |
| **M7: lists** ✅ | A virtualised `List` on each platform's list control (§4) | A 10,000-row list scrolls, selects and filters on every backend, and the `lists` and `contacts` suites pass natively |

Since then: every widget got its survey of shared options, its own example (ToggleButton shares Button's) and a story; the app shell (windows opened while the app runs, modal windows, toolbars, sidebars, tabs, menus, context menus, the platform's quit); and 2ksbox's player (`GpuSurface`, full screen, window sizes). A Finder-like app (the `files` example) then asked for what every file manager has: the trash and opening files in their apps as services (§14.10, §14.11), files' own icons and thumbnails (`FileIcon`, §13.30), and dragging rows out as files (§13.31).

Still to do:

- **Visual baselines** reviewed by eye on every platform, and CI run again: development has moved faster than baselines could follow, and much of §13 and §14 is only type-checked on some platforms.
- **Accessibility for drawn widgets** (native controls already get their labels and roles), a fixed locale and text scale in tests, capabilities (§11), and animations.
- **Shell components:** `Preferences`; document tabs (closable, reorderable), which are another widget: native window tabs on macOS, libadwaita's `AdwTabView`, WinUI's `TabView`.
- **Left out on purpose:** an `AppShell` type (`App`, `Window`, `Sidebar` and `Toolbar` cover it) and a devtools inspector, for now.

---

## 17. Decision log

| Decision | Choice |
|---|---|
| Name | **mitsuami** (三つ編み, "three-strand braid": native toolkits woven into one) |
| GTK flavour | gtk4, with libadwaita (1.4) for shell components: the sidebar's split view, the header bar, and from 1.7 the tab view's inline view switcher. `adw::init` gives the app libadwaita's style |
| Reactivity | Our own single-threaded runtime (`mitsuami-reactive`), not a reused one |
| `view!` syntax | JSX-like; the builder API remains the real API |
| OS support | Backend floors, and capabilities above them (§11); the Windows floor is whatever Windows App SDK 2.4 supports |
| Layout ownership | Ours (Taffy); native widgets positioned absolutely |
| Toolkit per platform | Strictly native: AppKit, WinUI 3, GTK 4, one per OS, no cross-toolkit builds. Linux has a second native toolkit, Qt Quick with Kirigami for KDE Plasma, picked at build time with the `kde` feature |
| KDE backend | Qt Quick Controls and Kirigami (what current KDE apps use), not Qt Widgets. Driven through a small C++ layer compiled with `cc` and `moc` (no `cxx-qt`, no CMake): each node is a QML item created from a line of QML, its props set by name |
| Windows bindings | windows-rs 0.100 (the `windows-reactor` ecosystem, WinAppSDK 2.4); MSRV 1.95, edition 2024 |
| WinUI integration | Our own `windows-bindgen` bindings (minimal mode, member filters) over the WinAppSDK metadata, driven imperatively. No `windows-reactor` at run time; `windows-reactor-setup` stays an option for self-contained deployment (§15, WinUI) |
| Platform and run-time checks | The platform is compile-time (`platform!`); capabilities are run-time |
| Testing | No unit tests. Integration (headless) and end-to-end (native) tests with one API; a11y-driven queries and actions; Chromatic-style visual regression; the testing toolkit shipped to app authors |
| Modal windows | Both window-modal and app-modal, the app's choice, rather than one picked for macOS, where they look different (§14.1) |

---

## 18. Risks

1. **WinUI 3 through `windows-rs` 0.100 and `windows-reactor`.** Retired by the M0.5 spike (§15, WinUI).
   - Microsoft ships `windows-reactor`, a declarative WinUI 3 framework (60+ controls, targeting Windows App SDK 2.4.0), and `windows-reactor-setup`, which stages the App Runtime for self-contained apps.
   - The problem was integration, not access. Reactor has its own component, state and effect system, we must not run two reactive systems on top of each other, and we need *imperative* access to the XAML elements.
   - The options were: (a) drive reactor's own elements imperatively; (b) generate the XAML bindings ourselves the way reactor does, and keep reactor only for bootstrap and staging; (c) host reactor components for leaf widgets.
   - **Chosen: (b).** (a) is closed: reactor's `ElementRef` only offers narrow requests (focus, swap chain, WebView2, child visual), with no element handle and no `Measure`, and reactor's WinUI bindings are crate-private. (b) works end to end with `windows-bindgen` 0.100, with no XAML subclassing: the `Application` is composed from Rust, and hosts are plain `Canvas`es. The C++/WinRT shim fallback wasn't needed.
2. **Measurement fidelity.** Native intrinsic sizes can be quirky: `NSTextField` wrapping, WinUI's `Measure` needing the live tree, Qt Quick sizing when it polishes. Headless tests can't catch this; the conformance suite, the mirror check and the stories cover it.
3. **Our own reactive runtime is on the critical path.** Everything depends on it: effect ownership and cleanup, batching, and flush ordering relative to layout. Its scenario-based suite came first, before anything was built on it.
4. **What's only type-checked.** Much of §13 and §14 has run on one or two platforms and only type-checks on the others, and CI hasn't run for a while. Each section says which; the maintainer runs CI in a later pass.

---

## 19. Open questions

- **The app icon for bundles:** `App::icon` takes one image (PNG, or `.ico` on Windows), and packaged apps' own icons win (§14.8). Still open: whether bundling tools (`cargo mitsuami`) should own per-platform assets (an asset catalog on macOS, the icon theme on Linux, `.ico` sizes on Windows).
- **The exact Windows floor** for Windows App SDK 2.4. The InteractiveExperiences metadata still ships a 10.0.17763 variant, which suggests 1809 holds.
- **Visual baseline storage:** plain files in git to start with, then LFS. Revisit when baselines grow (4 backends × variants × stories).
