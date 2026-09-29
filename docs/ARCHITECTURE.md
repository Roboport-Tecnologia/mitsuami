# Architecture — native, declarative, cross-platform UI for Rust

> Name: **mitsuami**.
> Status: design draft, pre-MVP.

## 0. Goals and non-goals

**Goals**

- Native widgets only: AppKit (macOS), WinUI 3 (Windows), GTK 4 (Linux), or Qt Quick with Kirigami for KDE Plasma (Linux, a cargo feature).
- One shared declarative layer, Vue-inspired (setup-once components, `signal`/`computed`/`watch`, props/emits/slots, provide/inject).
- Layout owned by us: flexbox + grid (CSS semantics) with abstract units (`px`, `em`, `rem`, `%`, `vw`, `vh`, `fr`, platform tokens).
- HiDPI works without app code doing anything special.
- Accessibility-ready from day one: every node carries semantics even if backends ignore them at first.
- First-class escape hatches:
  1. Platform-specific **screens** that share the same logic.
  2. Raw **native views** embedded in the shared tree.
  3. **Custom widgets** with generic logic and one render per platform.
- Platform backends are detached from the core, behind a narrow, data-oriented contract.
- Testing is first class: integration and e2e tests with one API, visual regression, and a testing toolkit shipped to app authors (§12).

**Non-goals (for now)**

- Pixel-identical rendering across platforms. We want *native feel*, not sameness.
- Arbitrary CSS styling of native controls. Visual styling is semantic (variants, text styles), not pixel-level.
- Mobile targets. The architecture shouldn't rule them out, but they aren't on the roadmap.

---

## 1. Big picture

```
┌──────────────────────────────────────────────────────────────────────┐
│  App code                                                            │
│  ┌───────────────┐   ┌──────────────────────────────────────────┐    │
│  │ Domain logic  │   │ Views (components)                        │    │
│  │ plain Rust,   │◄──│ shared views + optional per-platform ones │    │
│  │ no UI deps    │   │ view! / builder API                       │    │
│  └───────────────┘   └──────────────────────────────────────────┘    │
│          ▲ stores / composables (use_xxx) bridge the two             │
├──────────┼───────────────────────────────────────────────────────────┤
│  mitsuami-core                                                         │
│  reactive runtime · component model · node tree (source of truth)   │
│  style + unit resolution · layout (Taffy) · a11y tree · focus ·     │
│  event dispatch · scheduler (batching, flush)                        │
├──────────────────────── Backend contract ────────────────────────────┤
│   Commands (data) ──►                       ◄── Events (data)        │
│   measure(id, constraints) (sync query)     run loop / services      │
├───────────────┬──────────────────────┬───────────────┬───────────────┤
│ mitsuami-appkit │ mitsuami-winui         │ mitsuami-gtk    │ mitsuami-headless│
│ objc2         │ windows-rs + WinAppSDK│ gtk4-rs       │ integration tests│
└───────────────┴──────────────────────┴───────────────┴───────────────┘
```

"Backend" means two different things here, and they stay separate:

- **Platform backend**: the AppKit, WinUI or GTK renderer. It is detached from the core by the command/event contract (§5).
- **App backend**: the application's domain logic. It is plain Rust that knows nothing about UI. Every platform's views bind to the same logic (§8).

### Crate layout

| Crate | Responsibility |
|---|---|
| `mitsuami-reactive` | Signals, computed values, effects, watchers, scopes/ownership, batching. Single-threaded, no UI knowledge. |
| `mitsuami-core` | Node tree, components, widget kinds and props, styles and units, layout (Taffy), a11y model, focus, events, scheduler, backend trait. |
| `mitsuami-widgets` | Built-in widget definitions: typed props, events and a11y defaults. Platform-free. |
| `mitsuami-macros` | `#[component]` and `view!`. Pure sugar over the builder API. (`platform!` is a `macro_rules!` in `mitsuami`.) |
| `mitsuami-appkit` / `mitsuami-winui` / `mitsuami-gtk` / `mitsuami-kirigami` | Backend implementations and their `NativeRender` traits for custom widgets. |
| `mitsuami-linux` | `GpuSurface` for the Linux backends: a desync subsurface of the window's surface on Wayland, a child window on X11, and their pointer locks (§16). Empty elsewhere. |
| `mitsuami-headless` | In-memory backend with deterministic fake measurement and wireframe rendering. The default backend for integration tests. |
| `mitsuami-test` | Public testing toolkit (§12): test runner, a11y-based queries, actions, assertions, fake clock and services, snapshots, stories, visual capture and diff. |
| `cargo-mitsuami` | CLI: `visual` (run, diff, review, accept baselines), and later scaffolding and gallery. |
| `mitsuami` | Facade crate. Re-exports everything and selects the backend by `cfg(target_os)`: AppKit on macOS, WinUI 3 on Windows, and on Linux GTK 4, or Qt Quick with Kirigami with the `kde` feature. There is no cross-toolkit override: a toolkit is only used on its own platform. |

---

## 2. Rendering model: fine-grained reactivity over a retained tree

This is Vue 3 **Vapor mode**-style, closer to Solid or Leptos than to a VDOM:

- A component function runs **once** (Vue's `setup()`). It creates signals and returns a view description.
- Building the view creates **nodes** in a retained tree owned by `mitsuami-core`. Each node gets a stable `NodeId`.
- Every dynamic prop is an **effect** bound to exactly one `(NodeId, Prop)` pair. When a signal changes, only that prop is re-sent to the backend. There is no diffing of whole subtrees.
- Structural changes go through control-flow primitives: `Show` (v-if), `For` with keys (v-for, keyed reconciliation), `Switch` / `Match` and `Dynamic` (`<component :is>`).

Why not a VDOM? Native widgets are expensive to create and have their own internal state (focus, selection, scroll position, IME). Fine-grained updates mutate them surgically and never recreate them by accident.

### The node tree is the source of truth

```rust
struct Node {
    id: NodeId,
    kind: WidgetKind,          // Button, Text, View, …, Custom(&'static str), Native
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    style: ResolvedStyle,      // layout + semantic visual props
    layout: taffy::NodeId,
    a11y: A11yProps,           // always present, see §7
    handlers: EventHandlers,
    frame: Rect,               // last computed layout, logical units
}
```

Layout, a11y, focus order, hit testing for custom widgets and tests all read this tree. The native tree is a **mirror** of it.

### Update pipeline (one "tick")

1. Something happens: a native event, a timer, or a task completes on the UI thread.
2. Handlers run and mutate signals. Mutations are **batched**.
3. Effects flush. Each queues a `Command` (prop updates, inserts, removals) and marks layout dirty where needed.
4. Taffy computes layout for the dirty roots. It calls back into the backend for **native measurement** of leaf widgets.
5. Frame diffs become `SetFrame` commands.
6. The backend applies the whole batch in one transaction (`CATransaction`, a single dispatcher pass, or GTK's frame clock).

The backend schedules the flush through the platform run loop: a `CFRunLoopObserver` (before waiting) on macOS, a `DispatcherQueue` on Windows, and a GLib idle source or frame clock on Linux.

---

## 3. Layout and units

### Decision: we own layout; native widgets are positioned absolutely

This is the React Native / Yoga model:

- Every container (`View`, `Row`, `Column`, `Grid`, `Stack`) maps to a thin native **layout host** that does no layout of its own:
  - macOS: a flipped `NSView` subclass.
  - WinUI: a `Canvas`. A custom `Panel` would need composable-type subclassing from Rust, which is hard.
  - GTK: a `gtk::Widget` subclass whose `size_allocate` places children at our computed frames.
- Leaf widgets report their **intrinsic size** through `measure(id, known, available)`:
  - macOS: `fittingSize` / `intrinsicContentSize`, plus `preferredMaxLayoutWidth` for wrapping text.
  - WinUI: `UIElement.Measure` + `DesiredSize`, only once the element is in the live tree, with our frame's `Width`/`Height` lifted to Auto for the call (§17).
  - GTK: `gtk_widget_measure`.
- [Taffy](https://github.com/DioxusLabs/taffy) computes flexbox, grid and block layout, using those sizes for leaves.

Pros: identical layout semantics on all three platforms, CSS mental model, and one layout engine to test headlessly.

Cons: we give up native auto-layout behaviours, and we must handle RTL mirroring ourselves (§7). Measurement calls cross into native code synchronously, so we cache them per `(node, constraints, content version)`.

`ScrollView` is the exception at the edges. The native scroll container scrolls, and we lay out its content.

`List` goes further: it is the platform's list control (NSTableView, ListView, gtk::ListView, QML `ListView`). Virtualising lists is a solved problem, and the platform solves it: it scrolls, decides which rows to realise (prefetching around the view), recycles them, and owns the selection. We build and lay out what's in the rows.
- **The platform decides which rows exist.** When it realises a row, the backend reports `RowShown(key)` and the core mounts that row, in a host `Container` carrying its `RowKey`; when it lets one go, `RowHidden(key)`, and the core disposes it. The data is just the keys (`Prop::Rows`).
- **The core lays out each mounted row** on its own, at the width the list gives its rows (`RowWidth`), and sends its size; the platform makes the row that high and places it. Where rows are is the platform's: the core reads a row's position back (`native_state`) for frames, visibility and the a11y tree, and scrolls to a row with `ScrollToRow`, which works for rows that aren't mounted.
- **Rows are keyed** like `For`'s: a data change keeps the mounted rows whose keys stay, with their state.
- **Selection and activation are the platform's:** backends report `Changed(Rows)` and `RowActivated`; the app binds the selected keys (`List::selected`) and handles `on_activate`. Rows read as list items named by their text.
- **How a list sits in its surroundings is a semantic choice** (`List::list_style`), like `ButtonStyle`: `Plain` (edge to edge: sidebars, main content) or `Framed` (a bordered box on the content background: a list in a form or dialog), each drawn the platform's way: Breeze's scroll view frame on Kirigami, `ScrolledWindow` `has-frame` on GTK, the bezel border on AppKit, a card's border on WinUI. `Automatic`, the default, is `Plain` everywhere. Where platforms' habits differ (KDE frames more lists than GTK or macOS), the app picks per platform with `platform!`: `.list_style(platform! { kde => ListStyle::Framed, _ => ListStyle::Plain })`. A frame takes room from the rows, which backends report with `RowWidth`. Run on all four platforms.

### Units

```rust
enum Length {
    Px(f32),        // logical px = DIP = point. NOT physical pixels.
    Em(f32),        // relative to the node's inherited font size
    Rem(f32),       // relative to the platform body font size (follows user text-size settings)
    Percent(f32),   // of the containing block (Taffy-native)
    Vw(f32), Vh(f32), Vmin(f32), Vmax(f32),  // of the window's content area
    Fr(f32),        // grid tracks only
    Token(Spacing), // platform design tokens: Spacing::{Xs,Sm,Md,Lg,Xl}, ControlHeight, …
    Auto,
}
```

- **HiDPI is handled for free.** All three toolkits already work in logical units (points, effective pixels, GTK logical px × scale). We never touch physical pixels, and a scale change needs no relayout.
- `em`/`rem`/`vw`/`vh`/`Token` are resolved in core before handing styles to Taffy, which only knows length, percent and auto. Core tracks which nodes depend on the viewport or on font size, so a window resize or a system text-size change re-resolves only those nodes.
- `Token` gives platform-appropriate spacing. For example, `Spacing::Md` can be 8pt on macOS, 12 epx on WinUI and 6/12px on GNOME. The backend provides the values through `PlatformMetrics`.
- Ergonomics: `16.px()`, `1.5.em()`, `50.pct()`, `100.vw()`, `1.fr()`, `Spacing::Md`.
- **Windows can fit their height to the content:** `WindowSize::FitHeight(width)`. Control heights differ per platform, so the same content adds up to a different height on each one. Core lays the window out once at max-content height, then sends that height through the ordinary `SetWindowSize`, so backends need nothing new. Fitting happens at the window's first layout only; after that it's an ordinary window that the user can resize and that doesn't jump when content comes and goes. Platforms size windows in physical pixels, so the size they report back can differ from the requested one by a fraction of a point.
- **Or follow it as it changes:** `WindowSize::FollowHeight(width)`, whose height the user can't change, or `FollowHeightUntilResized(width)`, an ordinary window once the user changes its height (§16, Full screen and window size).

### Responsive / adaptive

- `use_viewport()` returns a signal of the window's content size, like media queries.
- `use_size(node_ref)` returns a signal of a node's size, like container queries: `let panel = node_ref();` and `.node_ref(panel)` on the node (§16, Measurements).
- `Platform::current()` is also available at runtime, but compile-time `platform!` (§6) is preferred.

---

## 4. Styling (what can and can't be styled)

A style has two halves:

1. **Layout:** the full flex and grid property set (`direction`, `gap`, `padding`, `margin`, `align_*`, `justify_*`, `grow`, `shrink`, `basis`, `grid_template_*`, `grid_area`, `position`, `inset`, `min/max/size`, `aspect_ratio`, `overflow`). It applies to every node.
2. **Semantic visual:**
   - Text styles: `TextStyle::{LargeTitle, Title, Headline, Body, Callout, Caption, Monospace}`. These map to `NSFont.preferredFont(forTextStyle:)`, the WinUI type ramp, and GTK/libadwaita style classes.
   - Text options every platform's label has: a colour (`Text::color`, semantic or `Rgba`), a weight over the style's (`FontWeight::{Regular, Medium, Semibold, Bold}`), italics, and an alignment (`TextAlign::{Start, Center, End}`, which the core resolves to left or right for the text's direction). No font family or point size: they'd bypass the type ramp and the user's text size (§16, Text: colour, weight, italics and alignment).
   - Button roles and styles: `ButtonRole::{Normal, Default, Cancel, Destructive}` (what the button does: Return clicks the default one, Escape the cancel one, where the platform does that) and `ButtonStyle::{Automatic, Bordered, Borderless}` (how it's drawn). Each platform maps them its own way (`keyEquivalent` and `bordered` on macOS, `suggested-action` / `destructive-action` and `has-frame` on GTK, `Accessible.defaultButton` and `flat` on Qt, `AccentButtonStyle` and `SubtleButtonStyle` on WinUI) and ignores what it has no equivalent for.
   - Past the semantic props, a widget's `.native(tweak)` sets raw platform settings (§6.4).
   - Semantic colors (`Color::Label`, `Color::SecondaryLabel`, `Color::Accent`, `Color::Separator`, `Color::Error`, `Color::Warning`, `Color::Success`, …) that follow dark mode and high contrast.
   - Containers (layout hosts) can also take a background, border, corner radius and opacity, because they are plain views.

Raw RGB is allowed on containers and text. It is deliberately not exposed on native controls.

---

## 5. Backend contract

> Implementing a backend? The step-by-step guide is [BACKENDS.md](BACKENDS.md).

The core speaks **only in `NodeId`s and plain data**. Each backend keeps its own `NodeId → native handle` map, so core has no generic parameters and no `dyn Any` handles.

```rust
// As implemented (crates/mitsuami-core/src/{command,backend,services}.rs).
pub enum Command {
    Create        { id: NodeId, kind: WidgetKind, props: Vec<Prop> }, // zero frame; initial props, reactive ones too
    SetProp       { id: NodeId, prop: Prop },
    Insert        { parent: NodeId, child: NodeId, index: usize },  // a ScrollView has exactly one child
    Remove        { parent: NodeId, child: NodeId },
    Destroy       { id: NodeId },       // every native node of a removed subtree, children first
    SetFrame      { id: NodeId, frame: Rect },          // parent-relative, logical units; never for windows
    SetA11y       { id: NodeId, a11y: A11yProps },      // backends may no-op initially
    SetWindowSize { id: NodeId, size: Size },
    SetFocusOrder { window: NodeId, order: Vec<NodeId> }, // Tab order, owned by the core
    ScrollTo      { id: NodeId, offset: Point },        // already clamped; backend reports Scrolled
    ScrollToRow   { id: NodeId, row: RowKey },          // lists: the platform scrolls to a row
    Focus         { id: NodeId },
}

pub enum UiEvent {   // backend → core, through the EventSink
    Click, Changed(EventValue), Submit, FocusIn, FocusOut, Scrolled(Point),
    RowShown(RowKey), RowHidden(RowKey), RowActivated(RowKey), RowWidth(f32),   // lists (§3)
    WindowResized(Size), WindowCloseRequested, MetricsChanged,
    Pointer(PointerEvent),  // drawn custom widgets (§6.3)
    Custom(AnyValue),       // custom widgets and native views, their own event types
}

pub trait Backend {
    fn init(&mut self, events: EventSink);
    fn metrics(&self) -> PlatformMetrics;             // fonts, spacing tokens, scale, color scheme, reduced motion…
    fn apply(&mut self, batch: &[Command]);
    fn measure(&mut self, id: NodeId, request: MeasureRequest) -> Size;
    fn services(&self) -> Box<dyn Services>;          // clipboard, dialogs, menus (replaceable: tests use a fake)

    // Testing and accessibility hooks: part of the contract from day one.
    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError>;     // act on the native control
    fn synthesize(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError>; // keys, scroll wheel
    fn native_state(&self, id: NodeId) -> Option<NativeState>; // props, frame, children, focus, scroll offset
    fn capture(&mut self, id: NodeId, reply: Reply<Result<Image, CaptureError>>); // offscreen screenshot
}

pub trait Services {
    fn clipboard_text(&mut self, reply: Reply<Option<String>>);
    fn set_clipboard_text(&mut self, text: &str, reply: Reply<Result<(), ServiceError>>);
    fn alert(&mut self, parent: Option<NodeId>, alert: &Alert, reply: Reply<usize>);   // never blocks
    fn open_file(&mut self, parent: Option<NodeId>, request: &OpenFile, reply: Reply<Option<Vec<PathBuf>>>);
    fn save_file(&mut self, parent: Option<NodeId>, request: &SaveFile, reply: Reply<Option<PathBuf>>);
    fn set_menu(&mut self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>); // the app's or a window's; keeps the platform's standard menus
}
```

Anything a platform might complete later is **reply-based** (async), even when AppKit answers immediately: `capture`, clipboard reads and writes, dialogs. `measure`, `native_state` and `metrics` stay synchronous; see [BACKENDS.md §11](BACKENDS.md#11-sync-and-async-in-the-contract).

A platform's **run loop** drives the `Ui` through three hooks:
- **`Ui::tick()`** runs ready tasks and due timers, dispatches events and commits, repeating until idle. Call it before the loop sleeps.
- **`Ui::set_commit_scheduler`** and **`Ui::set_waker`** (thread-safe) make the loop turn when something changes.
- **`Ui::time_to_next_timer()`** says when to wake up for `sleep`.

On AppKit, these are a `CFRunLoopObserver` and one re-armed `CFRunLoopTimer`, both in common modes.

Properties of this contract:

- **Serializable and inspectable.** Commands can be logged, snapshot-tested and replayed. They could even be sent to a devtools inspector later.
- **The headless backend is trivial to write.** It makes layout, trees and a11y fully testable in CI without a display, and it is the default backend for integration tests (§12).
- **Controlled inputs** (`v-model`) avoid feedback loops: the backend emits `Changed(text)`, core updates the signal, and the effect sends `SetProp` back only if the value differs from what the native widget already holds. This preserves the cursor, selection and IME composition.
- **Threading:** all UI work runs on the main thread, and the reactive runtime is `!Send`.
  - `spawn_local` runs futures on the UI thread.
  - `spawn_blocking` runs work on another thread and resumes the task on the UI thread. It uses standard `Waker`s, which call the run loop's thread-safe waker.
  - `sleep` uses the `Ui`'s clock, which tests replace with a manual one (`app.advance(...)`).
- **Scopes:** tasks and event handlers run in the reactive scope of the component that created them. `inject`, `spawn_local` and `sleep` therefore work inside them, and disposing the component cancels its tasks.

---

## 6. Escape hatches

> Working example: `crates/mitsuami/examples/escape_hatches/`, tested by `crates/mitsuami/tests/escape_hatches.rs`.

### 6.1 Platform-specific screens with shared behaviour

Logic lives in **composables** (Vue's `useXxx`) or **stores** (Pinia-like). Views are thin, so writing two views costs little.

```rust
// shared, platform-agnostic: signals and actions, one instance per app
#[derive(Clone, Copy)]
pub struct Review { pub stars: Signal<u8>, pub comment: Signal<String>, /* … */ }
impl Store for Review { fn create() -> Review { /* … */ } }

// every screen: let review = use_store::<Review>();
pub fn review_screen() -> impl View {
    platform! {
        macos => macos::review_screen(),   // trailing labels, NSStepper, button at the trailing edge
        _     => shared::review_screen(),  // the Windows/Linux version
    }
}
```

- `platform!` is compile-time (`cfg`), so code for other platforms is never compiled into the binary. Arms may have different types.
- Arms are `macos`, `windows`, `linux`, several joined with `|`, and a final `_`. The first matching arm wins.
- On Linux, `gtk` and `kde` name the toolkit, settled when `mitsuami` is built (its `kde` feature); `linux` matches either.
- Without a `_` arm, building for a platform no arm names fails, so a missing screen can't ship by accident.
- Per-platform view files follow a convention: `review/mod.rs`, `review/macos.rs`, `review/shared.rs`. The shared screen is compiled everywhere so it can be tested everywhere.
- Capability predicates on arms (`macos if has(…)`) wait for capabilities (§11).

### 6.2 Raw native view inside the shared tree

```rust
NativeView::appkit(|cx: &mut AppKitCx| {
    let stepper = NSStepper::new(cx.mtm());
    let emitter = cx.emitter();
    cx.on_action(&*stepper, move |s| emitter.emit(s.doubleValue().round() as u8));
    stepper
})
.update(review.stars, |stepper, stars| stepper.setDoubleValue(*stars as f64))  // re-applied when stars changes
.on_event(move |stars: &u8| review.rate(*stars))
.measure(|view, request| /* optional; default intrinsicContentSize */)
.a11y_label("Stars")
```

- It takes part in layout, a11y and events like any other node. Core sees `WidgetKind::Native`.
- The factory and the current value of every `update` travel as one `Prop::Native` payload (an `Opaque`: compared by identity, printed as its label). When any value changes, the payload is sent again and every update re-applied.
- Native callbacks never touch signals directly; they `emit` events that are queued and dispatched on the next turn, like any native event.
- Accessibility actions (`Activate`, `Increment`, `Decrement`) go to the view's accessibility element, as VoiceOver's would.
- Headless tests show a native view as an empty box sized by its styles.
- `mitsuami::appkit` re-exports `objc2`, `objc2_app_kit` and `objc2_foundation` at the backend's versions.

### 6.3 Custom widgets: generic logic plus one render per platform

Custom widgets come in three tiers. Pick the lowest that works:

1. **Composition:** built from existing widgets. It runs everywhere, and it's the tier for what the canvas can't draw (text, editing). A plain component works; as a custom widget's render (`Renderer::composed`), it keeps the widget's props and events, so it can stand in for a native render.
2. **Drawn:** a shared `Drawn` implementation using a small 2D API (`Canvas`: fill and stroke rects, rounded rects, ellipses and paths) with semantic colors (`Color::Accent`, `Color::Label`, …). It runs everywhere and follows dark mode and the accent color, but isn't truly native.
3. **Native per platform:** one shared definition plus one render per platform.

```rust
// Shared definition — platform free.
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
impl NativeRender for Rating {                 // trait defined by mitsuami-appkit
    type View = NSLevelIndicator;
    fn create(p: &RatingProps, cx: &mut AppKitCx) -> Retained<NSLevelIndicator>;
    fn update(v: &NSLevelIndicator, old: &RatingProps, new: &RatingProps);
    fn measure(v: &NSLevelIndicator, p: &RatingProps, request: &MeasureRequest) -> Option<Size> { None } // None = intrinsicContentSize
    fn read(v: &NSLevelIndicator, p: &RatingProps) -> RatingProps;  // read back, for the mirror check
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

- A usage site is `Rating::view(move || RatingProps::new(stars.get())).on_event(…)`, the same on every platform. `.drawn()` and `.composed()` pick those renders where a native one exists.
- **A widget native on one platform stands in on the others.** The renderer uses the first render it has: native, then drawn, then composed. A native render is either the platform's own control (`native::<W>()`) or built **ad hoc** from the platform's widgets the way that platform's apps build it (`ad_hoc::<W>()`), so it still looks at home. `Renderer::is_native()` is true only for the platform's own control, so a screen can be honest about it.
- The `escape_hatches` example is one screen for every platform with three such widgets:
  - a lock: native on GTK (`GtkLockButton`) and KDE (Qt's `DelayButton`, which acts once held), composed elsewhere;
  - a rating: native on macOS (`NSLevelIndicator`) and Windows (`RatingControl`), ad hoc on GTK and KDE (star buttons, as GNOME Software and Discover build it), drawn elsewhere;
  - a pips pager: native on Windows (WinUI's `PipsPager`) and KDE (Qt's `PageIndicator`), drawn elsewhere.
- **A composed render** gets the props (reactive), a way to emit the widget's events, and the app's accessible label, which it puts on the control that stands for the widget. It builds as a plain container, so its built-in widgets carry the semantics.
- **Compile-time coverage:** using a widget requires `Render`, and `Render` has to name a render that exists on the platform being built: a `NativeRender` impl or a `Drawn` one. A missing render doesn't build.
- **Transport:** commands carry `WidgetKind::Custom(NAME)` and `Prop::Custom(CustomProps)`: the props as an `AnyValue` (type-erased, but still compared and printed with their own `PartialEq` and `Debug`) plus the widget's definition (semantics, action mapping, renders). Events come back as `UiEvent::Custom(AnyValue)`. Props don't need to be serializable. The native render travels with the props, so backends keep no registry.
- **Semantics** come from `CustomWidget::a11y`; app overrides (`.a11y_label(…)`) win. Accessibility actions go to the native render first; if it doesn't handle one, the core emits the event `CustomWidget::action` maps it to. Drawn and native renders behave the same for assistive technology and tests.
- **The drawn tier runs in the core.** The core measures drawn widgets itself, draws them after layout (on new props, a new size or new metrics) and sends the result as `Prop::Drawing(DisplayList)` with the frames. Backends only rasterize display lists and report `UiEvent::Pointer`, which the core turns into widget events with `Drawn::pointer`. Headless wireframes draw them too.
- **Headless** lays out natively rendered widgets with their drawn render (hence `.with_drawn()` above), or as empty boxes without one.
- **Controlled:** a render emits an event when the user changes the view; the app answers with new props, and `update` shows them. If the app ignores the event, the view shows something the core doesn't know about, and the mirror check (via `read`) reports it.

### 6.4 Raw settings of a built-in widget

Semantic props cover what every platform has. For what only one platform has, a built-in widget takes a `Tweak`: a closure over the native control itself, made by the backend's `tweak`, picked per platform with `platform!`.

```rust
Button::new("Continue").role(ButtonRole::Default).native(platform! {
    macos => appkit::tweak(|b: &NSButton| b.setControlSize(NSControlSize::Large)),
    gtk => gtk::tweak(|b: &gtk::Button| b.add_css_class("circular")),
    kde => kirigami::tweak(|b: &QmlObject| b.set_bool("checkable", true)),
    windows => winui::tweak(|b: &Button| b.cast::<IControl>()?.SetCornerRadius(round)),
})
```

- It travels as `Prop::Tweak` (an `Opaque`), and runs after the widget's other props, and again whenever one changes, so what it sets wins. Tweaks should be idempotent.
- `tweak_with(value, |b, v| …)` runs again when the value changes; only the tweak is sent again.
- A tweak can make the native control disagree with the core's props (an icon for a label, say); the mirror check then fails in tests. Set what the semantic props don't.
- `_ => Tweak::none()` leaves the other platforms alone. Headless tests keep the tweak but don't run it.
- The widget's type names the native one (`Tweakable`): `Button` is `NSButton`, `gtk::Button`, a `QQC2.Button` item (`QmlObject`, set by property name) and XAML's `Button`, whose closure returns a `windows_core::Result`; `Checkbox` is `NSButton`, `gtk::CheckButton`, a `QQC2.CheckBox` item and XAML's `CheckBox`; `Switch` is `NSSwitch`, `gtk::Switch`, a `QQC2.Switch` item and XAML's `ToggleSwitch`; `Select` is `NSPopUpButton`, `gtk::DropDown`, a `QQC2.ComboBox` item and XAML's `ComboBox`; `Slider` is `NSSlider`, `gtk::Scale`, a `QQC2.Slider` item and XAML's `Slider`; `Progress` is `NSProgressIndicator`, `gtk::ProgressBar`, a `QQC2.ProgressBar` item and XAML's `ProgressBar`; `Spinner` is `NSProgressIndicator`, `gtk::Spinner`, a `QQC2.BusyIndicator` item and XAML's `ProgressRing`; `Separator` is `NSBox`, `gtk::Separator`, a `Kirigami.Separator` item and XAML's `Border` (WinUI has no separator control); `TextInput` is `NSTextField`, `gtk::Entry`, a `QQC2.TextField` item and XAML's `TextBox`; `PasswordInput` is `NSSecureTextField`, `gtk::PasswordEntry`, a `Kirigami.PasswordField` item and XAML's `PasswordBox`; `ScrollView` is `NSScrollView`, `gtk::ScrolledWindow`, a `QQC2.ScrollView` item (its `contentItem` is the `Flickable`) and XAML's `ScrollViewer`; `List` (its type's defaults make `Tweak<List>` name it) is the list view inside the scroll view: `NSTableView`, `gtk::ListView`, the QML `ListView` item and XAML's `ListView`; `Text` is `NSTextField`, `gtk::Label`, a `QQC2.Label` item and XAML's `TextBlock`.

## 7. Accessibility and i18n affordances (designed in now, implemented later)

- Every node carries `A11yProps`: role, label, description, value/range, state flags (disabled, checked, expanded, selected, busy), `labelled_by`/`described_by` relations, live-region politeness, and supported actions.
- Built-in widgets derive defaults: a `Button`'s label comes from its text, and so on. Apps override with `.a11y_label("…")`, and so on.
- Most of the work comes for free because the controls are **native**: NSAccessibility, UIA and GtkAccessible already understand native controls. `SetA11y` mainly carries overrides and relations.
- Drawn and custom widgets are where real work is needed. The plan is to implement native a11y protocols per backend. [AccessKit](https://github.com/AccessKit/accesskit) is an option for drawn subtrees, because it provides the same semantic model on all three platforms.
- Core owns **focus order**. The Tab order is reading (tree) order, so it's correct in right-to-left layouts and for absolutely positioned controls. `.tab_index(n)` moves controls ahead. Backends receive it as `SetFocusOrder` and chain native focus accordingly (AppKit: `nextKeyView`). Which controls can take focus stays a platform decision; on macOS, for example, it depends on the Keyboard navigation setting.
- `PlatformMetrics` exposes reduced motion, high contrast, text scale and color scheme as signals.
- **RTL:** styles use logical edges (`padding_inline_start`, not `padding_left`). Core mirrors frames for RTL locales, since Taffy doesn't.
- Text is never baked into images. All strings go through props, so they can be localised.

---

## 8. App logic and state ("backend behaviour is the same")

- Domain logic is **plain Rust**: no mitsuami dependency, `Send` where useful, async-friendly, and testable through its own public API.
- **Stores** (Pinia-like) are the UI-thread adapter. They own signals and expose actions that call into domain logic. A store is a plain `Clone` struct implementing `Store` (`fn create() -> Self`); `use_store::<S>()` returns the app's one instance, created on first use in a scope that lives as long as the app (`App` and `TestApp` call `provide_stores()` in their app scope), so a store outlives the views that use it, and its resources and tasks keep running.
- **Async** uses plain futures: `spawn_local`, `spawn_blocking` and `sleep`, plus `alert`, `open_file` and `save_file` for dialogs.
- **Resources and actions** wrap these as signals, all `Copy`:
  - `resource(fetch)` / `resource_on(source, fetch)` load a `Result<T, E>`: `loading()`, `data()`, `error()`, `refetch()`, `set_data()`. They fetch when created, and `resource_on` again whenever the signals `source` reads change. A new fetch cancels the one in flight. The last data stays available while reloading and after an error, so views can show both.
  - `action(fn)` runs an async operation on `dispatch(input)`: `pending()` while any dispatch runs, and `value()` of the latest dispatch (an earlier one finishing later doesn't overwrite it).
  - Both are owned by the scope that created them: disposing it cancels what's in flight.
- `provide` / `inject` replaces globals, so screens can be tested with mock stores: a store `provide`d in a scope takes precedence over the app's instance there.

Every per-platform screen (§6.1) consumes the same stores and composables. That is the reuse boundary.

---

## 9. API flavour (Vue → Rust mapping)

| Vue | mitsuami |
|---|---|
| `ref(x)` / `reactive` | `signal(x)` (`ref` is a Rust keyword). `Signal<T>` is `Copy` (arena-backed) |
| `computed` | `computed(move || …)` |
| `watch` / `watchEffect` | `watch(source, cb)` / `effect(move || …)` |
| props | typed parameters: `#[component] fn Foo(label: String, #[prop(default)] n: i32)`; `Value<T>` for reactive ones |
| `emit('x')` | typed callback props: `on_change: Callback<f32>`, set with `@change=…`, fired with `on_change.call(v)` |
| `v-model` | `bind=signal` (two-way) |
| `v-if` / `v-show` | `<Show when=… fallback=…>` / `hidden=…` |
| `v-for` + `:key` | `<For each=… key=… let:item>` |
| slots / named slots | `children: Slot` / named slots later |
| `provide` / `inject` | `provide(ctx)` / `inject::<T>()` |
| `onMounted` / `onUnmounted` | `on_mounted` / `on_cleanup` |
| template refs | `let r = node_ref(); <TextInput node_ref=r/>`, then `r.focus()` |

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
- **Children** are passed in a closure, which is how `Show` rebuilds a branch and `For` renders a row (`let:item` names the row's parameter). Other tags call it right away, so their children are built like the builder API's and siblings can share variables; only `Show`, `For` and `Window` (whose content is built at each opening) get a `move` closure, which owns what it uses. One child is passed as is, which gives `Text` its text and `Button` its label; several become a tuple.
- **What a constructor takes is an attribute too.** `Select::new("Plan")` is `<Select label="Plan">`: `label` sets the same `Prop::Label` the platform labels its control with (`a11y_label` only names the core's node, so an AppKit number field and its stepper would go unlabelled). `ScrollView::horizontal()` is `<ScrollView axes=ScrollAxes::Horizontal>`. A builder that takes two arguments has a one-argument form for attributes: `range_with=(0.0, 100.0)`. Every example is written with `view!`, and `tests/macros.rs` checks that these tags build what their constructors build.
- **Required attributes are types.** `<Show>` is `ShowWithoutWhen` until `when` is set, and only a `Show` has children. `<For>` wants `each`, then `key`. `<Window>` is a `Window<()>` until `title` is set, in any order among its attributes, since each builder method keeps the title's type.
- **Custom widgets are tags too:** `<Rating props=move || RatingProps { … } a11y_label="Rating" @event=move |e| …/>`. The tag is a `Custom<Rating, ()>` until `props` is set, and only then a `View`.
- **Mistakes are compile errors** that point at the tag or attribute: an unknown attribute is an unknown method (with rustc's "did you mean"), and a tag missing a required attribute isn't a `View`; the error's note says so.
- **`#[component]`** turns the function into a builder struct with one method per parameter, usable without `view!` (`Counter::new().initial(3)`). Each required prop is a type parameter, `()` until set, and the builder is a `View` only once all are set, so a missing prop is a compile error. `#[prop(default)]`, `#[prop(default = expr)]`, `Option<T>`, `Callback<T>` and `children: Slot` are optional; `#[prop(into)]` accepts `impl Into<T>`, and `Value<T>` accepts a literal, a signal or a closure.
- **A component runs once, when it's built, in a scope of its own.** What it `provide`s reaches its children but not its siblings, and its scope is disposed with it.

---

## 10. Built-in widget set

| Widget | AppKit | WinUI 3 | GTK 4 | Kirigami |
|---|---|---|---|---|
| Window | NSWindow | Window | gtk::Window + HeaderBar | Kirigami.ApplicationWindow + Page |
| Container / Row / Column / Grid | flipped NSView host | Canvas host | custom Widget host | Item host |
| Text | NSTextField (label) | TextBlock | gtk::Label | QQC2.Label |
| Button | NSButton | Button | gtk::Button | QQC2.Button |
| TextInput | NSTextField | TextBox | gtk::Entry | QQC2.TextField |
| PasswordInput | NSSecureTextField | PasswordBox | gtk::PasswordEntry | Kirigami.PasswordField |
| Checkbox | NSButton (checkbox) | CheckBox | gtk::CheckButton | QQC2.CheckBox |
| Switch | NSSwitch | ToggleSwitch | gtk::Switch | QQC2.Switch |
| Slider | NSSlider | Slider | gtk::Scale | QQC2.Slider |
| NumberInput | NSTextField + NSStepper | NumberBox | gtk::SpinButton | QQC2.SpinBox |
| Select | NSPopUpButton | ComboBox | gtk::DropDown | QQC2.ComboBox |
| Progress | NSProgressIndicator (bar) | ProgressBar | gtk::ProgressBar | QQC2.ProgressBar |
| Spinner | NSProgressIndicator (spinning) | ProgressRing | gtk::Spinner | QQC2.BusyIndicator |
| Separator | NSBox (separator) | Border in the divider brush | gtk::Separator | Kirigami.Separator |
| Image | NSImageView | Image | gtk::Picture | QML Image |
| GpuSurface | NSView backed by a CAMetalLayer | child HWND | Wayland subsurface or X11 child window | Wayland subsurface or X11 child window |
| ScrollView | NSScrollView | ScrollViewer | gtk::ScrolledWindow | QQC2.ScrollView |
| List (virtualised) | NSTableView | ListView | gtk::ListView | ListView |
| Sidebar | source-list NSTableView in an NSSplitViewController | NavigationView | gtk::ListBox in an adw::NavigationSplitView | Kirigami.ScrollablePage in the page row |
| Tabs | NSTabView | SelectorBar over the pages | AdwInlineViewSwitcher over an AdwViewStack (gtk::Notebook before libadwaita 1.7) | QQC2.TabBar over the pages |

**What comes next is driven by 2ksbox.** mitsuami was started to replace the Qt Quick launcher of 2ksbox (a Windows 98/XP emulator; `launcher-qt/qml` in that repo). Widgets are added as that launcher needs them, and only widgets every platform has a native control for: what one platform lacks is the app's to build, as a custom widget (§6.3). From the launcher so far:

- Built: `NumberInput` (its `SpinBox`), `Image` (its shader preview), tooltips (`.tooltip(text)` on any widget, for its elided status line).
- Built, though not widgets: windows opened while the app runs, modal or not (`Window`, §16; every secondary window of the launcher is an application-modal dialog over it); Escape closing its dialogs (the launcher's `Shortcut`s), as a modal `Window` does (§16); its header, whose download progress and status line are items of the window's `Toolbar` (§16); menus, which 2ksbox plans to add: submenus, check and radio items, reactive titles and items, roles, and a window's own menus in `view!` (§16); context menus on any widget (`.context_menu(…)`, §16).
- Left to the app: a separator line (WinUI has no separator control outside menus and app bars) and a disclosure header (Qt Quick has none; 2ksbox builds its own from a `ToolButton`).
- Built, for 2ksbox's player (its emulated machine's picture): `GpuSurface`, a widget the app presents to with its own GPU API (wgpu, Vulkan, Metal, Direct3D) at its own pace, off mitsuami's loop, through `raw-window-handle` handles, told of its size and scale (§16). 2ksbox's spike (`spikes/player-gtk` on its `track/player-gtk-spike` branch) measured two routes on GTK. `GtkGraphicsOffload` fed dma-bufs waits for GTK's frame clock, about one refresh more than a winit window (47.6 against 32.8 ms from a frame's publish to its presentation at 60 Hz). A desync `wl_subsurface` of the window's surface, placed over the widget, sized with `wp_viewport` and given an empty input region so GTK keeps the pointer, matched the winit window; that's the route taken on GTK and Kirigami.
- Built for the player too: the system's quit (macOS's Dock, logging out) going to the app, which asks first (§16, Menus); full screen, a minimum window size (no larger than the screen) and the app resizing its windows (§16, Full screen and window size), the cursor over the surface, the mouse's raw moves while locked, and keys held let go when the window stops being the active one (§16, GpuSurface). They replace what the player asked of winit (`set_fullscreen`, `set_cursor`, `set_min_inner_size`, `request_inner_size`, `Focused`, `DeviceEvent::MouseMotion`).
- Built for the player too: the surface's own input (keys by where they are on the keyboard, the pointer, buttons and scrolling), a pointer lock and a keyboard grab (§16), which none of the toolkits offers everywhere (GTK has `gdk_toplevel_inhibit_system_shortcuts`, and nothing for pointer lock), and X11 on GTK and Kirigami. One lesson for any app that runs a GLib-based library's main loop on another thread in its process (2ksbox runs QEMU's): GTK, and Qt's GLib event dispatcher, run on GLib's global default `GMainContext`, so that library needs a GLib of its own, or its loop dispatches the toolkit's sources on its thread.

**Idiomatic shell components (post-MVP).** These are where most of the "feels native" effect comes from:

- `AppShell`; `Sidebar` (source list / NavigationView / split view): built (§16)
- `Toolbar` (NSToolbar / CommandBar / HeaderBar): items at its trailing end are built (§16)
- `MenuBar` (the global menu on macOS; an in-window menu or hamburger elsewhere): built (§16)
- `Tabs` (a view switcher: a settings window's panes): built (§16). Document tabs (closable, reorderable) are another widget: native window tabs on macOS, `AdwTabView`, WinUI's `TabView`
- `Preferences`

---

## 11. Platform versions and capabilities

We don't pick one fixed OS version per platform. Instead:

- Each backend has a **hard floor**: the oldest version the backend itself can run on.
- Everything above the floor is a **capability**. Capabilities can be queried, and built-in widgets degrade gracefully when one is missing.

### Floors

| Platform | Floor | Why |
|---|---|---|
| Windows | Whatever Windows App SDK 2.4 / WinUI 3 supports (historically 10 1809, build 17763; to be confirmed for 2.4) | Pinned to the SDK version that `windows-reactor` targets. |
| macOS | macOS 11 | The practical floor for arm64 and current Rust targets. Everything newer is a capability. |
| Linux | GTK 4.10, or GTK 4.8 in reduced mode | Chosen at build time through a cargo feature (`gtk_v4_8`, `gtk_v4_10`, `gtk_v4_12`, …). libadwaita 1.4 is required (the sidebar's navigation split view); 1.7's inline view switcher is looked up at run time (`Tabs`). |
| Linux (KDE Plasma) | Qt 6.5 and Kirigami 6 | The backend's build asks for Qt 6.5 (the first Qt 6 LTS with what it uses); a given Kirigami release may need a newer Qt. Developed on Qt 6.11 and Kirigami 6.30. Kirigami's features are resolved in QML at run time, not at build time. |

### Capability model

```rust
#[non_exhaustive]
pub enum Capability {
    // cross-platform semantics, answered by every backend
    NativeSwitch, SymbolIcons, NativeFileDialog, SystemAccentColor,
    WindowBackdropMaterial,        // Mica on Windows 11, vibrancy/glass on macOS, none on GTK
    SplitViewSidebar, SearchField, DatePicker, …
    // platform-specific ones live in each backend's own enum
}

caps().has(Capability::WindowBackdropMaterial)   // plain bool, fixed for the process lifetime
```

How capabilities are detected differs per platform, and the design follows that:

- **macOS: detected at runtime.** Objective-C is dynamic, so the backend checks the OS version, `respondsToSelector:` and class existence at startup. A single binary adapts to whatever macOS it runs on.
- **Windows: fixed at build time, plus the OS build.** The WinAppSDK version is bundled with the app, so its features are fixed. OS-dependent features, like Mica needing Windows 11, are checked at runtime from the build number.
- **Linux: fixed at build time.** gtk4-rs links newer symbols directly, so a binary built with `gtk_v4_12` won't even load on GTK 4.10. The capability set is therefore whatever the build feature enables. Distro packages pick the feature that matches their GTK. Runtime `dlsym` probing is possible later, but it isn't worth it for the MVP.

### How capabilities are used

- **Built-in widgets degrade automatically.** For example, `Switch` falls back to a checkbox-style toggle, and `SymbolIcon` falls back to a bundled icon. Each fallback is documented on the widget. App code only branches when it wants different behaviour.
- **Apps declare hard requirements:** `App::new().require(Capability::X)`. Startup then fails with a clear, native error dialog instead of breaking halfway.
- **Views can branch on capabilities:** `platform!` arms accept capability predicates, e.g. `macos if has(WindowBackdropMaterial) => …`. There is also a runtime `<Supports cap=… fallback=…>` component.
- **Custom widgets** (`NativeRender`) can query `cx.caps()` to choose an implementation.

This also settles "runtime vs compile-time platform checks": the platform is compile-time (`platform!`), and capabilities are runtime (fixed per process).

---

## 12. Testing

Testing is a first-class feature, both for mitsuami itself and for apps built with it.

### Policy

- **No unit tests.** There are no `#[cfg(test)] mod tests` blocks and no tests of private functions. Every test goes through a public API.
- There are two kinds of test:
  - **Integration tests:** public API, headless backend, in `tests/` directories. Fast and deterministic; they run on every platform, in CI and locally.
  - **End-to-end tests:** a real native backend in a real window, driven the way a user would drive it.
- **Visual regression tests** (Chromatic-style) are a third track built on the same infrastructure.
- The tooling we build for ourselves is the tooling we ship. `mitsuami-test` is a public crate for app authors, and mitsuami's own suite is its first user.

### One test API, two backends

The same test runs headless or on the real native backend. Only the runner flag changes.

```rust
use mitsuami_test::prelude::*;

#[mitsuami_test::test]
async fn increments(app: TestApp) {
    app.mount(|| Counter(0));
    app.get_by_role(Role::Button, "Increment").click().await;
    app.expect(by_text("Count: 1")).to_be_visible().await;
    app.expect(by_role(Role::Button, "Increment")).to_have_frame_within(app.window());
}
```

```
cargo test                      # headless (default): integration tier
MITSUAMI_NATIVE=1 cargo test    # same tests on AppKit / WinUI / GTK: e2e tier
                                # (or `-- --native` for a single harness=false target)
cargo mitsuami visual           # visual regression tier (below)
```

Design points:

- **Queries go through the accessibility tree**, like Testing Library: `get_by_role`, `get_by_label`, `get_by_text`, `get_by_test_id` as a last resort. If a test can't find a control by role and label, a screen-reader user can't either. The a11y model does real work from day one.
- **Interactions are a11y actions:** `click` → `A11yAction::Activate`, plus `set_value`, `focus`, `scroll_into_view`, `expand`, `type_text`. The backend carries these out on the actual native control:
  - AppKit: `performClick:`, `accessibilityPerformPress`
  - WinUI: UIA patterns (Invoke, Value, Toggle)
  - GTK: `gtk_widget_activate`, `GtkAccessible`
  
  The same vocabulary later powers screen-reader support.
- **Raw input** (`app.keyboard()`, `app.pointer()`) is also available, for shortcuts, drag and drop, and custom widgets. It is synthesised at the native event level where the platform allows it.
- **No sleeps, no flakiness.** We own the scheduler, so `await` on an action or assertion means "run until the UI is settled": effects flushed, layout done, the native commit applied, and pending tasks idle. On top of that there is Playwright-style auto-retrying of assertions for async work, with a timeout.
- **Deterministic environment:**
  - a controllable fake clock (`app.clock().advance(500.ms())`)
  - a fixed locale, text scale and color scheme per test
  - animations off by default
  - `TestServices` replaces `PlatformServices`: dialogs, file pickers, clipboard and notifications are recorded, and their responses are scripted.
- **Dependency injection:** tests `provide` fake stores or services before mounting, the same `provide`/`inject` apps already use.
- **Native desync checks.** `app.native_state(node)` reads back what the native widget actually shows: text, checked state, enabled state, frame. In e2e mode, assertions compare core state with native state. This catches backend bugs that headless tests can't.
- **Snapshots:**
  - Tree, layout frames, a11y tree and command log snapshots (insta-style `.snap` files).
  - A **wireframe render**: an SVG of the headless layout (boxes, labels, roles). It is deterministic, platform-independent and diffable in review, so it's a cheap first line of visual testing.

### The main-thread problem

Native UI must run on the process main thread; on macOS this is strictly enforced. libtest runs tests on worker threads. So:

- Test targets use `harness = false`, with our runner: `mitsuami_test::main!()`, generated by `#[mitsuami_test::test]` through inventory-style registration.
- The runner owns the main thread and the native event loop, and runs test futures on it.
- In native mode, tests run serially in one process with a fresh window each (a fast, window-per-test mode). `--isolate` gives process-per-test when needed.
- CLI filtering and output stay compatible with libtest conventions, so `cargo test name_filter` and IDE runners keep working.

### Visual regression (Chromatic-style)

**Stories.** A story renders a component in a specific state. Stories double as a component gallery during development.

```rust
#[mitsuami_test::story(sizes = [(320, 200)], variants = [Light, Dark])]
fn counter_big_number() -> impl View { Counter(42) }
```

Stories can also be interaction states. `#[story(play = ...)]` runs a test script first (e.g. focus a field, type text) and then captures the result.

Each size and variant runs as its own test, named `<story>@<width>x<height>-<variant>` (`signup_ready@280xfit-dark`), in a window opened at that size with the variant's appearance forced. A height of `fit` fits the content at its first layout (`WindowSize::FitHeight`): control heights differ per platform, so a fixed height that suits one clips or pads another. Its baseline is `tests/visual/<backend>/<image>/<story>@<width>x<height>-<variant>.png`. `sizes` defaults to the test window's 800×600, and `variants` to `[Light, Dark]`. Headless has no pixels, so there a story only checks that the view mounts and the script plays. `crates/mitsuami/tests/stories.rs` has the built-in widgets as stories.

**Capture.** Capture happens in-process and offscreen, so no screen-recording permissions are needed and nothing depends on window placement:

- AppKit: `cacheDisplayInRect:toBitmapImageRep:`
- WinUI: `RenderTargetBitmap`
- GTK: `WidgetPaintable` → `GskRenderer::render_texture`

This is exposed as `Backend::capture`.

**Matrix.** Each capture is taken for every combination of platform, scale factor (1×, 2×), light/dark, and optionally high contrast and large text.

**Baselines.**
- Keyed by `platform / os-image / story / variant`. Native rendering differs between OS versions and display scales, so baselines belong to the machine image that recorded them: `tests/visual/<backend>/<image>/`, and the native tree, wireframe and command snapshots likewise in `tests/snapshots/<backend>/<image>/`. Headless snapshots don't depend on the machine and stay in `tests/snapshots/`.
- The image is `MITSUAMI_IMAGE`, which CI sets to its pinned runner image (`macos-26`, `ubuntu-24.04`, `windows-2025`), or for the KDE job to its Arch Linux container, pinned to a day of the Arch Linux Archive (`archlinux-2026.09.20`). Otherwise it's the OS and its version (`macos-26`, `ubuntu-24.04`, `windows-26100`). A scale other than 1× is appended (`macos-26@2x`), since captures are in physical pixels. Only CI's baselines are kept in the repository: a developer's machine records its own under its image, which git ignores, and compares only with those.
- Snapshot failures don't stop a test. It fails at the end with all of them, having written every missing or changed one next to its baseline (`.new`, `.new.png`, plus a `.diff.png`), on CI too. CI uploads them as a `snapshots-<image>` artifact, and `.github/scripts/accept-snapshots.sh <run id>` moves them over the baselines. That's also how a new image, or a new platform's first baselines, gets recorded.
- Stored in the repo as plain files for now (git LFS when volume demands it). A hosted store can be added later behind the same interface.

**Diffing.**
- Perceptual diff: pixelmatch's. Colours are compared by their distance in YIQ, and pixels that differ only by anti-aliasing don't count. `VisualOptions` (the story options of the same names) sets the `threshold` for how different a colour must look (0.1 by default), `max_changed` for the fraction of pixels that may change (0.1% by default), and regions to leave out: nodes found by a query, where they are at capture time (`ignore(by_test_id("clock"))`; a node whose content changes needs a fixed size, so the baseline's content is in the same region), or rects in window coordinates. The `.diff.png` shows changes in red, anti-aliasing in yellow and ignored regions in blue.
- A **layout diff** says *why* pixels moved: a layout change versus a native rendering change. Each baseline has the layout it was captured with next to it, `<name>.layout.txt` (the tree snapshot's format: every node's kind, props and frame), recorded and accepted with the PNG. When pixels change, the failure lists the nodes whose layout or props differ, or says the layout is the same and the platform draws it differently. A layout change alone doesn't fail: the pixels are what's compared, and the recorded layout follows along when updating.

**Review.**
- `cargo mitsuami visual review` (the `cargo-mitsuami` crate; in this repository an alias in `.cargo/config.toml` runs the workspace's copy, and apps `cargo install` it) finds every pending capture in the workspace (`<name>.new.png` under `tests/visual/`) and serves a review page on the loopback interface, behind a random token in its URL. Each change shows why its pixels changed (from the layouts), and its baseline and capture side by side, as a swipe, as an onion skin, or as the diff image. Accept moves the capture and its layout over the baseline, Reject deletes them. `--out <dir>` writes the same page as a static, read-only report instead. `cargo mitsuami visual accept [filter]` accepts without the page, and `cargo mitsuami visual` runs the native tests and says what's left to review.
- CI runs the story matrix on macOS, Windows and Linux runners (and KDE's container), and fails the build on unapproved diffs. Each failed job uploads the static report as a `visual-review-<image>` artifact, next to `snapshots-<image>`, and says in the run's summary how many captures it holds (`.github/scripts/collect-review.sh`).
- Later: a PR comment with a summary, and a small hosted review service (the real "Chromatic" part).

### What mitsuami tests about itself

- **The reactive runtime** is tested through its public API in `mitsuami-reactive/tests/`. It is exhaustively scenario-based: ownership and cleanup, batching, glitch-freedom, flush ordering.
- **Layout and units:** headless integration tests plus wireframe snapshots against a CSS reference. The expected frames come from what a browser computes for the equivalent HTML and CSS.
- **Backend conformance suite:** one shared suite that every backend must pass in `--native` mode. It covers each built-in widget's props, events, measurement sanity, a11y roles, and focus traversal. It is the executable definition of the backend contract.
- **Widget gallery stories** for every built-in widget, on every platform, form the visual baseline for the toolkit itself.

---

## 13. Risks, ranked

1. **WinUI 3 through `windows-rs` 0.100 / `windows-reactor`.** ✅ Retired by the M0.5 spike (§17).
   - Microsoft ships `windows-reactor`, a declarative WinUI 3 framework (60+ controls, targets Windows App SDK 2.4.0), and `windows-reactor-setup`, which stages the App Runtime for self-contained apps.
   - The problem was integration, not access. Reactor has its own component/state/effect system, we must not run two reactive systems on top of each other, and we need *imperative* access to the XAML elements.
   - The options were: (a) drive reactor's own elements imperatively; (b) generate the XAML bindings ourselves the way reactor does, and keep reactor only for bootstrap and staging; (c) host reactor components for leaf widgets.
   - **Chosen: (b).**
     - (a) is closed. Reactor's `ElementRef` only offers narrow requests (focus, swap chain, WebView2, child visual). It has no element handle and no `Measure`, and reactor's WinUI bindings are crate-private.
     - (b) works end to end with `windows-bindgen` 0.100. It needs no XAML subclassing: the `Application` is composed from Rust, and hosts are plain `Canvas`es.
     - The C++/WinRT shim fallback is no longer needed.
2. **Measurement fidelity.** Native intrinsic sizes can be quirky: NSTextField wrapping, and WinUI's Measure needing to be in the live tree. Headless tests can't catch this. The backend conformance suite and the visual gallery (§12) cover it.
3. **The own reactive runtime is on the critical path.** Everything depends on it: effect ownership and cleanup, batching, and flush ordering relative to layout. It needs a thorough scenario-based integration suite before anything is built on top of it.

---

## 14. MVP plan

| Milestone | Scope | Done when |
|---|---|---|
| **M0 — Core + test harness** ✅ | Workspace; `mitsuami-reactive`; node tree; styles and units; Taffy integration; `Command` protocol; headless backend; `mitsuami-test` basics (custom runner, a11y queries, actions, settle, tree/layout/wireframe snapshots) | Counter and a flex/grid form pass headless integration tests written with the public test API; the reactive suite passes |
| **M0.5 — WinUI spike** ✅ | Throwaway, using windows-rs 0.100: a window, a Canvas, a Button, Click and Measure, driven imperatively (`spikes/winui`) | Integration route (a/b/c) chosen: (b) |
| **M1 — AppKit** ✅ | Window, View hosts, Text, Button, TextInput, Checkbox, Switch; run-loop flush; measure; resize → relayout | The M0 tests pass with `--native` on macOS; conformance suite v1 passes; `Backend::capture` works and a first visual baseline exists |
| **M2 — GTK 4** ✅ | The same widget set (developed and tested on Linux, e.g. a VM or CI) | The same tests and conformance suite pass with `--native` on Linux |
| **M3 — WinUI 3** ✅ | The same widget set | The same tests and conformance suite pass with `--native` on Windows |
| **M4 — Escape hatches** ✅ | `platform!`, `NativeView`, `CustomWidget` + `NativeRender` (+ drawn and composed fallbacks) | Demo: one screen for every platform, with three custom widgets, each native where the platform has the control and built ad hoc, drawn or composed elsewhere |
| **M5 — Ergonomics** ✅ | `#[component]`, `view!`, stores, resources | Demo rewritten with macros |
| **KDE Plasma** ✅ | Qt Quick and Kirigami backend (`mitsuami-kirigami`, the `kde` feature), after a spike (`spikes/kirigami`) | The same tests and conformance suite pass with `--native` on Kirigami |
| **M6 — Visual review** | Stories, the variant matrix, perceptual diff, `cargo mitsuami visual review` HTML report, CI on three OSes | A PR that changes a widget shows up as a reviewable visual diff on all three platforms |
| **M7 — Lists** | A virtualised `List` on each platform's list control (§3): the platform realises rows and the core mounts those, keyed, and lays out what's in them; scrolling, selection, activation and keyboard navigation are native | A 10 000-row list scrolls, selects and filters on every backend, and the `lists` and `contacts` suites pass with `--native` |

Out of scope for the MVP: menus beyond a basic app menu, dialogs beyond an alert, the a11y implementation (the model exists), animations, and a devtools inspector.

---

## 15. Decision log

| Decision | Choice |
|---|---|
| Name | **mitsuami** (三つ編み, "three-strand braid": three native backends woven into one) |
| GTK flavour | gtk4, with libadwaita (1.4) for shell components: the sidebar's split view, the header bar, and from 1.7 the tab view's inline view switcher. `adw::init` gives the app libadwaita's style |
| Reactivity | Our own single-threaded runtime (`mitsuami-reactive`), not a reused one |
| `view!` syntax | JSX-like; the builder API remains the real API |
| OS support | Backend floors plus capabilities (§11); Windows floor = whatever Windows App SDK 2.4 supports |
| Layout ownership | Ours (Taffy); native widgets positioned absolutely |
| Toolkit per platform | Strictly native: AppKit / WinUI 3 / GTK 4, one per OS, no cross-toolkit dev builds. Linux has a second native toolkit, Qt Quick with Kirigami for KDE Plasma, picked at build time with the `kde` feature |
| KDE backend | Qt Quick Controls and Kirigami (what current KDE apps use), not Qt Widgets. Driven through a small C++ layer compiled with `cc` and `moc` (no `cxx-qt`, no CMake): each node is a QML item created from a line of QML, its props set by name |
| Windows bindings | windows-rs 0.100+ (`windows-reactor` ecosystem, WinAppSDK 2.4); MSRV 1.95, edition 2024 |
| WinUI integration | Route (b): our own `windows-bindgen` bindings (minimal mode, member filters) over the WinAppSDK metadata, driven imperatively. No `windows-reactor` at runtime. `windows-reactor-setup` stays an option for self-contained staging |
| Platform vs runtime checks | Platform is compile-time (`platform!`); capabilities are runtime |
| Testing | No unit tests. Integration (headless) + e2e (native) with one API; a11y-driven queries and actions; Chromatic-style visual regression; testing toolkit shipped to users |

## 16. Implementation notes

Things the AppKit backend taught us, some of them now part of the contract:

- **Native views start with a zero frame.** The core only sends frames that differ from the last one it sent. AppKit controls come with frames of their own, so the backend zeroes them on creation. The mirror check caught this for a `display: none` label.
- **Presses go through `accessibilityPerformPress`**, the path VoiceOver uses. For windows that aren't on screen, it returns `NO` even after pressing, so the result is ignored.
- **Typing goes through the field editor** (`insertText:`, `deleteBackward:`, `insertNewline:`), so the delegate and action paths are the real ones. Focusing a field selects all of its text, so the backend puts the caret at the end before typing, like clicking past the end would.
- **Tests run in offscreen windows** with the appearance forced (light, or the story's variant), for comparable captures (`MITSUAMI_SHOW_WINDOWS=1` shows them). The test app is never the active app, so its windows are never key or main, even with `MITSUAMI_SHOW_WINDOWS=1`. AppKit captures therefore show the **unfocused-window look**: default (Primary) buttons are grey instead of the accent color, and controls use their inactive colors. Baselines reflect that, so a grey Submit button in an AppKit baseline is expected, not a regression.
- **Code-built windows get no Tab order.** AppKit's automatic key view loop orders controls by position on screen, which is wrong for right-to-left layouts and absolute positioning. The core now sends the order (`SetFocusOrder`), and the backend links `nextKeyView` into a loop. The conformance tests use three controls on purpose: with two, wrap-around would make any order pass. They were confirmed to fail with AppKit's position-based order.
- **Reactive styles:** every style setter takes a literal, a signal or a closure, and `style_with` edits several fields reactively. `hidden` is a flag of its own, so un-hiding restores the node's `display` (a grid stays a grid).
- **Focus:** AppKit reports focus through KVO on `NSWindow.firstResponder`. While a text field is being edited, the first responder is the window's field editor, and its delegate isn't set yet when focus moves. The backend therefore walks up from the responder through its superviews to the nearest known view. The mirror check compares native focus with the core's on every settle.
- **ScrollView:** `NSScrollView` with our content view as its document view. Scroll changes are observed through the clip view's bounds-change notifications, including programmatic scrolls, so `Scrolled` fires for both. Window-coordinate frames, visibility (clipped by enclosing scroll views) and `scroll_into_view` are computed in the core, so all backends agree. As in CSS, a scroll view's natural size is its content's, so its siblings need `.shrink(0.0)` to keep their size.
- **Run loop:** `-[NSApplication stop:]` waits for an event, so stopping from an observer posts an empty application-defined event.
- **Services:** tests always use the scripted fake, even natively, so they never show real dialogs or touch the real clipboard. The real AppKit services have their own checks (`mitsuami-appkit/tests/services.rs`): a private pasteboard, the real `NSMenu` bar, an alert sheet answered by clicking, and a cancelled open panel.
- **`cargo test -- --native` also reaches libtest harnesses**, which reject the flag, so `MITSUAMI_NATIVE=1` is the workspace-wide switch.
- **Known gap: min-content text measurement.** Min-content currently falls back to max-content, so text never shrinks below one line inside flex rows. It still wraps under a definite width (columns, fixed widths).

### M4 (escape hatches)

- **Widgets are created with their initial props, reactive ones included.** Reactive props used to arrive as `SetProp` right after `Create`. That's harmless for built-in widgets, but a custom widget or native view can't be created without its props. The core now merges props set before a node's `Create` has gone out into that `Create`. Headless rejects a custom widget or native view created without its prop; the AppKit run caught the gap first.
- **Native views' payloads are one prop.** With a factory prop plus update props sharing a key, the first update replaced the factory in `Create`. Now one reactive payload holds the factory and the current value of every update.
- **Act on the accessibility element, not the view.** An `NSStepper` isn't an accessibility element; its only accessibility child (the cell) is, and only that one increments and sends the action. The backend walks down to it, as VoiceOver does.
- **Synthesized clicks** (`SyntheticInput::Click`) are for drawn widgets only. Native controls track the mouse in a loop of their own, waiting for real events; tests drive them with accessibility actions.
- The drawn render sizes its stars from the body font, so it sits close to the native rating control. Headless wireframes draw display lists, so drawn widgets show up in reviews without pixels.

### M7 on AppKit (lists)

- **A view-based `NSTableView`, one column and no header, in an `NSScrollView`.** Plain style (`NSTableViewStylePlain`), no intercell spacing, row heights from `tableView:heightOfRow:`: the hosts' heights, and for rows not shown yet the app's estimate, or else the first row measured. The estimate stays put once known (the table re-reads every height that once): the table keeps the heights it read, so a drifting estimate left rows above the view at stale heights.
- **Rows shown are the table's row views.** `tableView:didAddRowView:forRow:` and `didRemoveRowView:` report `RowShown` and `RowHidden`. Each cell is a plain view that takes the row's host when it arrives. The data source and delegate only read the list's own data (keys, heights, hosts, cells) and emit: the table calls them in the middle of `apply`.
- **Tables add row views in a layout pass,** at the next display, which offscreen windows never get. The backend lays its lists out at the end of each `apply` and in `settle`, so rows a change or a scroll reveals are reported and built in the same run-loop turn, before anything is drawn. Scrolling doesn't mark the table as needing layout, so it's marked first.
- **Data changes reload, then reselect by key.** The reload lays out right away, and only the difference in rows shown is reported: rows that stay keep their state. Height changes go through `noteHeightOfRowsWithIndexesChanged:` with animations off; a list scrolled to its end stays there when rows turn out taller than estimated.
- **Return activates the selected row** (a table subclass's `keyDown:`), as it opens the selected item in Finder and Mail; double-click is the table's `doubleAction`. Home and End only scroll, as in every AppKit list.
- **No automatic content insets.** `NSScrollView` insets its content for the title bar on its own (`automaticallyAdjustsContentInsets`), which showed once a fit-height window shrank: content drawn a title bar's height up while the offset still said 0. Scroll views and lists turn it off; the core places them.
- **Captures lay the window out first** (`layoutSubtreeIfNeeded`), for the same reason.
- **Known gap: captures show no row selection.** macOS 26's `NSTableRowView` sets its selection on its layer instead of drawing it, and `cacheDisplayInRect:` only runs views' drawing code. The selection itself is real (the tests check it natively); only baselines miss it.

### M7 on GTK (lists)

- **A `gtk::ListView` in a `ScrolledWindow`, over a `gio::ListStore` of row keys,** with `NoSelection`, `SingleSelection` or `MultiSelection` around it. A `SingleSelection` only takes `set_selected` (`set_selection` does nothing), and a splice that replaces items drops their selection even when the same keys come back, so the backend reselects by key after every data change.
- **Rows are the factory's binds.** Each item's child is a box that takes the row's host when it arrives and is as high as the estimate until then: empty cells would be 0px high, and GTK would bind every row. GTK binds and unbinds during layout, and unbinds and rebinds a row it keeps when the model changes, so the rows bound are compared with the rows reported once the main loop is idle, and only the difference is reported: rows that stay keep their state.
- **Data changes are one splice**, of the part between the rows that stayed at the start and at the end.
- **GTK prepares about 200 rows** (GtkListView keeps that many item widgets), and keeps its cursor row and selected rows bound wherever it scrolls. When rows are inserted above the view, it scrolls to keep the rows in view where they were.
- **Rows it keeps but doesn't place** (only the rows in view are allocated) are positioned from the nearest placed row and the heights of the rows between.
- **Tests allocate lists on settle.** List views bind and place rows when allocated, at the next frame, and on a Broadway display nobody watches frames stall after a paint (§ M2): waiting for a frame timed out. `settle` allocates each changed list's scrolled window again at its frame, as its host does, which lays the list view out synchronously.
- **No key events, no key signals:** GTK 4 can't inject keys and its list keyboard handling has no signals to emit, so synthesized arrows, Home, End and Enter do what it does: move the selection and show it, or activate. `list.scroll-to-item` scrolls to a row (`ListView::scroll_to` needs GTK 4.12). Row padding is reset with CSS (`listview.mitsuami-list > row`).

### M7 on WinUI (lists)

- **A `ListView` whose `Items` are the row keys, boxed strings,** so XAML's own collection takes the inserts and removes (one splice per data change, as on GTK). An item container style from markup takes the padding, margin and minimum height off `ListViewItem`s.
- **Rows are realised containers.** `ContainerContentChanging` fires when a container is realised for a row and when it goes to the recycle queue (a reused container fires for its old row, then its new one). Each container's content is a `Canvas` cell that takes the row's host, as high as it or the estimate. The rows realised are compared with the rows reported once the dispatcher is free, and at the end of each `apply`, after `UpdateLayout` realises what a change brought into view.
- **Selection** is `SelectedIndex` or `SelectedItems`; the selection set is recorded first, so the later `SelectionChanged` finds it reported. Double-clicks find their row up the visual tree to its container; Return activates the selected row (`PreviewKeyDown`). The list's scroll viewer is its template's, found in the visual tree.
- **Rows are placed from the scroll viewer's content,** not the view: XAML applies a scroll at its next layout, so right after one the view's transform plus the offset counted the scroll twice, and rows in view read as far below it.
- **A cell is never 0 high.** A row keeps its last measured height, or the estimate until it has one, including while a new host waits for its frame. Rows above the view that shrank to 0 and grew back made XAML shift the offset to keep the rows in view still, a little further each layout pass: a scroll to row 500 crept to the end of the list.
- **Focus is a row container's,** so the list has it when the window's focus tracking says so. `settle` resyncs that tracking from `FocusManager.GetFocusedElement(XamlRoot)`, walked up to the nearest node, as `GotFocus` does; it used to look for a node whose own control was focused, found none, and cleared the list's (and a `NumberBox`'s, whose focus is its text box's).
- **Run on WinUI:** the `lists` and `contacts` suites pass natively.

### M7 on Kirigami (lists)

- **A QML `ListView` over the row keys, with Qt Quick Controls' `ItemDelegate`s,** so the style draws the rows' highlight. Each delegate holds its row's host (`mitsuamiHost`) and is as high as it, or the estimate until it arrives. No C++: the view's QML keeps the selection, activation and keyboard handling, and Rust sets properties (`mitsuamiKeys`, `mitsuamiSelected`, `mitsuamiScrollTo`, …) and listens to three argument-less signals.
- **Rows are the delegates.** Delegates announce their creation and destruction with `Qt.callLater`, which coalesces them to once per event loop turn; Rust then compares the rows that have a delegate with the rows it reported. A data change resets the view and recreates its delegates, so without that a row would be hidden and shown again, and lose its state. The scroll position is put back after the reset.
- **The keyboard moves the current row, which is the selection** (`keyNavigationEnabled`), with Home, End and Return added in `Keys.onPressed`. Synthesized keys are real key events.
- **The delegates keep the style's padding and insets at the sides,** though rows don't use the padding: Breeze draws the highlight from both, and with zero padding it cut the highlight short at the right.
- **Multiple selection is built in the QML, as KDE's apps do theirs:** a QML `ListView` has no selection model, so the backend adds Qt's extended selection, which Dolphin and Qt's item views have: Ctrl-click toggles a row, Shift-click and Shift with the arrows select the rows from the anchor, and Ctrl+A selects them all. Rows are clicked through a `TapHandler`, because `clicked` doesn't say which modifiers were held. The tests can't hold modifiers, so this was checked with a `qmltestrunner` test over the list's QML.
- **`ListView`'s content starts at `originY`,** which moves as rows turn out taller or shorter than estimated: offsets and row positions are taken from it. At the end, a list stays there as rows are measured (`positionViewAtEnd()`, which also handles a scroll to the end over estimated rows).
- **List views place delegates when they polish,** before a frame: `settle` polishes the windows, so rows are where the view says when tests look.
- Qt keeps the heights of rows it has laid out; the estimate is the app's, or the first row measured.

### Select

- **One option is always chosen, as with HTML's `<select>`,** the first unless the app says otherwise; an index past the options chooses the first too. Only a `Select` without options has none. GTK forced this: `gtk::DropDown`'s selection autoselects and can't be cleared. `Prop::SelectedIndex` is `None` only when there are no options.
- **Sized as the platform sizes it:** `NSPopUpButton` for its widest item, `gtk::DropDown`, `ComboBox` and `QQC2.ComboBox` for the chosen one, so on those choosing can resize it (and relayout). Native wins: the backends don't even that out.
- **Replacing the options keeps the chosen index** where it can, else chooses the first, as the core does; the core only sends the index when that changes it. Every backend replaces its items in one step and puts the index back itself: `NSPopUpButton`'s `removeAllItems`, a `StringList` splice, `ItemCollection.Clear` and a new QML model all lose the selection.
- **Options with the same text stay apart.** AppKit adds `NSMenuItem`s to the menu (`addItemWithTitle:` drops earlier items with the same title); XAML's items are `ComboBoxItem`s rather than boxed strings.
- **Choosing is what the pop-up does** (`select_option`, `A11yAction::SetValue`): AppKit performs the menu item's action (`performActionForItemAtIndex:`), GTK and XAML set the index and report it, and Qt sets it and emits `activated`, the user's signal. Opening the pop-up would start a modal loop, so tests never do.
- **Tab reaches it where the platform says:** macOS skips pop-up buttons unless Full Keyboard Access is on, as it skips buttons. The core puts selects in the focus order.
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### Slider and Progress

- **A step means what the platform's step means.** AppKit's is tick marks that the knob only stops at (`allowsTickMarkValuesOnly`); WinUI snaps drags of the knob to `StepFrequency` (not values set through automation or by the app) and steps by `SmallChange`; Qt's sliders only move by it from the keyboard (`snapMode` stays off unless asked). GTK's scales have no stepped mode at all, so the backend snaps the user's moves to the step in `change-value` (see below). Without a step each keeps its default: WinUI's is 1, which it snaps to, and Qt's `increase()` moves by 0.1. GTK needs one to move at all, so it gets a tenth of the range. Tests only check which way a step moves.
- **The value follows the range.** The platforms clamp the value to the range, so a range sent after the value would lose it: the core queues the value again after every range change.
- **A clamp while the range is set isn't the user's.** XAML reports it from inside the range's setter: a new `Slider` starts at 0 of 0–100, so a range of 120–480 moved it to 120 and WinUI reported that as a move, overwriting the app's value (the text example's width). The backend now expects the clamped value before it sets the range, for `Slider` and `NumberBox` alike.
- **Sliders and progress bars have no natural width on AppKit** (no intrinsic width): they're as wide as the layout makes them, stretched in a column. The others measure theirs, but WinUI's slider measures only its thumb (18 wide): XAML apps stretch sliders or size them. Give a slider in a row `grow` or a width.
- **Indeterminate progress animates as the platform animates it:** `NSProgressIndicator` and XAML's and Qt's bars on their own, GTK's by `pulse()` calls, which a 100 ms timer makes while the bar is indeterminate, as GTK apps do.
- **Stepping and setting do what assistive technology or the keyboard does:** VoiceOver's increment, and the action as for a drag; GTK's step on the adjustment; UIA's RangeValue pattern; and on Qt the keys' `increase()` or `decrease()`, then `moved()`, the user's signal.
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### Button roles, styles and tweaks

- **`ButtonVariant` became a role and a style,** since what a button does and how it's drawn are separate choices (a borderless destructive button). Both are sent only if the app picks one, and backends keep them on the node: no toolkit reads every role back.
- **Cancel is Escape on AppKit only.** GTK, Qt and WinUI have no cancel button outside their dialogs, so it's a normal button there. Default is Return only on AppKit so far; GTK's default widget isn't set yet.
- **Qt's default button is `Accessible.defaultButton`,** which the desktop style turns into `QStyleOptionButton::DefaultButton`: Breeze tints it, except when flat. `highlighted` only draws the focus frame there, so it showed nothing. Screen readers hear the default button too. Qt Quick has no Return-clicks-default outside dialogs.
- **Default buttons draw their accent only in the key window** on AppKit, so test captures (never key) show them grey. A `bezelColor` tint didn't show in captures either, so the story's AppKit tweak is a large control size.
- **Tweaks run inside `apply`,** with events muted where the backend mutes them. `Tweakable` lives in each backend, which now depends on `mitsuami-widgets`.
- **Per-widget examples:** `examples/button.rs` shows every role in every style, a playground of the semantic props, and one tweak that differs per platform. Other widgets get one each as they're refactored.
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### Checkbox: the mixed state and tweaks

- **`Checkbox::mixed`** shows the mixed state over `checked` (some of what the box stands for is checked, as in "Select all"). Every platform has it: `NSControlStateValueMixed`, GTK's `inconsistent`, Qt's partly checked `checkState`, XAML's null `IsChecked`. The a11y tree has `mixed` beside `checked`.
- **A click leaves the mixed state, and lands where the platform lands:** AppKit checks the box (run); Qt's cycle from partly checked goes to checked; XAML's toggle from null goes to unchecked, as far as its toggle rule goes; GTK flips `active` underneath. The box reports what it landed on, the core absorbs `Mixed(false)`, and the app works out `mixed` again. Headless checks the box. Tests only check that the box leaves the mixed state and reports where it went.
- **Clicks never go back into it.** AppKit only allows the mixed state while it's shown, and Qt's `tristate` (which a partly checked `checkState` turns on) goes off after a click, so user clicks can't cycle into it. GTK leaves `inconsistent` to the app, so the backend clears it on a user toggle, as GTK apps do.
- **`Checked` is kept underneath while mixed:** AppKit and Qt keep it on the node (the control has one state), GTK's `active` holds it, and WinUI keeps it in `shown_checked`; leaving the mixed state always emits, whatever it lands on.
- **Tweaks as for buttons.** The example and story tweaks: the box after its label on AppKit (`imagePosition` trailing, captured), GTK's `selection-mode` class (a round check where the theme has one), Qt's `spacing`, a round box on WinUI (`CornerRadius`).
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### Switch: tweaks only

- **No semantic options past `checked`.** What the platforms offer isn't shared: control sizes on AppKit, GTK's `state` apart from `active` (for settings that take time to apply), on and off text on WinUI, a caption Qt's switch draws itself. So `Switch` only gets `.native(tweak)`, and the example's tweak is one of those per platform: a small control on AppKit (captured), a delayed state on GTK, the switch's own text on Qt, `OnContent`/`OffContent` on WinUI.
- **A tweak that connects a signal must guard itself,** since tweaks run again when props change: the GTK tweak marks the switch with a widget name.
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### Select: tweaks only

- **No semantic options past the options and the choice.** The nearest, a borderless select for toolbars, is AppKit's `bordered` and Qt's `flat` only: GTK's theme has no flat dropdown (GTK 4.14's `_common.scss` flattens dropdowns only inside `.toolbar`) and `GtkDropDown` has no API for one, and WinUI's `ComboBox` has no such style. Editable combo boxes are another widget on AppKit (`NSComboBox`) and missing from `GtkDropDown`. So `Select` only gets `.native(tweak)`.
- **The example's tweaks:** a borderless pop-up on AppKit (captured; it's measured narrower), search in the pop-up on GTK (`enable-search`), `flat` on Qt, a `Header` on WinUI (`IComboBox::put_Header` added to the bindings).
- **Tweaks run after the options too:** replacing them re-runs the tweak, like any prop change.
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### RadioGroup

- **Radio buttons, one for each option, down a column:** radio `NSButton`s in an `NSStackView` on AppKit, `gtk::CheckButton`s in one group in a `gtk::Box` on GTK, `RadioButtons` on WinUI, and `QQC2.RadioButton`s in a `ColumnLayout` on Qt. Its options and choice are a `Select`'s (`Prop::Options`, `Prop::SelectedIndex`, `Changed(Index)`); the label is its accessible name, not drawn, as a select's.
- **None chosen is allowed,** unlike a `Select`: every platform's radio buttons can all be off, as HTML's start, so the core enforces nothing. `RadioGroup::selected` and `bind` take an `Option<usize>` (`IntoValue` now takes an `Option` as a literal), and an index past the options chooses none. The user can't get back to none: a click on the chosen button changes nothing on every platform, which the tests check.
- **How far apart the buttons are is the platform's:** `NSStackView`'s own spacing on AppKit (8), 6 on GTK (as GNOME's dialogs space related controls), Kirigami's `smallSpacing` on Qt, `RadioButtons`' own on WinUI. The tests only check that each option adds height and the longest decides the width.
- **Each backend keeps the choice across new options** as the core does: the buttons already there keep their place on AppKit and GTK, with the new text; WinUI's items are replaced and the index set again; Qt's `Repeater` makes new buttons for a new model, so the backend reads the chosen one back first and sets it again.
  - AppKit: buttons with the same action in the same superview turn each other off, but only on a click, so the backend sets every button's state. A click on the chosen button sends its action again: the backend compares with the choice it knows.
  - GTK: `toggled` fires for programmatic sets (muted) and for the button turned off (ignored: only the one turned on reports).
  - Qt: `toggled` is the user's only; `perform` checks the button and emits it, as `Select` emits `activated`. The column hands its focus to the chosen button, or the first, so Tab reaches the group once. A layout sizes itself when polished, so `measure` polishes it first.
  - WinUI: `SelectionChanged` fires for programmatic sets too, guarded by `shown_index` as a select's is.
- **A radio group to assistive technology** (`Role::RadioGroup`), named by its label, holding a `Role::RadioButton` for each option, checked while it's chosen. The buttons are data, not nodes, as tabs are: they stand for the group, and the test kit's `click()` on one chooses its option (`A11yAction::SetValue`). Natively the group is `NSAccessibilityRadioGroupRole`, GTK's `RadioGroup` role, `RadioButtons`' own peer, and a `Grouping` on Qt, which has no radio group role. It's one stop in the Tab order.
- **No semantic options past the options and the choice.** Side by side isn't shared: WinUI's `RadioButtons` has `MaxColumns`, AppKit and GTK a horizontal stack or box, Qt a `RowLayout`. So `RadioGroup` only gets `.native(tweak)`, on the container: the stack view, the box, the `ColumnLayout`, the `RadioButtons`. The example's tweaks: horizontal on AppKit and GTK, no spacing on Qt, a `Header` on WinUI (`IRadioButtons::put_Header` added to the bindings).
- **Run headless and natively on GTK and Kirigami** (Arch Linux: GTK 4.22, libadwaita 1.9, Qt 6.11, Kirigami 6.30), `tests/radio_group.rs`, and the `radio_groups` story checked by eye on both. `examples/radio_group.rs` is for trying it by hand, and it's in the showcase. AppKit and WinUI are only type-checked. Unverified until they run:
  - AppKit: that `NSStackView`'s `fittingSize` is the group's size when placed by frame, that `performClick:` on the chosen button leaves it on, and focus with and without Full Keyboard Access;
  - WinUI: that `RadioButtons` reports `SelectionChanged` when its items are replaced, that `ContainerFromIndex` has a button to focus before the group is first laid out, and its measure;
  - how both look.

### Slider: orientation and tweaks

- **`Slider::orientation`** is the one semantic option every platform's slider has: AppKit `vertical`, GTK's `orientation`, Qt's `orientation`, XAML's `Orientation` (bindings added). Tick marks (not on Qt's), a drawn value (GTK only) and reversed direction (not on AppKit or Qt) aren't shared, so they're tweaks.
- **Up is more, everywhere.** AppKit, Qt and WinUI put the minimum at the bottom; GTK puts it at the top, and GTK apps set `inverted` to turn that round, so the backend does too.
- **Length comes from the layout.** A vertical slider is as tall as its container makes it (AppKit's have no natural height, as its horizontal ones have no natural width); headless measures one 20 × 160.
- **The example's tweaks:** a circular slider on AppKit (captured, sized at its natural size), `draw-value` on GTK, `snapMode` on Qt, tick marks on WinUI (`TickFrequency`, `TickPlacement` added to the bindings).
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### Slider: steps on GTK

- **A fair exception to "native always wins".** GTK's `gtk::Scale` has no stepped mode: the step is only a keyboard increment, and marks pull the knob in only within a few pixels. AppKit and WinUI stop on steps, so without this a `step(10.0)` slider on GTK reports whatever value the drag ends on. Where a platform lacks a behaviour the others share, and its apps build it themselves (GTK apps round in `change-value`), the backend does it the same way.
- **Only the user's moves snap.** `change-value` carries drags, clicks, scrolls and keys; values the app sets pass through as given. `SetValue` emits `change-value` too, as a drag would, so it snaps.
- **A mark at each step, by default,** as AppKit draws tick marks for a step: `gtk::Scale::add_mark` below (or beside) the trough, redrawn when the step or range changes. `mitsuami::gtk::show_step_marks(scale, false)` in a tweak turns them off; since tweaks run after every prop, the setting lives on the scale (its `Steps`, kept as object data) and marks are only redrawn when what they'd show changes. Marks make the scale taller, which it measures itself, and Adwaita draws the knob as a pin pointing at them rather than a circle (`slider-horz-scale-has-marks-below.png`), as in any GTK app with marks.
- **Marks only where steps are 24 px apart.** While dragging, GTK holds the knob on a mark until the pointer is 12 px away (`MARK_SNAP_LENGTH`), and snapping always leaves it on one. With steps closer than 24 px it holds past the next step, and the knob jumps several at once (the example's 57 px slider with ten steps jumped four). At 24 px the hold is at most half a step, which snapping does anyway. The length the knob travels is the frame less the scale's CSS padding (12 px each side on Adwaita, read with the deprecated `StyleContext::padding`, the only API for it). Whether marks fit depends on the length and changes the thickness, so `measure` decides them for the length it's asked about, and `SetFrame` for the final one.
- **Run on GTK and headless;** `stops_on_its_steps_where_the_platform_snaps` expects 80 for a move to 83 on GTK, where the backend snaps a screen reader's moves too, and 80 or 83 elsewhere (WinUI keeps 83: run); `gtk_marks_its_steps_unless_told_not_to` checks the marks by the scale's height, and that dense or short sliders have none, on GTK only.

### Progress: tweaks only

- **No semantic options past the value.** Orientation is GTK's only; paused and error states are WinUI's; a percentage label is GTK's. A circular style would be the one to share, but on GTK, Qt and WinUI a spinner is another control, and GTK and Qt have none that shows a value, so spinners get a widget of their own (`Spinner`, next) instead of a style.
- **Leaving the indeterminate state rebuilds AppKit's bar.** On macOS 26, `startAnimation` gives the bar a layer (AppKit's Swift `ProgressIndicatorLayer`) that keeps drawing the indeterminate animation after `stopAnimation`, whatever the value or `indeterminate` say; removing its animations or redrawing doesn't help. Setting the style to spinning and back rebuilds the layer, so the backend does that when a bar gets a value after having none. A capture test (`shows_its_value_after_being_indeterminate`) goes there and back twice.
- **The example's tweaks:** a small bar on AppKit (captured, measured thinner), `show-text` on GTK, `palette.highlight` on Qt (Qt 6's palette is an object; whether Breeze draws the bar with it isn't checked), `ShowPaused` on WinUI (bindings added).
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### Spinner

- **A widget of its own, not a style of `Progress`:** every platform has a spinner for work of unknown length, but on GTK, Qt and WinUI it's another control than the bar, and GTK's and Qt's show no value.
- **`running`, on by default.** Stopped, every platform's spinner shows nothing (AppKit's with `displayedWhenStopped` off, which the backend sets) and keeps its size, so nothing around it moves; apps hide it with `Show` to give the room back.
- **It reads as a progress bar without a value** (`Role::ProgressBar`), as ARIA has it and as GTK and WinUI report theirs; AppKit reports a busy indicator natively. Named by its label; takes no focus.
- **Sized as the platform sizes it:** AppKit's regular spinner is 32 pt, GTK's 16 px, Breeze's two grid units, WinUI's 16 (its style's minimum); headless measures 16 × 16. A `ProgressRing` has no size until XAML loads it and applies its template, at the frame after it's added: the backend asks the core to measure it again on `Loaded`, and waits for it in `settle`. Before, spinners on WinUI stayed 0 × 0 until something else moved them.
- **The example's tweaks:** a small spinner on AppKit (captured), a 32 px size request on GTK, a larger implicit size on Qt, a determinate ring on WinUI (`IsIndeterminate`, `Value` added to the bindings), which only WinUI's spinner can show.
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### Separator

- **Every platform has one but WinUI:** a separator `NSBox`, `gtk::Separator`, `Kirigami.Separator`. XAML's separators are for menus and command bars (`MenuFlyoutSeparator`, `AppBarSeparator`), so WinUI's is a `Border` in `DividerStrokeColorDefaultBrush`, 1 epx across (`MinHeight` or `MinWidth` in its style), as Fluent apps and the Settings app draw dividers. It has no automation peer, so UIA doesn't list it, as it doesn't list those apps' dividers.
- **`Orientation`, horizontal unless told otherwise,** always sent. GTK's separator has one and reads it back; AppKit's takes it from its frame's shape and Kirigami's has none (its frame is long one way), so those keep it on the node, as WinUI does.
- **As thick as the platform draws it; the layout gives its length.** It measures 0 along its length (1 on GTK and Kirigami, their minimum), so it spans a column that stretches its children, or a row when vertical, as the platforms' own separators span a box (`hexpand` on GTK, `Layout.fillWidth` on Kirigami). It isn't stretched when the app aligns the column's children otherwise; `align_self(Align::Stretch)` does. AppKit's is 1 pt unless its intrinsic size says more; headless measures 1. WinUI's 1 epx is 1.33 at 150 % (XAML rounds to whole pixels): the backend rounds it to the nearest epx rather than up, which drew it 2 epx thick.
- **It reads as a separator** (`Role::Separator`), with no name, as ARIA, GTK (`GTK_ACCESSIBLE_ROLE_SEPARATOR`) and Qt (`Accessible.Separator`) have it. It takes no focus and no actions.
- **The example's tweaks:** `transparent` on AppKit and the `spacer` class on GTK keep its room but draw nothing; Kirigami's light `weight`, as inside lists and cards. WinUI has none of its own.
- **Run on WinUI** (tests and a capture, checked by eye); AppKit, GTK and Kirigami are only type-checked, and CI hasn't run them. Whether AppKit's `NSBox` reports an intrinsic thickness, and Kirigami's `weight`, aren't checked.

### TextInput: read-only and tweaks

- **`TextInput::read_only`** is the one semantic option every platform's text field has: AppKit `editable` off (still `selectable`), GTK's `editable`, Qt's `readOnly`, XAML's `IsReadOnly`. The text can be selected and copied, and the app can still set it. The a11y tree gets `read_only`.
- **Focus is the platform's.** GTK, Qt and WinUI keep read-only fields in the Tab order; AppKit's refuse keyboard focus (`acceptsFirstResponder` is false) unless Full Keyboard Access is on, and take it from a click. The core's focus order still lists them: AppKit skips views that can't become key.
- **Nothing can be typed into one.** `synthesize` returns `ActionError::ReadOnly` for any key, before focusing, and `perform(SetValue)` too: AppKit can't deliver keys to a field it won't focus, and WinUI's backend edits through the selection, which `IsReadOnly` doesn't stop.
- **Where typing goes after the app sets the text is the platform's.** A field focused when the window opened keeps its caret where the set left it: GTK's `set_text` leaves it at the start, AppKit at the end. The tests accept either.
- **Tweaks for the rest.** A length limit is GTK's, Qt's and WinUI's (AppKit needs a formatter); icons in the field are GTK's; a header is WinUI's. Secure entry is another control on AppKit and WinUI (`NSSecureTextField`, `PasswordBox`), so it is a widget of its own.
- **The example's tweaks:** a borderless field on AppKit (captured; macOS 26 draws a rounded bezel as it draws the default one, so that tweak showed nothing), a search icon on GTK, `maximumLength` on Qt, a header on WinUI (`ITextBox.Header` added to the bindings).
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### PasswordInput

- **A widget of its own**, not a `TextInput` option: AppKit and WinUI make password fields from other controls (`NSSecureTextField`, `PasswordBox`), and GTK's `PasswordEntry` isn't a `gtk::Entry`. Qt uses `Kirigami.PasswordField`, KDE's own, a `QQC2.TextField` that echoes bullets. It has a text field's props and events: `Value`, `Placeholder`, `Enabled`, `Changed(Text)`, `Submit`. No read-only: `PasswordBox` has none.
- **Hidden as each platform hides it.** Bullets everywhere; a button that shows the text on Qt and WinUI, their default; GTK's peek icon is off, GTK's default. Showing it is up to the platform and the app's tweaks.
- **A text field to assistive technology**, as every platform exposes one (AppKit's secure subrole, Qt's and UIA's password flag): role `TextField`, named by its label or placeholder, with `password` set and no value. `native_state` still reports the text, for the mirror check.
- **WinUI edits at the end.** `PasswordBox` has no caret or selection API, so synthesized keys append to or trim `Password`, where typing into a focused box goes, and it has no settable Value pattern, so `SetValue` sets `Password`.
- **The example's tweaks:** no bullets on AppKit (`echosBullets` off on the cell; captured), the peek icon on GTK, `showPassword` on Qt, asterisks on WinUI (`PasswordChar`; `PasswordBox` bindings added).
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### ScrollView: scroll bars and tweaks

- **`ScrollView::scroll_bars`** is the one semantic option every platform's scroll view has: hide the bars and keep scrolling, by wheel, trackpad and touch, as a strip of photos does. AppKit turns its scrollers off, GTK's policy is `External`, Qt's scroll bar policy `AlwaysOff`, XAML's visibility `Hidden`. Shown, they're the platform's own, overlay or not. "Always shown" isn't shared: on macOS it's the user's setting.
- **Shift+wheel scrolls sideways on WinUI too.** AppKit, GTK and Qt do it, and so do Windows' own apps (Explorer, Edge), but XAML's `ScrollViewer` doesn't (microsoft-ui-xaml#8553, closed as not planned). The backend fills it in: a `PointerWheelChanged` handler on the scroll view's content, which sees the wheel before the scroll viewer, scrolls sideways by XAML's 48 px a notch when Shift is down and the content is wider than the view. The content gets a clear background, as a host with a tooltip does: a `Canvas` without one is only hit where its children are, and the wheel passed it by between them. Scrolls apply at XAML's next layout, so quick notches add up from where the last one sent the view. Not covered by tests: wheel input can't be synthesized here; to be tried by hand in the scroll_view example.
- **Controls in a new WinUI scroll view are measured once it's in the live tree.** A `ScrollViewer` shows its content only after a layout pass applies its template, and XAML measures only elements in the live tree, so controls in it measured as if untemplated: in the todos example, whose list scrolls, Remove buttons were 0 wide and rows 19 px tall. After each batch the backend lays out any scroll view that's live while its content isn't (`UpdateLayout`), which connects the content before the core measures it. Found and run on WinUI (the todos test's layout); the other backends weren't checked for the same.
- **AppKit keeps the axes on the node.** It read them back from its scrollers, which hidden bars turn off; synthesized scrolls use the node's axes too.
- **Tweaks for the rest.** Elasticity and borders are AppKit's, classic (non-overlay) scroll bars GTK's, the wheel's step Kirigami's, inertia and zoom WinUI's. A border narrows the visible area, which the core doesn't know about: content can lose a point or two at its edges.
- **The example's tweaks:** a bezel border on AppKit (captured), `overlay-scrolling` off on GTK, one line per wheel notch on Qt (`WheelHandler.verticalStepSize`; run on Kirigami), `IsScrollInertiaEnabled` off on WinUI (bindings added).
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### List: tweaks only

- **No semantic options past rows, selection and `ListStyle`** that every platform shares. Alternating row colours are AppKit's (GTK, Qt and WinUI have none built in); separators GTK's; single-click activation GTK's and WinUI's; wrapping key navigation Qt's; selection that doesn't follow focus WinUI's.
- **The selection mode can change while the list shows** (`List::selection_mode` takes a signal; the example has a switch). The selection keeps what the platform keeps, at most what the new mode holds, and the rows let go are reported as the user's, as for removed rows. XAML's `ListView` clears its selection on every mode change, even to one that holds more, and WinUI keeps that. AppKit's table keeps its selection when it stops allowing several rows, or any, so the backend keeps `selectedRow` (the row selected last) for Single and none for None. GTK has no mode, only selection models to swap: the backend carries over what the new model holds (the first row for Single), as AppKit does. The swap also takes the focused row out of the view while the window keeps it as its focus, so keys went nowhere; the backend gives focus back to the list, as AppKit's table keeps it. Removing the focused row does the same (GTK doesn't move the window's focus off the row's item widget, and doesn't report it), so focus goes back to the list there too; found by `context_menu.rs` deleting the row GTK had focused. Qt's selection is the QML view's own, and keeps the first row. Run on every backend and headless.
- **Tweaks reach the list view,** not the scroll view the node stands for: the table on AppKit, the `gtk::ListView`, the QML `ListView`, XAML's `ListView` (the node's element already). The scroll view is its parent (AppKit `enclosingScrollView`).
- **`Tweak<List>`:** `List`'s type parameters have defaults (`List<(), (), RowRender<()>>`), so the widget can be named without its item and key types.
- **The example's tweaks:** alternating rows on AppKit (captured), separators on GTK (a line per row, which GTK's rows add to their padding: row placement is GTK's, as before), `keyNavigationWraps` on Qt, `SingleSelectionFollowsFocus` off on WinUI (bindings added).
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### Text: a line limit and tweaks

- **`Text::max_lines`** is the one semantic option every platform's label has: at most so many lines, the last cut off with the platform's ellipsis (AppKit `maximumNumberOfLines`, GTK `lines` with `ellipsize`, Qt `maximumLineCount` with `elide`, XAML `MaxLines` with `TextTrimming`). The label is measured as limited, and still read out in full. 0 lifts the limit, as on AppKit and WinUI. Qt's eliding labels also drop the lines past their height, so Kirigami measures them with the height lifted: a label's frame is 0 high until its first layout, and measured as one line from it.
- **Not selectable text, yet.** AppKit, GTK and WinUI labels can be made selectable (`selectable`, `selectable`, `IsTextSelectionEnabled`), but Qt's `QQC2.Label` can't; KDE's `Kirigami.SelectableLabel` is another control, a `TextEdit`. It would be a prop that makes the Kirigami backend create a different item.
- **Tweaks for the rest.** Style classes, rich text, selection and letter spacing are each platform's own.
- **The example's tweaks:** selectable text on AppKit and GTK, Markdown (`textFormat`) on Qt, `CharacterSpacing` on WinUI (bindings added, with `MaxLines` and `TextTrimming`). The `text_tweaked` story still sets AppKit's secondary colour and GTK's `dim-label`, which `Text::color(Color::SecondaryLabel)` now does.
- **Run on AppKit, GTK, Kirigami and WinUI,** and checked by eye on each.

### Text: colour, weight, italics and alignment

Every platform's label has these, so they're semantic props (§4). Semantic colours stay the platform's own so they follow the appearance live; `Color` gains `Error`, `Warning` and `Success`, for drawn widgets too.

- **AppKit:** `textColor` from the catalogue colours (`labelColor`, `secondaryLabelColor`, `controlAccentColor`, `systemRedColor`, `systemOrangeColor`, `systemGreenColor`), `Rgba` a fixed sRGB colour. Weight rebuilds the style's font with `systemFontOfSize:weight:` (the monospaced one for `Monospace`), italics adds the italic trait; the font is rebuilt from style, weight and italics whenever one changes, so their order doesn't matter. `alignment` Left, Center, Right. Read back: the weight trait (nearest step), the italic trait, `alignment`, and `textColor` compared with the colour sent.
- **GTK:** colours are theme style classes where GTK has one (`dim-label`, `accent`, `error`, `warning`, `success`; `Label` is no class), so they follow light, dark, high contrast and the accent; the rest and `Rgba` are a Pango foreground attribute, the theme ones resolved once when set, so those don't follow a theme change. Weight and italics are Pango attributes, over the style's class. Alignment is `xalign` and `justify`, with the label set left-to-right, since GTK mirrors both in a right-to-left widget. Drawn colours use the theme's `error_color`, `warning_color` and `success_color`, with libadwaita's values as the fallback.
- **Kirigami:** colours are `Kirigami.Theme` bindings in the label's QML (`textColor`, `disabledTextColor` for the secondary label as Kirigami's subtitles use, `highlightColor`, `negativeTextColor`, `neutralTextColor`, `positiveTextColor`), chosen through properties of ours (`setProperty` can't make a binding), which `native_state` reads. Weight and italics are `font.weight` and `font.italic` bindings, apart from the style's family and size. `horizontalAlignment` Left, HCenter, Right: Qt mirrors an explicit one only under `LayoutMirroring`, which nothing enables.
- **WinUI:** colour is a `Foreground` setter in a style loaded with `XamlReader`, based on the text style's (`BasedOn`), so theme brushes follow the theme as Fluent's own styles do: `TextFillColorPrimaryBrush`, `TextFillColorSecondaryBrush`, `AccentTextFillColorPrimaryBrush`, `SystemFillColorCriticalBrush`, `CautionBrush`, `SuccessBrush`. A brush can't be told apart once resolved, so the colour is reported from the node. Weight (400 to 700) and italics are local values, which beat style setters, so a new text style keeps them. `TextAlignment` Left, Center, Right.
- **Measured with the font:** weight and italics change a label's size (`affects_measure`); alignment and colour don't.
- **Run on AppKit and headless** (`tests/text.rs`, the `text_options` story recorded on this machine). GTK, Kirigami and WinUI are only type-checked. To verify when they run: whether GTK's own theme (not libadwaita's) styles `.accent`, `.error`, `.warning` and `.success` labels (the mirror check reads the class, not the colour); Pango's automatic direction putting a right-to-left paragraph's left alignment on the right; Kirigami's colour and weight bindings (unrun QML), and that a label without a colour draws as the desktop style's does; WinUI's `SetBasedOn` on a freshly loaded style, and a coloured label without a text style replacing an implicit `TextBlock` style, if any. How the colours and weights look, in light and dark, is for the eye (`examples/text.rs` has a control for each).

### NumberInput

- **A spin box for a whole number, where every platform has one:** `NumberBox`, `gtk::SpinButton`, `QQC2.SpinBox`. AppKit has no single control, and its apps put an `NSStepper` beside a text field, so the backend does that (`mitsuami::appkit::NumberField`, a flipped view with both; the stepper holds the number, range and increment, and the field shows it). It came from 2ksbox's launcher (memory in MB, disk size in GB), and Separator and Disclosure, which it also uses, didn't qualify: WinUI has no standalone separator and Qt Quick no disclosure.
- **Whole numbers in an `i32`, because Qt's `SpinBox` holds an `int`:** a rule the core enforces since one platform makes the alternative impossible. Decimals typed or set are rounded: GTK with `digits` 0, Qt by its validator, WinUI in `ValueChanged` (`NumberBox` takes decimals; an emptied box, NaN, gets the last number back), AppKit when the field commits.
- **Typing reports when the edit is committed,** as every platform commits one: Return, or leaving the field (AppKit's cell `sendsActionOnEndEditing`). Buttons and arrow keys report at once. Text that isn't a number puts the number back.
- **What happens past an end is the platform's.** AppKit's stepper wraps round (`valueWraps` is on by default); GTK, Qt and WinUI stop. Numbers typed past an end are clamped everywhere. `stops_or_wraps_at_its_ends_as_the_platform_does` expects each.
- **Held buttons repeat where the platform's do** (`NSStepper.autorepeat`, GTK's and Qt's buttons, WinUI's `RepeatButton`s), reporting each step. AppKit's stepper is left as AppKit makes it: on macOS 26 it repeats only inside an `NSSplitView` (a window with a `Sidebar`), faster the harder a Force Touch trackpad is pressed (driven by pressure events: about 100 ms between steps pressing normally, 33 ms harder, 16 ms harder still), and elsewhere steps once per click, in a bare AppKit app too. `NumberField` used to fill the repeat in by having the cell send its action on periodic events too; inside a split view that stepped on every pressure event as well, twice as fast as the stepper, and the showcase (a sidebar window) made it show. Found with small AppKit apps logging each step, `+[NSEvent stopPeriodicEvents]`'s callers (`NSStepperDidReceivePressureEvent`) and pressure events, held by hand. Not tested: the test kit can't hold a native button down.
- **Inline spin buttons on WinUI.** `NumberBox` hides them by default; `Inline` is its documented spin-box mode, and a tweak can pick `Compact` or `Hidden`.
- **Sized as the platform sizes it:** GTK for its range's widest number, Qt for its text, WinUI by `Measure`, AppKit for the range's longest number in the field plus the stepper; so `Range` and `Number` re-measure a `NumberInput` (`Prop::affects_measure` now takes the kind). Headless measures one 96 wide.
- **A spin button to assistive technology** (`Role::SpinButton`), named by its label, with the number as its value. On AppKit the field and the stepper both get the label, as VoiceOver finds them separately.
- **Not tested: typing keys into one.** `synthesize` has no `NumberInput` path yet on any backend (each would drive the field inside); the suite uses assistive technology's `SetValue`, `Increment` and `Decrement`.
- **The example's tweaks:** `valueWraps` off on AppKit, `wrap` on GTK and Qt, `Compact` spin buttons on WinUI (`NumberBox` bindings added).
- **Run on every backend and headless,** and checked by eye on each.

### Image

- **A picture from a file or from pixels in memory,** in each platform's image view: `NSImageView`, `gtk::Picture`, XAML's `Image`, a QtQuick `Image` (`Kirigami.Icon` is for themed icons: `Icon`). 2ksbox's shader preview is pixels the app renders, so pixels aren't a detour through a file.
- **Pixels are straight RGBA8, sRGB, with a scale** (pixels to a point), so an app can render at the window's scale factor and have each pixel shown as one. Each backend makes its own image from them: an `NSBitmapImageRep` retagged sRGB, a `gdk::MemoryTexture`, a `WriteableBitmap` (premultiplied BGRA, converted), and on Qt a `QImage` served by a `QQuickImageProvider` registered as `mitsuami`, Qt's way to give QML images from memory. Every set of pixels gets its own `image://` URL there, and `cache` is off, so QML never shows a stale one. `Pixels` holds an `Arc`, so props clone cheaply, and prints its size, not its bytes.
- **A file is read when it's set.** AppKit, GTK and Qt (`asynchronous: false`) decode it right away; WinUI decodes in the background, so its `ImageOpened` and `ImageFailed` send the new `UiEvent::Remeasure` and the core measures it again. A file that changes under the same path isn't read again: set other pixels, or another path. A missing or unreadable file shows nothing and measures zero. Headless reads a PNG's size from its header, and gives other files no size.
- **Its natural size is the image's in points:** pixels over their scale, or the file's size as the platform reads it (AppKit honours a PNG's resolution; the others count pixels).
- **Only the fits every platform has:** `Contain` and `Stretch`. Aspect fill is missing from `NSImageView`, and "only shrink" from Qt and XAML, so those are tweaks. The fit is sent only if the app picks one, since the defaults differ: AppKit shrinks proportionally but never enlarges, GTK and XAML contain, Qt stretches.
- **Smoothing isn't shared** (Qt's `smooth` is the only switch), so pixel-sharp scaling is a tweak on Qt; elsewhere, pixels made at the window's scale aren't scaled at all.
- **An image to assistive technology** (`Role::Image`), named by its label (GTK's `alternative-text`, Qt's `Accessible.Graphic`); without one, decorative. It takes no focus. No platform gives an image's source back, so backends keep it on the node for the mirror check.
- **`draws_what_it_is_given` checks the pixels on screen:** a capture of the window, blue and red where the fixture has them, so a mirrored or swapped-channel image fails.
- **The example's tweaks:** a photo frame on AppKit (`imageFrameStyle`), `content-fit` cover on GTK, `smooth` off on Qt, `UniformToFill` on WinUI (`Image`, `Stretch`, `BitmapImage`, `WriteableBitmap`, `Uri` and `IBufferByteAccess` added to the bindings).
- **Run on every backend and headless,** and checked by eye on each. An unpackaged WinUI app loads a `BitmapImage` from an absolute `file:///` URI; paths with spaces or `#` are unverified. WinUI's `settle` waits for files being decoded (`ImageOpened` or `ImageFailed`), as it waits for spinners to load: `expect` only retries while the app has tasks, and XAML's decoding isn't one.

### Icon

- **An icon from the platform's own set, by its name there,** as the sidebar's icons are: an SF Symbol in an `NSImageView` (or else an image AppKit has by that name), a themed icon in a `gtk::Image` or a `Kirigami.Icon`, a Segoe Fluent Icons glyph in a `FontIcon`, as WinUI's `NavigationView` items show them. Names differ per platform, so the app picks them with `platform!`. A shared set of names mapped per platform could come later; the names stay each set's own, so an app can use any icon its platform has. An empty name shows nothing. A name the set lacks shows the way the platform shows one: nothing on AppKit, GTK's missing-image icon, Kirigami's fallback icon, the font's fallback glyph on WinUI.
- **Each platform's own size** unless the app gives `icon_size` in points. On AppKit that's the symbol's point size, as a font's: a symbol's shape sets its frame (at the default size, a disc is 15 × 15 and a trash can 15 × 17). Elsewhere it's the side of a square: GTK's 16 (`pixel_size`, whole pixels), Kirigami's `iconSizes.small`, 16 at the default scale (what KDE's buttons and menus show inline), and XAML's `FontIcon` `FontSize`, 20 by default. Tests compare sizes, never a number.
- **Drawn in the colour the platform gives icons,** which follows dark mode: AppKit's image view draws a lone symbol in a secondary grey, while GTK, Kirigami and XAML give symbolic icons the text colour.
- **`Icon::color` takes the colours `Text` takes** (`Prop::TextColor`): a semantic one is the platform's own, so it follows dark mode, high contrast and the accent without the core sending it again; `Rgba` is fixed. Symbolic icons take it (SF Symbols, `-symbolic` theme icons, Fluent glyphs), and icons in full colour keep theirs.
  - AppKit: the image view's `contentTintColor`.
  - GTK: CSS `color`, which symbolic icons are drawn in. Semantic colours use the labels' classes (`dim-label`, `accent`, `error`, …). A fixed colour gets a class of its own, with its rule in one style sheet for the display, since an image has no Pango attributes.
  - Kirigami: `color`, bound as a label's is, to `Kirigami.Theme`'s colours. Without one it's `transparent`, Kirigami's default, which leaves the icon to the theme.
  - WinUI: a style whose `Foreground` setter is the colour's theme resource, as for text. It's kept on the node, since a resolved brush can't be told from another.
  - `draws_in_its_colour` checks the pixels in a capture: red where a red icon is, none where an icon has its own colour.
- **Symbol weights and rendering modes** (hierarchical, palette, multicolour) are AppKit's alone, so they're tweaks.
- **Buttons show one before their caption,** as each platform places a button's icon:
  - AppKit: the button's image, `imagePosition` leading, sized for the bezel.
  - GTK: libadwaita's `ButtonContent`, as GNOME apps do it.
  - Qt: `icon.name`.
  - WinUI: a horizontal `StackPanel` of a `FontIcon` and the caption, 8 apart as in WinUI's gallery.
- **`icon_only` hides the caption,** which stays the accessible name: AppKit's image-only position, GTK's own icon button with the caption as its accessible label, Qt's `display: IconOnly`, and on WinUI the glyph alone with the caption as its automation name (UIA reads nothing from a glyph or a panel). A tooltip saying the same is up to the app. Without an icon the caption shows.
- **AppKit measures a borderless image-only button smaller than its symbol** (15 × 9 for the trash can's 15 × 17), so the backend makes it at least the image's size.
- **An icon to assistive technology** (`Role::Image`), named by its label; without one it's decorative. It takes no focus. AppKit's symbol images have no name to read back, so its backend keeps the name and size on the node. GTK, Kirigami and WinUI read them from the widget.
- **Run on every backend and headless** (`tests/icon.rs`, the pixel test too).

### MenuButton

- **A button that opens a menu of actions,** as a toolbar's "Add" or "New" does: a pull-down `NSPopUpButton` on AppKit, `gtk::MenuButton` on GTK, `DropDownButton` on WinUI, and on Qt a `QQC2.Button` that pops up a `QQC2.Menu`, as Kirigami apps make one. A `Select` is for choosing a value; this is for actions, and it keeps no choice.
- **Its menu is a context menu's:** the same builder (`MenuItem`, `MenuSeparator`, `Menu` for submenus, checks and radio items), the same data (`Prop::Menu`, next to `Prop::ContextMenu`, so a menu button can have both) and each backend's context menu code, with the choice reported as `UiEvent::MenuItem`.
- **Caption, icon and style are a `Button`'s,** and each platform draws its own arrow:
  - AppKit: a pull-down's title is its first menu item, so the backend puts the caption and icon there and makes the menu again with it first. It keeps the caption as the accessible name when only the image shows.
  - GTK: a caption shows GTK's arrow. An icon with a caption is libadwaita's `ButtonContent` with `always-show-arrow`. An icon alone has no arrow, as GNOME's icon menu buttons have none. The actions are in a group of their own (`button.*`), apart from the context menu's.
  - WinUI: the content is a `Button`'s. Fluent has no subtle drop-down button, and `SubtleButtonStyle` would replace the template and its chevron, so borderless is a style that only clears the fill and border; it still fills on hover.
  - Kirigami: its role is `Accessible.ButtonMenu`, for which the desktop style should draw a menu arrow, as Breeze does for a `QPushButton` with a menu. The menu pops up under the button and moves into the window's overlay while open, as context menus do. The button shows pressed while it's open.
- **No click of its own:** clicking opens the menu, which is modal, so `perform` takes `MenuItem(id)` (the item's own path, as for context menus) and refuses `Activate`. It's a `Role::MenuButton` to assistive technology, named by its caption, and in the Tab order.
- **The example's tweaks:** a large control size on AppKit, the popover opening upwards on GTK (`direction`), `flat` on Qt, a pill-shaped `CornerRadius` on WinUI.
- **Run on AppKit and headless.** GTK, Kirigami and WinUI are only type-checked. Unverified until they run:
  - that Breeze draws the arrow for a `QQC2.Button` with the `ButtonMenu` role;
  - that Kirigami's menu pops up under the button in the overlay;
  - that WinUI's borderless style keeps the chevron;
  - the arrow's room in each measure.

### Group

- **A box around related content, under an optional heading,** as each platform groups settings: an `NSBox` on AppKit (the title inside at its top), a `heading` label over a libadwaita `card` on GTK (as `AdwPreferencesGroup` lays out a group), a `BodyStrongTextBlockStyle` heading over a card on WinUI (Windows 11's Settings; WinUI has no group box), and a `QQC2.GroupBox` on Qt (Breeze draws the title inside its top). Where the heading goes is each platform's.
- **The core lays out the content,** as a column's (`gap`, and any style), inside the platform's insets: `PlatformMetrics::group_insets`, or `titled_group_insets` with a heading, added to the app's padding. Backends measure them from a probe, as tab views' insets. The group is at least as wide as its heading, which backends measure as they measure a tab strip (`size_tab_strips` now does groups too).
- **The box is drawn behind the content,** in the same host: every backend's group is a layout host with the platform's box (and heading) as its first children, sized with it, so the core's children are placed as in any container.
  - AppKit: the `NSBox` follows the host with an autoresizing mask, and inserts count past it. A box made empty and grown keeps its content view's first frame, so the probe is made at its size. Its content margins are `NSBox`'s own, 5 points (17 at the top with a title on macOS 26), tight beside the others': an app that wants more adds `padding`.
  - GTK: content is 12 inside the card, as GNOME apps put it, and the heading is 12 above; its height comes from a throwaway label.
  - WinUI: the card is the Community Toolkit's `SettingsCard` (the card brushes, a 1 epx border, `ControlCornerRadius`, 16 of padding) and the heading 6 above it, as the WinUI Gallery's settings page spaces them. The heading's height starts from its line height (20), and the first one that loads is measured, sending `MetricsChanged` if it differs, as tab views do for their bar.
  - Kirigami: the insets are a probe `GroupBox`'s paddings (`topPadding` grows with a title), and its implicit size is the empty group's measure.
- **A group to assistive technology** (`Role::Group`), named by its heading, around its content; it takes no focus. The platforms' own heading labels and boxes are hidden from assistive technology, so there's one group, not two.
- **Tweaks get the box:** the `NSBox`, GTK's card, WinUI's card `Border`, the `QQC2.GroupBox`.
- **Tweaks get the box, after the group's props and again when they change** (`a_tweak_gets_the_box_after_its_props`): AppKit's tweak ran on the layout host at first, so a tweak typed for `NSBox` never ran. A tweak can move the heading or change the border, so after measuring a group the core asks the backend where that group puts its content (`Backend::group_insets`), and uses the metrics' insets only when it says nothing. AppKit reads a probe box set up as the group's (title, its position and font, the box type, border and margins), since the box itself may be too small to say: with the title moved to the bottom, the content moved up under the border and ran over the title (`a_tweak_that_moves_the_heading_moves_its_room`). GTK, Qt and WinUI say nothing yet, so their tweaks keep the metrics' insets.
- **The example's tweaks:** the title at the bottom on AppKit (`titlePosition`), GTK's `activatable` class on the card, `flat` on Qt, square corners on WinUI (`IBorder`'s `CornerRadius` added to the bindings).
- **Run on AppKit and headless** (`tests/group.rs`, the `groups` story). `examples/group.rs` is for trying it by hand. GTK, Kirigami and WinUI are only type-checked. Unverified until they run:
  - each platform's insets (the probes outside a window, WinUI's estimate and its re-layout);
  - that the boxes stay behind the content and don't take its clicks;
  - how they look.

### Dropping files

- **A prop of hosts, not a widget:** `Container::file_drop` and `Group::file_drop` take a `FileDrop` (extensions, or any file, and whether folders), `on_drop` gets the paths, and `on_drop_hover` says when files it takes are over it, for the app's own highlight: platforms show only that they'll copy. 2ksbox's disc library takes .iso and .cue files and folders.
- **The backend filters, not the core,** because the platform decides while the files are over the node whether the drop is welcome (the copy cursor), before any drop reaches the core. Every backend filters with the core's `FileDrop::accepted`, which asks the file system whether a path is a folder, and reports only what it keeps, in the drag's order.
- **Not reachable without a pointer:** no platform has an accessible drop, so apps should offer another way in (an open dialog, as the examples do).
- **Each platform's drop target:**
  - AppKit: the host view registers for file URLs and is the `NSDraggingDestination`, reading the URLs from the dragging pasteboard.
  - GTK: a `gtk::DropTarget` for `gdk::FileList` with `preload` on, since GTK hands a drop's files over only once they're read: until then it offers to copy, and once they're in, a drag with nothing the node takes is refused (`reject`). Remote URIs without a local path aren't taken.
  - WinUI: the host's `Canvas` with `AllowDrop` and XAML's drag events, and a clear background so its empty areas are hit. XAML gives a drag's files only asynchronously (`GetStorageItemsAsync`), so entering starts reading them; until they're read the host takes the drag on its format, and from then on only if the filter keeps one of them. A drop before the read finishes reports when it does; leaving ends the drag, so a late read is dropped.
  - Kirigami: a Qt Quick `DropArea` over the host, after its children (Qt Quick hands drags only to items that take drops). On `entered` the backend filters the URLs and the area accepts the copy only if something is kept, so a refused drag gets no `exited` or `dropped`. A drop ends the hover itself, since Qt sends no `exited` after one.
- **Tests drag through each backend's own handling** (`SyntheticInput::DragFiles`, `DragLeave`, `DropFiles`, the test kit's `drag_files`, `drag_leave` and `drop_files`), with real files and folders in a temporary folder (`tests/file_drop.rs`). What they skip is reading the paths from a real drag, which nothing here can start: `examples/file_drop.rs` is for trying it by hand from the file manager, and the icon example's library takes dropped discs too.
- **Run on AppKit and headless.** GTK, Kirigami and WinUI are only type-checked. Unverified until they run: reading a real drag's paths on every platform (AppKit's pasteboard included); GTK's preload during hover and `reject`; WinUI's read finishing during a drag from Explorer, and its canvas getting drags over empty areas; Kirigami's `drag.urls` and `keys` for drags from Dolphin.

### Tooltips

- **A prop, not a widget:** `.tooltip(text)` on any widget or container (`ElementBuilder`), sent as `Prop::Tooltip`; empty removes it. 2ksbox puts one on a status line cut off at one line, holding the whole text.
- **Shown as each platform shows them,** with its delay, placement and look: `NSView.toolTip`; `set_tooltip_text`; `ToolTipService` on WinUI; on Qt the attached `QQC2.ToolTip` (Breeze draws it), visible while hovered after `Qt.styleHints.mousePressAndHoldInterval`, as Kirigami apps do it. Qt Quick has no tooltip without that binding: controls use their own `hovered`, labels, images and container hosts a `HoverHandler`.
- **On the view under the pointer:** a list's table or list view rather than the scroll view around it, and on AppKit a `NumberInput`'s field and stepper as well as their host.
- **Containers:** a WinUI host is a Canvas with no background, which never gets the pointer, so it gets a clear background while it has a tooltip (and takes the pointer over its empty areas meanwhile). A box's tooltip also shows over children without one of their own: on AppKit (tried by hand), on Qt (hover is passive), and GTK looks for a tooltip up the widget tree.
- **Read as the description** unless the app gave one: the core's tree does that, AppKit reads a tooltip as the view's help and GTK as its description on their own, Qt gets `Accessible.description`, WinUI `AutomationProperties.HelpText`.
- **Custom renders, drawn and native items on Kirigami** keep the tooltip on the node but don't show it: their QML is the app's.
- **Not tested: showing on hover,** since nothing here can rest a pointer on a native widget. The suite checks that each backend's native widgets and containers carry it, and the description; `examples/tooltip.rs` is for trying it by hand.
- **Run on every backend and headless,** and tried by hand on each.

### Windows opened while the app runs

- **`Window` is a view, declared anywhere in the tree,** next to the state that opens it: shown while its `open` value is true, like `Show`, which is how 2ksbox's launcher drives its windows (a model's `open` flag). Its content is built when it opens and disposed when it closes, so each opening starts fresh; it closes with the scope that declared it, so a window declared in another window's content closes with that one. In the tree it's a fragment that takes no room; the native window is top-level. `App::window` stays for the windows open at startup; the app ends when the last window closes, whichever kind.
- **The close button asks, and the app decides.** `bind(flag)` closes it by clearing the flag; `on_close_request` lets the app keep it open (to ask about unsaved changes, as `examples/windows.rs` does); without either the close button does nothing, since the flag is the app's. Platforms deliver the request their own way (`windowShouldClose:`, `close-request`, `closing`, `AppWindow.Closing`), always vetoed, and the core destroys the window when the app closes it.
- **Modal windows are the app's choice of two** (`.modal(Modality::…)`, or `.modality(value)` read at each opening): `Window` blocks the window it's declared in, `Application` the whole app. The maintainer chose to offer both rather than pick for macOS, where they look different: `Window` is a sheet on its window (the macOS idiom for a dialog about a window), `Application` a separate window centred on its owner, run in `NSApp.runModalForWindow` (what Qt does for 2ksbox's dialogs today). Elsewhere: Qt's `WindowModal` and `ApplicationModal` with a `transientParent`; GTK's `modal` with `transient_for`, which blocks the whole app either way (GTK has no window-modal; GNOME attaches modal dialogs to their parent); WinUI's owned window with `OverlappedPresenter.IsModal`, which disables the owner, plus the app's other windows disabled for `Application`, as Win32 apps do.
- **A window's size is read at each opening too** (`.size(value)`), as its modality is, since each opening creates the window anew. The windows example needs it: a dialog follows its content's height, while a plain window has a fixed size, because macOS can open a plain window as a tab of another (always in full screen, or as the user's "Prefer tabs" setting says), and a tab takes its window's frame: following its content's height shrank the whole showcase window. Run on AppKit and headless (`its_size_applies_at_the_next_opening`); GTK, WinUI and Kirigami only type-checked, though the change is in the widget, not the backends.
- **Only modal windows belong to a window:** the one they're declared in, found through `CurrentWindow`, which each window's scope provides (`App::window`, `Window`, the test kit's mount). A window declared inside a modal window belongs to that one. One declared outside every window (rare) has no owner, so a `Window` modality becomes `Application`: Qt's window-modal blocks nothing without one. Plain windows belong to none, so they aren't kept above another window, as owned windows are on Qt, GTK and WinUI. Every `Window` builds in a scope of its own, even when always open, so its `CurrentWindow` doesn't leak to what's declared after it.
- **Qt ignores a modality set on a window already shown,** and `Kirigami.ApplicationWindow` shows itself when created (`visible: true`), so modal windows didn't block anything. The backend's windows start hidden, as they were meant to, and show after their first layout. The tests read the modality back, which Qt keeps even when it ignores it, so this was found by hand and checked against `QGuiApplication::modalWindow()` in a small program.
- **Alerts and file dialogs without a window go on the focused one, and Escape closes only the focused modal window.** Qt's `active` is true for the focused window's transient parents, and so for their other transient children: a modal window's owner and its other dialogs all read as active. Taking the first active window put alerts on the owner, which the modal window blocks, and Qt matches window shortcuts by `active`, so two modal windows' Escape shortcuts were ambiguous and neither fired. The shim sets each window's `mitsuamiFocused` from `QGuiApplication::focusWindow()`, which alerts and the Escape shortcut use. Synthesized keys go through the shortcuts first (`qt_sendShortcutOverrideEvent`, as QTest does), after activating their window.
- **The same on WinUI: the active window** (`GetActiveWindow`). The backend took the first window with a focused control, but every XAML window keeps one while inactive, so the alert could land on a modal window's owner, which the modal window disables. Without an active window (the app in the background), a window that isn't disabled, focused if one is. The test kit answers alerts itself, so this was checked by hand in the windows example.
- **Known gap: GTK's window-modal windows aren't tied to their window.** They're separate modal windows, `transient_for` their owner, which block the whole app. Only GNOME's compositor attaches them, so they move with their owner there; on sway they move on their own, as any GTK app's do. GNOME apps (Text Editor) now use libadwaita's dialogs, drawn in the parent window, which move with it everywhere and block only that window, like a macOS sheet. `Modality::Window` as an `adw::Dialog` and alerts as `adw::AlertDialog` would close the gap, at the cost of a libadwaita dependency, a dialog clipped to its owner (a bottom sheet in a narrow window), and title, size and close requests wired through the dialog. Left for later.
- **On WinUI it's applied when the window is shown** (windows are activated hidden at creation, for a live XAML tree): `GWLP_HWNDPARENT` to the owner, centred on it, not minimizable or maximizable, then `IsModal`; for `Application` the app's other windows are disabled with `EnableWindow`, leaving alone those another modal window already disabled, and re-enabled on destroy, before the owner comes back to the front.
- **A sheet has no close button,** so its content must offer a way out: a `ButtonRole::Cancel` button, which Escape presses on AppKit. The modal loop can't start inside a tick (it's nested), so AppKit schedules it on the main run loop after the tick that shows the window; the UI keeps ticking inside it, since the app's observer runs in the modal panel mode too. Destroying the window ends its sheet (`endSheet:`) or its loop (`abortModal`, the call for code outside event handling, with an empty event to wake the loop). Tests never show windows, so they check the modal prop the backends carry, not the sheet or the loop: `examples/windows.rs` lets you pick each and try them.
- **Escape asks a modal window to close** (tried by hand on each platform), through the close request, as every platform's dialogs close on Escape (AppKit's Cancel button, `QDialog`, `GtkDialog`, Win32's `IDCANCEL`); plain windows ignore it, as they do everywhere. Each goes through the platform's own path, so a focused control that uses Escape (an open pop-up, completion) gets it first: on AppKit a key equivalent (a `ButtonRole::Cancel` button takes it), then `cancelOperation:` up the responder chain to the window's delegate, which a field editor passes on; on GTK a bubble-phase `ShortcutController` calling `close()`, as `GtkDialog` has; on Qt a `Shortcut` on `StandardKey.Cancel`, as 2ksbox's launcher has; on WinUI a `KeyboardAccelerator`, which fires only when the focused control didn't handle the key. Synthesized Escape takes the real path on AppKit and Qt (a key event); GTK 4 can't inject key events and WinUI's backend drives controls, so theirs run the window's shortcut or accelerator action directly, and a focused control can't take Escape first in their tests.
- **`TestHooks::close_window`** clicks the close button through the platform, so the request goes the way a user's does: `performClose:`, `gtk::Window::close`, `QQuickWindow::close()` (which sends nothing to a window never shown; test windows are shown by then), and on WinUI a posted `WM_CLOSE`, where the close button and Alt+F4 end up, since `Window.Close()` closes without raising `Closing`. `TestApp::close_window` and `TestApp::window_titled` use it; locators already search every window.
- **In `view!`, the title is an attribute and the children are the content:** `<Window title="Machine" bind=editing>…</Window>`. `Window::new` takes the title; the tag starts without one, and the title is a type parameter, `()` until set, as `#[component]` does for required props.
- **`on_open` runs at each opening, before the content is built,** to start a form from what's saved. The builder API can do it in `content`'s closure, but `view!`'s children are views, so it needs a handler: `<Window title="Advanced" bind=open @open=move || draft.set(…)>`. A window's content is one view, as `content` takes; several children don't make a `View`, so they go in a `Column`.
- **Nested dialogs are windows declared in a window's content,** which is how the tests and `examples/windows.rs` write a cancellable dialog: "Advanced…" in a machine window opens a `Modality::Window` dialog on it, with a draft started in `on_open` that OK applies to the machine window's form and Cancel, Escape or the close button drop. It belongs to the machine window and closes with it.
- **Run on every backend and headless** (sheets and the modal loop tried by hand in `examples/windows.rs` on each, since tests never show windows).

### Full screen and window size

- **Full screen is each platform's own** (`Window::full_screen(signal)`, `Prop::FullScreen`): AppKit's `toggleFullScreen:`, a Space of its own with its animation (what winit's borderless full screen does on macOS too); `gtk::Window::fullscreen`; Qt's `WindowFullScreen` state; WinUI's full-screen presenter. The user changes it too, the platform's way (the title bar's green button, Escape and the menu bar on macOS, the window manager's key), which the backend reports as `FullScreenChanged` and the core absorbs, like `PointerLockEnded`. A refusal is reported the same way, so the signal always says what the window shows.
- **Every platform reports its own changes too, and later:** AppKit's `windowWillEnterFullScreen:` comes inside `toggleFullScreen:` (a small Swift program showed it, and that a toggle during the animation is ignored), GTK's `notify::fullscreened` when the compositor configures the window, Qt's `windowStateChanged` inside `setWindowStates`. So each backend keeps what the app asked for and reports only what differs from it; muting while applying commands wouldn't do.
- **A window takes full screen once it's shown** on AppKit and WinUI: the Swift program showed a hidden `NSWindow` goes into full screen unseen. Tests never show windows, so natively the mirror check compares what the window will show. AppKit applies a request made during a transition when it ends. A sheet can't have full screen (AppKit refuses), nor a dialog on WinUI (an owned, modal presenter): the app hears `false`.
- **Full screen keeps the window's size for when it leaves.** AppKit resizes a window in full screen when told to (the Swift program again), so every backend ignores `SetWindowSize` while it's in full screen; GTK's default size would apply when it leaves, and WinUI's kept `OverlappedPresenter` gives its settings back. The Fluent title bar is collapsed meanwhile on WinUI: the full-screen presenter has no caption.
- **A minimum content size** (`Window::min_size`, `Prop::MinSize`): AppKit's `contentMinSize`, a size request on GTK's content host (the header bar's above it), Qt's `minimumWidth` and `minimumHeight` with Kirigami's toolbar added, WinUI's `PreferredMinimumWidth` and `PreferredMinimumHeight`, which are the whole window's in pixels (the title bar, menu bar, toolbar and frame added). A window smaller when it's set grows to it: GTK does that itself, the others' backends do it, since 2ksbox's player had to ask for it (winit's minimum alone doesn't grow a window on every platform). Qt and WinUI apply it again when the chrome above the content changes. On WinUI, Windows grows the window to a new minimum itself as it's set, before XAML lays the root out again, so the resize that follows uses the client insets (the resize border inside the client area) measured before: measured after, off a root at the old width, they came out as their 8 px cap, and `grows_to_its_minimum_size`, run natively, got a window 8 px too wide. **It goes no larger than the screen:** capped at the content of a window filling its screen's visible area, and applied again when the window moves to another screen, since a machine's mode can be larger than a laptop's screen (the player capped it at `current_monitor`'s size), and AppKit would make a window as large as the minimum asks (a small Swift program gave a 5000 × 5000 window, in full screen too). `native_state` reports the app's minimum while the platform holds the capped one. AppKit caps at its screen's `visibleFrame` and again on `windowDidChangeScreen:`; GTK at the window's monitor less the header bar (GTK 4 has no work area on Wayland, so panels aren't taken off), again on `enter-monitor`; Kirigami at the screen's `availableGeometry` less the window's frame and the toolbar, again on `screenChanged`; WinUI at the monitor's work area (`GetMonitorInfoW`'s `rcWork`) less the frame and the chrome, again after `AppWindow.Changed`'s `DidPositionChange` (not yet on a scaling change on the same display). Run on AppKit and headless (`its_minimum_goes_no_larger_than_the_screen`); the others are only type-checked.
- **The app resizes a window** with `Ui::set_window_size(window, size)`, finding it through `CurrentWindow`. The platform may refuse, and gives no less than the minimum, so the core lays the content out at the size the platform reports (`WindowResized`) rather than the one asked for. AppKit's `setContentSize:` goes below `contentMinSize`, so its backend clamps, and its test hook does too, as a user's drag can't go below it. GTK used to set its content's size at once; now only before the window is first mapped. On GTK and Kirigami, `grows_to_its_minimum_size` and `the_app_resizes_it` failed when first run natively: both backends took the size they gave (the one asked for, or grown to the minimum) as the one the core already had, so it was never reported. The core's size is now the one it asked for, and the backend reports what the window gets once it has it; GTK allocates it at the next frame, so its settle waits for it (at most two seconds, as for a user's resize, about a second on Broadway), and so does Kirigami's until Qt has laid it out.
- **A window's height can follow its content,** as 2ksbox's Clone dialog does (its `height`, `minimumHeight` and `maximumHeight` bound to the content's): a note or a progress bar comes and goes, and the window grows and shrinks with it. The app chooses what the user may do: `WindowSize::FollowHeight(width)` keeps the height the content's, and the user resizes only the width; `FollowHeightUntilResized(width)` follows until the user changes the height, then it's an ordinary window, as WPF's `SizeToContent` stops on a user's resize. No platform does all of this on its own (GTK 4 non-resizable windows take their content's size, WinUI 3 has no `SizeToContent`, AppKit and Qt apps resize by hand), so the core does it the same way everywhere: whenever the content or the width changed, it lays the window out at max-content height, sends a new height through the ordinary `SetWindowSize`, then lays the content out at the height the window has, so a minimum taller than the content leaves room below it. A window in full screen doesn't follow; it catches up when it leaves. `Ui::set_window_size` gives a `FollowHeight` window only a width, and ends following for the others.
- **Whose height changed** is the core's guess, since only AppKit tells a user's resize apart: a reported height that's one the core asked for and the platform hasn't reported yet (GTK and Qt apply them later, so several can be under way), grown to the minimum, is the core's; any other is the user's. A window too tall for its screen isn't capped: an app whose content can grow that far gives it a scroll view.
- **The user can't change a `FollowHeight` window's height** (`Prop::HeightFollowsContent`, which the core sets): a minimum and a maximum at the height it has, moved with it, on AppKit (`contentMinSize` and `contentMaxSize`, so the resize cursors only go sideways), Qt (`minimumHeight` and `maximumHeight`) and WinUI (`PreferredMinimumHeight` and `PreferredMaximumHeight`, the outer size in pixels as for the minimum). GTK 4 can't hold one side of a window, so there it isn't resizable at all (`set_resizable(false)`), as GNOME's content-sized dialogs are; its default size still sets its size (GTK's `toplevel_compute_size` reads it for windows that aren't resizable, and doesn't remember the user's). The platform's minimum then holds the locked height, so `native_state` reports the app's minimum from what the backend kept. AppKit's `setContentSize:` keeps a window's top edge where it is (checked with a small Swift program), so it grows downwards, as the others do. Run on AppKit and headless (`tests/windows.rs`); GTK, Kirigami and WinUI are only type-checked. The windows example's machine dialog follows its content.
- **`App::open(window)` opens a `Window` at startup,** for what only a `Window` has (full screen, a minimum size, a title that changes): the player's one window is one. As any `Window`'s, its close button does nothing unless it's bound or handled.
- **Run on AppKit and headless** (`tests/windows.rs`: headless fills a 1280 × 800 screen and simulates the user's change; AppKit's full screen isn't shown in tests, as above). `examples/gpu-surface` has a full-screen checkbox, a minimum size and `App::open`, for trying by hand. GTK and Kirigami run natively on their private displays (Broadway, Qt's offscreen platform); the tests pass natively on WinUI too (once the minimum's resize used the insets measured before, which also showed the minimum is the outer size in pixels). Unverified until they run: whether Broadway does full screen at all (a request it ignores stays pending, and the mirror check wouldn't see it), whether a compositor that refuses full screen is ever heard on GTK (it sends nothing), whether Broadway before GTK 4.16 grows a mapped window for a new minimum without presenting it again, whether the offscreen Qt platform gives the window its old size back, and on WinUI, that the minimum follows a DPI change (nothing applies it again yet), that `AppWindow.Changed` fires for our own `SetPresenter`, and that the kept presenter gives the window its size and position back.

### Measurements

- **Views read the sizes layout gave** (§3, Responsive): `use_viewport()` is the window's content size, `use_size(node_ref)` a node's, both zero until the first layout. A `NodeRef` follows its node: `Show` and `For` building it again set it again, and it's `None` while nothing is built. They're the core's frames, so they need nothing from backends; text and controls measure per platform, so a breakpoint on a control's size falls at a different width on each one.
- **Sizes are reported in the same turn:** after each commit, `Ui::tick` sets the sizes that changed and commits again, so a view that switches at a breakpoint is laid out with its new branch before the run loop sleeps, and the old one never shows. A view whose size flips with its own size never settles: after 8 reports in a turn the rest wait for the next, as browsers' `ResizeObserver` does, so the run loop goes on (`a_view_that_never_settles_doesnt_hold_the_run_loop`).
- **Not yet:** a node's position (its frame in the window moves with scrolling, which isn't a layout), and text measurement for drawn widgets, which can't draw text yet.
- Run on every backend and headless (`tests/measurements.rs`). `examples/measurements.rs` is for trying it by hand: a sidebar that moves at a breakpoint, and cards in as many columns as their panel has room for.

### The app's id, name and icon

- **`App::id`, `App::name` and `App::icon`** (an `AppInfo` in the core, `Ui::set_app_info`, `Backend::set_app_info`), set before the first window. The id is reverse DNS (`org.example.Player`), the one Linux desktops and Windows want; the icon is an image file or its bytes (`AppIcon::bytes(include_bytes!(…))`), PNG everywhere, an `.ico` file too on Windows. There's one icon for the app, not one per window: macOS has no window icons, and Wayland shells show the app's.
- **Each platform takes what it has a place for, and a packaged app's own wins.** On macOS the bundle's id and icon (an asset catalog has the sizes and the dark and tinted variants one image hasn't), so the icon is set (`applicationIconImage`) only when the bundle has none, as for `cargo run`, and the id is ignored. The name goes in the app menu's About, Hide and Quit, which took the process's name before; the menu bar's bold title is the bundle's or the process's, which only AppKit sets. On Windows the AppUserModelID (which groups the app's windows on the taskbar) is set only when the app isn't in a package, whose id is its own, and the name is left to the executable's version resource or its shortcut. Each window gets the icon with `AppWindow.SetIcon`, as a Win32 app's windows get their class's: an `.ico` file's path (with its sizes), or else an `HICON` made from the PNG (`CreateIconFromResourceEx`, which takes PNG data; an `IconId` is that handle, as a `WindowId` is a window's).
- **Linux apps are known by their id:** the Wayland app id and X11 class match the `.desktop` file, and the icon theme has the app's icon under that name, which its package installs. Without a `GtkApplication` GTK takes both from the program name, so the backend sets it, in `run` before GTK starts (GDK's X11 backend reads the class then) and again in `set_app_info`, and gnome-session's registration already sent it. GTK 4 windows only show themed icons, so GNOME apps set the default icon name to their id and so does the backend; the app's image is ignored, and an app run from its build shows no icon, as an uninstalled GNOME app doesn't. Qt, as KDE apps do (`KAboutData`): the desktop file name, the display name, which Qt adds after each window's title ("Home — Dolphin"; not when the title already ends with it), and the window icon from the theme by the id, with the app's image when the theme has none.
- **Tests read it back** (`TestHooks::app_info`, `tests/app_info.rs`): what the platform shows, `None` where it has no place. AppKit keeps the name (only its menus show it), and reports the Dock icon's size in points: it keeps a snapshot of the image at the screen's scale, 40 × 20 pixels for the 20 × 10 PNG on a Retina display.
- **Run on AppKit and headless.** GTK, Kirigami and WinUI are only type-checked, and Kirigami's shim compiled against Homebrew's Qt. `examples/windows.rs` sets all three, for trying by hand. Unverified until they run: that `AppWindow.SetIcon` gives the window the big icon `WM_GETICON` reads back, and that `CreateIconFromResourceEx` makes an icon of the PNG's own size; that GTK updates windows already open for a new default icon name; that `QWindow::icon` falls back to the app's on Qt's offscreen platform.

### File dialogs: the start folder and every file

- **`OpenFile::start_folder` and `SaveFile::start_folder`** open the dialog in a folder, as 2ksbox's path fields do (the folder of the file a field names, else the last folder browsed): its users know where their disk images are, and a dialog that starts elsewhere makes them walk there every time. A folder that isn't there is dropped in the core (`services::existing_folder`), so the platform chooses, as with none. AppKit's `directoryURL`, GTK's `initial-folder`, Qt Quick's `currentFolder` (the save dialog's `selectedFile` goes in it when there's a name), WinUI's `SuggestedFolder` (`IFileOpenPicker2`, `IFolderPicker2`; the save picker's is on `IFileSavePicker`). The platform's own memory of the last folder is what applies without one. WinUI also has `SuggestedStartFolder`, which gives way to that memory; it isn't used, since the app asked for this folder.
- **`FileFilter::all(name)`,** a filter with no extensions, lets every file through, offered after a filter by type ("All files"), so a file the types miss can still be chosen: a `.Cue` when the filter lists `cue` and `CUE` (GTK and Qt match case-sensitively on Linux), or a file with an extension the app didn't list. GTK adds the pattern `*` to the filter; Qt reads `All files (*)`; WinUI's open picker takes named choices (`IFileOpenPicker2.FileTypeChoices`, as the save picker's) when there are filters, the flat `FileTypeFilter` with `*` when there are none. AppKit's panels have no filter menu, only the types they allow, so a filter that lets every file through allows every file. WinUI's save picker leaves it out: a save choice is the extension the name gets.
- **Run on GTK and Kirigami natively** (each backend's `tests/services.rs`: the dialog's folder and its filters, read back from GTK's `GtkFileChooser` and Qt's `currentFolder` and `nameFilters`; without the portal on GTK, as the test display has it) **and headless** (`tests/services.rs` in `mitsuami`). AppKit and WinUI are only type-checked. Unverified: that `FileTypeChoices` on the open picker shows as a choice of filters, and ignores `FileTypeFilter`, which stays empty then; that `SuggestedFolder` wins over the picker's memory on both pickers; that the portal's dialog (GNOME's, KDE's) honours `initial-folder` and the `*` pattern.

### Toolbar

- **What every platform shares:** a bar across the top of the window that shows its title (the unified `NSToolbar`, GTK's header bar, Kirigami's page toolbar, WinUI's `CommandBar`), with arbitrary items at its trailing end (`NSToolbarItem.view`, `pack_end`, a `Kirigami.Action`'s `displayComponent`, an `AppBarElementContainer`). Leading items and the title's place differ too much (Kirigami has no leading items; GTK centres the title), so only trailing items are built. The title stays the window's title, shown as each platform shows it.
- **`Toolbar` is a view, declared anywhere in a window's content,** like SwiftUI's `.toolbar`: it finds its window through `CurrentWindow` and puts its items there; they go with the scope that declared it, so `Show` around it takes them away. So it works for `App::window` and `Window` alike, with no new window API.
- **Each child is one item,** a `ToolbarItem` host: a native child of the window, after its content (the core sorts them last, so the content's indexes don't move). The core lays each out on its own at its natural size (`layout_toolbar`), outside the window's layout box; the platform places it and spaces the items, and the core reads where from `native_state`, as for list rows. So an item's frame is in the content's coordinates, above it (y < 0), and `visible_rect` clips what's in an item by the item, not by the content.
- **An empty item is hidden:** a `Show` that shows nothing leaves an item with an empty frame, which the backends hide (AppKit takes it out of the toolbar: `NSToolbarItem.hidden` is macOS 15 only). 2ksbox's download progress shows this way only while a download runs.
- **Items that don't fit are the platform's too:** AppKit and WinUI move them to an overflow menu. The backend then reports the item at `Rect::ZERO`, and the core takes it as hidden. GTK's header bar makes the window wider instead, and Kirigami keeps them (`KeepVisible`).
- **Tab: a gap.** Toolbar controls aren't in the core's Tab order, which is the content's. That's right for AppKit, whose toolbar items are outside the key view loop. On GTK, Qt and WinUI, whose own Tab chains reach the bar (the titlebar's widgets; QQC2 buttons; a `CommandBar` as one Tab stop), the backends' Tab handling walks only the core's order, so Tab never reaches the toolbar there. 2ksbox's toolbar has no controls. Letting Tab leave the content for the platform's own chain at its ends would close the gap.
- **The content keeps its size.** `SetWindowSize` is still the content area; the bar is added to the window.
- **AppKit:** the toolbar is made with the first item, with a flexible space ahead of the items; each item's view is its host, sized by Auto Layout constraints, as toolbars size views. A window never shown (as in tests) doesn't make its toolbar's views until asked, so the backend lays out windows with toolbars after each batch and in `settle`. Tests turn off the toolbar's animation, whose frames they'd otherwise read on the way. On macOS 26 each item gets a glass capsule drawn tight around its view, and the system's own views pad themselves; ours don't, so the backend insets each host 10pt from its capsule's sides, as far as the system's image items have their image (chosen by the maintainer from variants side by side; 7pt top and bottom as well looked the same). Before macOS 26 there's no capsule and no inset. Adjacent capsules still come close enough that the tests only check the items' order.
- **GTK:** items go in the window's own `HeaderBar`, packed at its end in order (packed again from the last one, since `pack_end` fills inwards), with GNOME's main-menu button still last; the header bar spaces them. Each host is `valign` centre: the header bar stretches its children to its height, and a host places its content from the top, so a lone `Text` sat at the top of the bar where a `GtkLabel` would be centred. An empty item is `set_visible(false)`. On a display nobody watches, frames stall, so the header bar wouldn't place an item shown again until the next one; `settle` allocates header bars that need it where they are, as it does lists. An item taller than the header bar grows the window, so the content keeps its size.
- **Kirigami:** items are page actions: a `Kirigami.Action` with `displayHint` `KeepVisible` whose `displayComponent` holds the host, which is how KDE apps put search fields in the page toolbar. The toolbar shows them after the title with its own spacing; an empty item's action is hidden. When the content host's height changes without the window's (an item taller than the bar), the backend measures the toolbar again and resizes the window.
- **WinUI:** a `CommandBar` in a row of its own under the title and menu bars, collapsed while no item shows, with each host in an `AppBarElementContainer` among its primary commands; its defaults (overflow, trailing commands) are kept. XAML measures only what's in a live tree, and a collapsed bar keeps its items out of it, so a button in an item measured nothing, got an empty frame and stayed hidden (text needs no template, so it showed): each new item is laid out once with the bar shown, then collapsed until its first frame. A container is the bar's height and puts its content at the top, so it centres it (`VerticalContentAlignment`), as the bar centres its own buttons. The bar's trailing "More" button spaces itself from the window's edge, and with no secondary commands it doesn't show, which left the items against the edge: the bar has a trailing margin of 16, as far as the title bar insets the title from the leading edge (`TitleBarLeftPaddingWidth` 2 + `TitleBarLeftHeaderPaddingWidth` 14). A margin, since the bar's padding only reaches its content area, and the bar has no background. The window grows by the bar, as with the menu bar. The windows' `TitleBar` control would put items beside the title (`RightHeader`), as on AppKit and GTK; `CommandBar` was chosen as WinUI's toolbar control, and `TitleBar`'s areas sit in the drag region.
- **Run on every backend and headless,** and checked by eye on each. On WinUI, XAML lays a window's content out at a new size only at its next frame (a new window's at the size it opened at, wider than the one set), so `settle` waits until each window's content is at the size last reported, or the toolbar's items would be read at the old edge. And `resize_client` aims with the title bar and resize border as XAML last laid them out, which at the new size are a pixel off (the content 599.33 high for 600): once XAML has laid the content out at the new size, the backend corrects the window by the miss, if it's a pixel or two.

### Menus

- **Menus stay a service, not widgets.** Every platform builds its menus from data and calls back with the item chosen, and on macOS the bar isn't in any window. So a `MenuBar` is data (`MenuBarData`) the core sends again whenever it changes, not nodes in the tree: menus have no frames, no Tab order and nothing for the mirror check to read back.
- **What's built:** submenus; check items (`MenuItem::bind` toggles a signal, `checked` only shows it) and radio items (`radio((signal, value))`, a pair so `view!` can write `radio=(zoom, Zoom::Large)`); reactive titles, visibility (`visible`, on items and menus) and lists of items (`Menu::children_with`); roles (`MenuRole::{About, Settings, Quit}`); and a window's own menus, a `MenuBar` in its content, written in `view!` (`<MenuBar>`, `<Menu title=…>`, `<MenuItem>`, `<MenuSeparator/>`). `MenuItem::new` takes only the title now, with `on_select` for the handler, as `Button` has `on_click`.
- **Radio items next to each other form a group,** as a separator ends one on every platform. The core keeps a group exclusive: choosing an item sets the signal, and every item's check comes from it. `MenuData::radio_groups` names each group by its first item, for the platforms that group natively.
- **Ids are unique across bars and stay put while the structure does:** each bar gives the item at each position the same id on every rebuild. So when only enabled and checked states change (`MenuBarData::same_structure`), GTK, WinUI and Kirigami update the items in place, and an open menu stays open. Any other change rebuilds that window's menus. AppKit rebuilds its bar every time, as it did before.
- **A window's menus are shown with the app's** (`MenuBarData::merged`): a window menu titled like an app menu joins it after a separator, and the others follow. On GTK, WinUI and Kirigami that's what the window shows. On macOS, where the bar is the app's, a window's menus are there while it's the main window (`NSWindowDidBecomeMain`/`ResignMain`), the window menu commands act on. AppKit posts those while a window closes, which the backend does while applying commands with its state borrowed, so the bar follows them once the run-loop turn is over; the first version looked the window up right away, and closing any window crashed. A window's menus reach the services while its content is built, before the backend has applied its `CreateWindow`, so backends keep them by `NodeId` and use them when the window is made; Kirigami builds its drawer inline then, as it needs to. A `MenuBar` outside any window is the app's, as `set_menu` is; installing the app's menus again replaces them. **Dialogs show only their own menus** there (`MenuBarData::for_window`): dialogs on Windows, GNOME and KDE have no menu bar of the app's, nor its Quit. A window's modality comes with its `Create`, so a dialog is never built with the app's menus (Kirigami couldn't take its drawer away without the binding-loop warning). Kirigami's drawer still ended with its own Quit in dialogs; `dialogs_show_only_their_own_menus` found it when the backend's tests first ran natively on Linux. On macOS the bar stays the app's while a dialog is open, as it does for every Mac app: its modal loop disables what doesn't apply.
- **Roles move items only where the platform has a place for them** (`MenuBarData::take_role` takes them out, with separators and menus left empty). AppKit puts About, Settings and Quit in the app menu with its own titles and shortcuts ("About <app>", "Settings…" ⌘,, "Quit <app>" ⌘Q), as Qt's menu roles do. GTK puts them in the primary menu's last section in GNOME's order (Settings, About, Quit), Settings getting Ctrl+, if the app gave it none. Kirigami ends the drawer with them (Settings with KDE's Ctrl+Shift+,, About, Quit, with their theme icons), as KDE apps do. On Windows there's no standard place, so they stay where the app put them. An app's Quit replaces the platform's (AppKit's `terminate:`, GTK's and Kirigami's "ask every window to close"), titled and bound as the platform's is; on Windows it's an ordinary item.
- **The platform's own Quit goes to the app too** (`Ui::request_quit`): the app's Quit item, as if chosen, if it has an enabled one, otherwise a close request to every window, as by its close button. The app decides either way, and ends when its last window closes. 2ksbox's player needed it: a guest killed while it writes leaves its disk dirty, so the player asks first when it's closed from the keyboard, and on macOS it had routed the Dock's Quit to its window's close request itself. On AppKit it's the app delegate's `applicationShouldTerminate:`, which every `terminate:` asks: the app menu's default Quit, the Dock's, logging out and restarting. The app's handlers run in that call (a tick), since the answer is due when it returns. With windows left, it's cancelled, and macOS says the app cancelled the log out, as it does for TextEdit with unsaved documents; with none, the process ends there, so code after `App::run` doesn't run then, as for any AppKit app the system quits. AppKit's default Quit used to end the app at once; it asks the windows now, as GTK's and Kirigami's did already. A small app whose window refused the first close request was quit twice from outside (`NSRunningApplication.terminate`, the Dock's quit event): it stayed, then ended.
  - GTK had no session integration (the backend doesn't use `GtkApplication`), so logging out ended the process. It now takes the routes `GtkApplication` takes, over GIO's D-Bus: outside a sandbox it's a client of `org.gnome.SessionManager`, whose `QueryEndSession` asks the app; with windows left it answers no with a reason, so GNOME lists the app in its log out dialog with "Log Out Anyway". `EndSession` is answered yes, since the session ends whatever the app says by then, and `Stop` ends the run. In Flatpak, where the session manager isn't reachable, the portal's session monitor does the same, holding a logout inhibit while the app keeps windows. Without a GNOME session (Sway) nothing asks, and the session ends as it did.
  - Kirigami: `QGuiApplication::commitDataRequest`, since Qt 6 closes no windows then; a window left cancels the logout (`allowsInteraction`, then `cancel`), as KDE apps with unsaved work do. If it comes while a tick runs, the app keeps the session: it hasn't been asked. Qt's session management is XSMP, which KWin likely doesn't give Wayland clients, so on Wayland it probably never comes.
  - WinUI: `WM_QUERYENDSESSION`, in a subclass of each top-level window. The app is asked once per session end, and every window gets that answer; with windows left it's no, with `ShutdownBlockReasonCreate`, so Windows lists the app on its "preventing shutdown" screen, as it lists Notepad with unsaved changes, and the user can end it anyway. A query that comes while the app is being asked, or in a pump inside a Ui call, says no with the same reason. `WM_ENDSESSION` forgets the answer, and the reason if the session end was cancelled.
  - Only AppKit has run. GTK, Kirigami (the shim only compiled against Qt 6.11's headers) and WinUI are only type-checked. Unverified: that gnome-session shows the reason and lets go when the process ends, the portal's session handle in Flatpak, ksmserver granting interaction, that `WM_QUERYENDSESSION` reaches our subclass while our loop waits, and that Windows lets the app finish once it closes its windows.
- **Check marks are the platform's:** AppKit's item `state` (a check mark for radio items too, as AppKit's menus show a choice); a boolean stateful action on GTK, and for a radio item an action of its own holding its id while chosen, with the id as the item's target, which GTK draws as a radio; `ToggleMenuFlyoutItem` and `RadioMenuFlyoutItem` (`GroupName` from the group's first id) on WinUI; `checkable` actions on Kirigami, each radio group in one exclusive `QQC2.ActionGroup`. XAML and Qt toggle an item themselves when it's clicked, so their backends put back the app's state before reporting the choice; only the user's click reports one.
- **XAML leaves room for a check mark on every row** of a menu with a toggle or radio item, checked or not, so a menu whose one check item is off shows an empty column. Explorer avoids it with plain items that show a check mark icon while on (XAML only makes room for icons some item has), but those read as plain menu items to screen readers, without a checked state. The toggle items stay: they're XAML's own control for this, and the column is how XAML draws it.
- **Run on every backend and headless** (`tests/menus.rs`, and the AppKit services test, which posts the main-window notifications itself since test windows are never main); `examples/menus.rs` is for trying them by hand, and they were checked by eye on each.

### Context menus

- **A prop, not a widget, made of menus:** `.context_menu(entries)` on any widget or container (`ElementBuilder`) takes what a `Menu` takes (items, separators, submenus, with reactive titles, states, `bind` and `radio`), and sends it as `Prop::ContextMenu` whenever it changes; empty is none. The platform reports the item chosen as the node's `UiEvent::ContextMenuItem(id)`, so ids only need to be unique within the node's menu. Unlike menu bars they're not a service: each belongs to a native view, and the mirror check reads it back.
- **Shown as each platform shows them,** on its own gestures: `NSView.menu` (right-click, Control-click); a `PopoverMenu` of a `gio::Menu` at the pointer on GTK (secondary click, long press on touch, Shift+F10 and the Menu key), its actions in a group inserted on the widget; `ContextFlyout` of a `MenuFlyout` on WinUI, which XAML opens on right-click, press and hold, Shift+F10 and the Menu key itself; a `QQC2.Menu` of `Kirigami.Action`s on Qt, popped up at the pointer from a right-button `TapHandler` (on press, as KDE's menus open), a long press on touch, and the Menu key or Shift+F10 at the item's centre. Qt 6.9's `ContextMenu` attached type would do that, but the backend supports Qt 6.5. Items are built by the menu bar's code on every platform.
- **Children without a menu show their container's,** as a right-click does everywhere: AppKit passes the click up the responder chain, XAML's `ContextRequested` bubbles, GTK's gesture denies a click when the widget has no items, and Qt's handlers are off while an item has none. The test kit does the same: `choose_menu_item` acts on the nearest node up the tree with a menu.
- **AppKit's tables take right-clicks on their rows' labels:** over a label, the table's `hitTest:` answers the table (so that a click selects the row), which then shows its own menu, so a list row's menu showed only around its text. Found by hand in the example: a `swiftc` probe sending right-clicks with `sendEvent:` hit the label instead, so a real click was traced. The table's `menuForEvent:` looks from the cell under the pointer (`rowAtPoint:`, `columnAtPoint:`, then the cell's own `hitTest:`) for the nearest view with a menu, and falls back to the table's own, the `List`'s.
- **Where a native menu is taken, the native one wins.** AppKit's pop-up button's menu is its options, and a right-click opens it: a `Select` keeps its context menu on the node without showing it (GTK, WinUI and Qt show one). Text fields keep their Cut/Copy/Paste menu: GTK adds the app's items to it (`extra-menu`, as GNOME apps do), Qt keeps KDE's and doesn't show the app's, WinUI's `ContextFlyout` replaces XAML's while the app has items, and on AppKit the field editor shows its own while editing. A GTK spin button's +/- keep their right-click (jump to the ends), so a `NumberInput`'s menu is on its text there. Qt's custom renders, drawn and native items keep it on the node, as they do tooltips.
- **Shortcuts are shown, not bound:** a context menu's items show their shortcut, as menus do, but only the menu bar's shortcuts work from the keyboard. Qt would match an action's shortcut with its menu closed (ambiguous between rows), so its actions have one only while the menu is open. XAML runs a flyout item's accelerator while focus is inside its owner; that's left as XAML does it.
- **Qt's open menus sit in the window's overlay.** An item that changes a list's rows resets the view, and a row's host leaves the window between delegates; a popup whose parent changes window shows itself again in the new one, and KDE's menus are still visible during their exit fade, so Duplicate and Sort By stayed open. While it's open, the menu's parent is the window's `Overlay.overlay`, and it goes back to its item once closed. Right-clicking a KDE text field logs `Shortcut: Only binding to one of multiple key bindings` for each item: qqc2-desktop-style's own text field menu gives its actions `StandardKey`s, which its `MenuItem` binds with `sequence`. Every Kirigami app logs it, and we leave it.
- **Choosing without opening:** `A11yAction::ContextMenuItem(id)` chooses an item as a screen reader does once it has shown the menu, through the item's own path: AppKit's `performActionForItemAtIndex:`, GTK's `activate_action`, Qt's `trigger()`, WinUI's automation peer (Invoke, else Toggle, else what `Click` does). Disabled items and disabled controls refuse it: disabled controls show no menu (GTK, XAML and Qt don't deliver them input).
- **Updated in place when only enabled and checked states change** (`MenuBarData::same_structure`, the entries as one menu), so an open menu stays open; GTK refills the same model otherwise, WinUI and Qt rebuild. GTK's popover (or text widget) lets go of the model while it's refilled: refilled under it, a `PopoverMenu` adds the new submenus' pages to its stack before the old ones go, and warns about their duplicate names (a title that changes, e.g. Start/Stop, rebuilds). A one-item GTK popover has extra space under its item; a bare GTK 4.22 `PopoverMenu` has it too, so it's left as GTK draws it. XAML's radio group names are shared across the thread, so groups are named by their window or node as well as their first id: two windows' bars, or a bar and a context menu, could uncheck each other's items.
- **Run on every backend and headless** (`tests/context_menu.rs`; right-clicks tried by hand in `examples/context_menu.rs` on each, on GTK also through Xwayland with `xdotool`), and checked by eye on each. Finder outlines the row a right-click is on (a table's `clickedRow`), and Files and Dolphin select it; here neither happens yet.

### Sidebar

- **What every platform shares:** a list down the window's leading side that picks what the window shows, as the system's own settings apps have: items with an icon and a title, in sections with an optional heading, one of them chosen, and the window's content beside it. How it collapses in a narrow window differs too much to share, so it's each platform's: AppKit's split view collapses the sidebar item as it does (dragging its divider away), libadwaita's split view becomes a stack of the two pages below 400sp (a choice shows the content, whose header bar has a back button), Kirigami's page row shows one page at a time, and WinUI's `NavigationView` in `Auto` shows only icons, then a menu button.
- **`Sidebar` is a view declared anywhere in a window's content,** like `Toolbar`: it finds its window through `CurrentWindow`, and goes with the scope that declared it. `Sidebar::new(selection)` takes a signal of the app's own type; each `SidebarItem::new(title, value)` gives it a value, and choosing an item sets it. A value no item has chooses none. Titles are reactive; icons are names in the platform's own set, picked with `platform!` (an SF Symbol, a symbolic theme icon, a Segoe Fluent Icons glyph), as there's no icon type yet. Items outside a `SidebarSection` next to each other are a section without a heading.
- **One node, its items as data.** The node (`WidgetKind::Sidebar`) is a native child of the window after its toolbar items, with `Prop::Sections` and `Prop::SelectedIndex` (the index across sections), and reports `Changed(Index)`. Items aren't nodes: every platform builds them from data, as menus are, and a native list needs no layout from the core. The a11y tree still shows them, as list items under headings; they stand for the sidebar node, and the test kit selects one by its title (`A11yAction::SetValue`), as a `Select`'s option is chosen.
- **The window's content is what's beside it,** so the core's window is the content: its size is the content's, and the window is larger by the sidebar, as with the toolbar. The sidebar's frame is the platform's, read back like a toolbar item's, beside the content (at negative x); a collapsed one is empty and hidden. It's first in the Tab order.
- **The content's page is titled after the item chosen** on GTK and Kirigami, whose panes have header bars of their own (as GNOME and KDE settings do); the sidebar's page takes the window's title. AppKit and WinUI leave the window's title as it is.
- **AppKit:** the window's content view becomes an `NSSplitViewController`'s (`contentViewController`), with the full-size content view the sidebar needs to reach under the title bar, as on macOS 11 and later; the host is in the content item, under the title bar and toolbar (its safe area). The window gets a toolbar with the sidebar's tracking separator first, so the title and toolbar items are over the content. The table is in the source-list style at AppKit's own row size (the user's sidebar icon size), with a fixed slot for icons so titles line up. Its width is AppKit's default (a fraction of the window's, within limits: 140 at 800 wide), and macOS 26 floats it, inset 8pt from the window's edges. A toolbar's identifier is now unique per toolbar: a window that got a new one while the old one wasn't freed yet (a sidebar shown again) had AppKit keep the two in sync, and assert. Offscreen captures can't draw the glass the sidebar sits in (nor, then, its rows), so window captures stay the content's.
- **GTK:** `adw::NavigationSplitView` in an `adw::BreakpointBin` (360×294, GNOME's smallest window) with the collapsing breakpoint; the window's header bar, now libadwaita's, moves to the content page, and the window's title bar gives way to the pages' (a hidden one, as `AdwWindow` has). The list is a `gtk::ListBox` in GTK's `navigation-sidebar` style with headings for titled sections and lines between untitled ones, as GNOME Settings had before libadwaita 1.9's `AdwSidebar`, which needs a newer floor than Ubuntu 24.04's 1.5. The window's extra width follows libadwaita's sidebar width (a quarter of the window, 180 to 280, taken as points). Every page has a title before it's realized, which libadwaita checks (a shown window realizes the split as it's added): the content's is the item's or the window's, and an untitled page takes the app's name, which its header bar doesn't show. With libadwaita, `Appearance` goes to `AdwStyleManager:color-scheme`, not GTK's `gtk-application-prefer-dark-theme`, which it warns about.
- **WinUI:** a `NavigationView` takes the content host's row, with the host as its content, no Settings item and no back button. The window's extra width is the pane's as `Auto` shows it at that width (open from 1008, icons only from 641). While the pane is hidden (`Minimal`), the menu button is the window's `TitleBar`'s, as Task Manager has it, and opens the pane; the view's own would sit over the content's corner. A navigation view takes focus on its selected item, and its selected item is compared with the items by COM identity (the `IUnknown` pointers).
- **Kirigami:** a `Kirigami.ScrollablePage` ahead of the content's page in the window's page row, as System Settings has its categories, at the row's default column width; `ItemDelegate`s, with `ListSectionHeader`s for titled sections, and a header's line alone between untitled ones (not before the first). `PageRow.insertPage` pops the pages from its position on first, which took the content's page away, so the sidebar is pushed after it and moved ahead (`movePage`). The page is made apart from the window, where Kirigami's `applicationWindow()` isn't defined: the window gives it its page row. A list view shows every section delegate it makes, so a header is hidden inside one. The user's choice (a click, the arrow keys) is a signal of the page's; the app's isn't reported.
- **Toolbar items go between the content and the sidebar,** so a toolbar declared after the sidebar (on one of its pages, as the showcase's Toolbar page is) is inserted before it. The backends placed items by counting the window's other children as content, the sidebar among them, and refused the index as before the content (`a_toolbar_can_come_after_it`). The fix is the same one line in each backend; it ran on AppKit and headless, and GTK, WinUI and Kirigami only type-checked.
- **The showcase** (`examples/showcase`) is the examples in one window, a sidebar item each: each page is its example's `page()` (or named view), the example file included as a module, so the two can't drift apart. It's a crate of its own, outside the workspace, as `examples/gpu-surface` is, since it shows that example too (so wgpu isn't built with the workspace's tests). The GPU surface page's render thread stops when the page goes (a flag its scope's cleanup sets); it used to draw until the app ended, which was the window's life there. Its full-screen checkbox is the showcase window's. Releases build it alone, for each platform (`release.yml`), where they used to build every example: its `kde` feature, with `--no-default-features`, builds the KDE one without GTK.
- **Run on every backend and headless** (`tests/sidebar.rs`, the `sidebar` story; `examples/sidebar.rs` for trying by hand). Qt's offscreen screen is 800×600 by default, too narrow for a sidebar beside a 500pt minimum, so Kirigami's tests give it 1920×1080 (the platform's `configfile`).

### Tabs

- **A view switcher, not document tabs:** a fixed set of pages, one shown, with a tab for each, as in a settings window. Every platform has one: `NSTabView`, libadwaita's inline view switcher, WinUI's `SelectorBar` (its `TabView` is for documents, closable and reorderable) and Qt Quick's `TabBar`. It's `Tabs` rather than GNOME's "view switcher", which names only the switcher in a header bar, not the pages.
- **`Tabs::new(selection)` with `Tab::new(title, value)`s,** as `Sidebar` takes its items: the signal holds the value of the page shown, and picking a tab sets it. Every platform's tab view always shows a page, so a value no tab has shows the first and leaves the signal as it is (as a `Select` shows its first option). Titles are reactive. A `Tab` is its page's column: `.padding`, `.gap` and the other style setters go on it. In `view!`: `<Tabs selection=page>` with `<Tab title="General" value=Page::General>…</Tab>`s.
- **Every page stays mounted,** as every platform's tab view keeps its pages: what's typed in one, and its scroll position, are still there when it's picked again. The pages the platform doesn't show are out of the a11y tree and the Tab order, and have an empty frame; which page shows is the core's to say (`SelectedIndex`), so an empty page shown still counts as shown.
- **One node, its pages as native children.** `WidgetKind::Tabs` has `Prop::TabTitles` and `Prop::SelectedIndex`, and reports `Changed(Index)`; its children are page hosts (`Container`s), as a list's are row hosts. Tabs aren't nodes: in the a11y tree they're `Role::Tab`s of the `Role::TabGroup`, before the page shown, and the test kit picks one by its title (`A11yAction::SetValue`), as a sidebar's item.
- **The core sizes the pages, the platform places them.** The tab view is a one-cell grid holding every page, so it's as big as its biggest page, as a notebook or `StackLayout` is, and doesn't change size from page to page; grown, its pages fill it. Its padding is where the platform's page area is (`PlatformMetrics::tab_insets`: the strip on top, the border elsewhere), measured from a real tab view by each backend; its own padding is ignored. Its minimum is its strip, which the backend measures (the only container the core measures), once it exists: at the start of layout, which resolves styles again if a strip changed. The pages are where the platform put them, as rows and toolbar items are, at the core's size.
- **AppKit:** an `NSTabView` with its tabs on top. Each host is in a plain flipped view of its own as its item's view, which the tab view sizes to its page area, at the view's top-left. Its insets are measured on one (27 on top and 3 elsewhere from its alignment rect on macOS 26: the frame's page area, less the alignment insets). Its minimum width is AppKit's `minimumSize`, at which it truncates its tabs' titles, as AppKit does. The delegate's `tabView:didSelectTabViewItem:` is called for the backend's own selections too: they're muted.
- **GTK:** libadwaita's `InlineViewSwitcher`, centred above an `adw::ViewStack` (12 pixels apart, GNOME's spacing between groups), as GNOME apps switch panes inside a window; the pages are the stack's, titled. The switcher is new in libadwaita 1.7, above the 1.4 floor (§11), so it's a capability: the backend builds against 1.4 and looks up the switcher's type when it runs (`dlsym` of its type getter); with an older libadwaita, or the `notebook-tabs` feature (`gtk-notebook-tabs` on `mitsuami`) for apps that want GTK's own tabs, it's a `gtk::Notebook`, a `gtk::Label` for each tab. The two measure differently, so `tab_insets` comes from whichever is used. CI's GTK runner is Ubuntu 24.04, whose libadwaita is 1.5, so CI tests the notebook, and the switcher only runs on newer systems (it passed here, on Arch Linux's libadwaita 1.9). Either stretches its pages over their area, so each host is in a layout host of its own at its top-left. A view stack only appends, so a page inserted before others takes them out and puts them back after it, keeping the page shown. Insets come from a throwaway view's page bounds (an even border all round if it can't be read). `visible-child` (the notebook's `switch-page`) is muted while commands apply. A view that switched pages is allocated again at settle, so the new page's place can be read. Both pass `tests/tabs.rs` natively on Linux (the notebook through the feature: this machine's libadwaita has the switcher).
- **WinUI:** a `Canvas` holding a `SelectorBar` (an item for each title) and the page hosts under it, the shown one `Visible` and the others `Collapsed`. The bar's height isn't known until one is in a window: `tab_insets` starts from 48 (worked out from its template), and the first bar that loads is measured and sends `MetricsChanged` if it differs. `SelectionChanged` is guarded by the page shown (`shown`), and a null selection (the selected item removed) is ignored.
- **Kirigami:** a `QQC2.TabBar` of `TabButton`s over a plain `Item` holding the pages, the shown one visible (a `StackLayout` would size the pages itself). The insets are a probe bar's height. Only `TabButton.clicked` reports the user's choice. Qt Quick's `TabBar` has no arrow keys, and Tab reaches only its selected tab, so the backend adds Left and Right (mirrored for right-to-left), as `QTabBar` and the other platforms' tabs have.
- **Run on every backend and headless** (`tests/tabs.rs`, the `tabs` story, a `view!` case in `tests/macros.rs`; `examples/tabs.rs` for trying by hand).

### GpuSurface

- **A surface the app presents to itself,** from its own thread, as it would present to a window of its own: `GpuSurface::new().on_ready(…).on_resize(…)`. `on_ready` gets a `SurfaceHandle`, which implements `raw-window-handle`'s traits (so `wgpu::Instance::create_surface(handle.clone())` takes it) and is `Send + Sync`; `on_resize` gets its `SurfaceSize` (pixels and scale), which `handle.size()` also has for a render thread. The backend reports `UiEvent::SurfaceReady` once the native surface exists, then `SurfaceResized` whenever its pixel size or scale changes. `examples/gpu-surface` (a crate of its own, so the workspace's tests don't build wgpu) draws moving bands from a render thread with `Fifo`.
- **The handle keeps the native surface alive,** after the widget is gone too: a GPU surface made on it must never outlive it (Wayland and Vulkan treat that as an error). When the node goes, the surface stops showing and reporting, and is freed with the last handle, on the UI thread wherever that's dropped: AppKit releases the view on the main queue, WinUI posts `WM_CLOSE` to its child window, and Wayland objects may be destroyed from any thread.
- **Each platform's route:**
  - AppKit: a layer-backed `NSView` whose backing layer (`makeBackingLayer`) is a `CAMetalLayer`, sized with the view by AppKit. Its contents stay at the top left while it resizes (`layerContentsPlacement`): AppKit's default for layer-backed views stretches the last frame to the new bounds, so every frame drawn at the old size showed stretched until one at the new size arrived. wgpu takes the view's layer as it is, and must be given the handle on the main thread (it reads the view there), so `on_ready` is the place. It's part of the view hierarchy, so views can draw over it.
  - GTK and Kirigami: a `wl_surface` of our own, a desync subsurface of the window's surface, over a widget (`gtk::DrawingArea`) or item that keeps the space, sized with `wp_viewport`, with an empty input region, on the toolkit's own Wayland connection (`mitsuami-linux`; libwayland-client is loaded at run time, as the toolkit loaded it). It's placed after each of the window's frames (GTK's frame clock's `after-paint`, Qt's `afterAnimating`), so it follows the widget wherever layout or scrolling moves it; a move takes effect with the window's next commit, so it asks for a frame. Without its subsurface role it isn't shown: hiding the widget, or destroying the node, takes the role away. A window's surface is new each time it's mapped, so the role is made again then. Qt's own routes weren't taken: a `QQuickRhiItem` renders in the scene graph's frame, and a child `QWindow` is destroyed and made again with its window. Qt's window surface comes from its platform native interface (`qpa/qplatformnativeinterface.h`, under Qt's versioned include directory).
  - GTK and Kirigami on X11: a child window of the window's, on an xcb connection of our own (`mitsuami-linux`, x11rb with libxcb loaded at run time), since xcb is thread-safe and the app presents from its own thread (the handle is `Xcb`). It has no background (what was there shows until the app presents) and an empty input shape, so the pointer goes to the toolkit's window. It's placed in the window's pixels ((widget point + GTK's surface transform) × scale; Qt's scene point × device pixel ratio, the window manager's frame being outside the client window), stacked above its siblings, and moved under the root window when its node goes, so the toolkit's window doesn't destroy it with itself. It's made with the toolkit window as its parent, so it has that window's visual (with a compositor, GTK's is ARGB: what the app presents with alpha shows through).
  - WinUI: a child window (HWND) over a `Canvas` that keeps the space, placed before each of XAML's frames (`CompositionTarget.Rendering`); without input it answers `WM_NCHITTEST` with `HTTRANSPARENT`. That passes the pointer to windows under it in the same thread, but XAML's content island (`DesktopChildSiteBridge`) never gets it: XAML sees no pointer over the child window, found by injecting real mouse input (`mouse_event`) over it, and seeing XAML's events come once it was hidden. When its node goes it's hidden and moved under `HWND_MESSAGE`, so the XAML window doesn't destroy it with itself. A `SwapChainPanel` would compose with XAML (wgpu has it for Direct3D 12 only); a child window works with any GPU API. With Vulkan on NVIDIA (an RTX 3090, driver 32.0.16.1664), fast resizes sometimes leave the surface at 2 frames a second: each acquire waits about 500 ms on a fence in the driver, and reconfiguring doesn't recover it. That happened in about 1 in 6 runs of 1,500 scripted resizes, and never with Direct3D 12 (0 of 23), nor with Vulkan in a plain winit window, drawing to the window itself or to a child window like ours. The example uses Direct3D 12 on Windows.
- **Where platforms differ:** on GTK, Kirigami and WinUI the surface sits above the window's own content, so nothing the toolkit draws can be over it (an overlay, a menu in full screen needs the platform's own layering); on AppKit it's a view like any other. A GTK 4.10 window's scale is a whole number, so a fractional display gets pixels at the next whole scale, as GTK draws its own. The size in pixels is rounded as each platform rounds it; the tests allow a pixel.
- **No natural size:** it's as large as the layout makes it. It reads as an image (`Role::Image`) named by its label, and takes no focus unless it takes input.
- **Input (`on_input`, `Prop::TakesInput`):** keys and the pointer, as 2ksbox's player needs them for its machine. Keys come by where they are on the keyboard (`KeyCode`, the W3C `code` names, from macOS virtual key codes, Linux evdev codes and Windows scan codes, with the platform's code as `native`; on Linux, keys that type no character by the keysym the keymap gives them), down, up and the platform's repeats; the pointer in points, every button, scrolling in lines (a wheel) or points (a trackpad), positive towards the end. A click or Tab focuses it. It gets every key the window doesn't take first for its shortcuts, in each platform's order, Tab too (a game's view or a terminal takes Tab); Control+Tab leaves it on AppKit, GTK and Kirigami, as it leaves a text view. Input comes through the widget under the surface, the toolkit's own events, since the surface takes none (the pointer on WinUI aside):
  - AppKit: the view is a responder like a game's: `acceptsFirstResponder`, `keyDown:` after the menus' key equivalents, `flagsChanged:` for modifier keys (each side's device flag; Caps Lock, whose flag is the lock's, is reported down and up per press), a tracking area for moves and leaving. A local event monitor, while it's the first responder, takes every `keyUp` (AppKit sends none for a key let go while Command is held). Focus leaving releases the keys it has down.
  - GTK: the area's own controllers (a key controller, motion, `EventControllerLegacy` for every button, scroll). mitsuami's window shortcuts are a bubble-phase `ShortcutController`, which GTK would run after the focused widget; so a key with Control, Alt or Super is left to the window when one of its shortcuts matches it exactly (`ShortcutTrigger::trigger`), as application accelerators in GTK's capture phase would win. Plain keys (Escape in a dialog too) go to the surface.
  - Kirigami: a C++ item filling the item that keeps the space, overriding Qt's key, mouse, hover and wheel handlers (`nativeScanCode` − 8 is the evdev code, `nativeVirtualKey` the keysym). Qt reports a held key as release and press pairs with `isAutoRepeat`: the releases are dropped. The backend's own Tab filter lets Tab through to a focused surface.
  - WinUI: keys through the `Canvas` itself (WinUI 3's `UIElement` has `IsTabStop` and `Focus`). The pointer over the surface never reaches XAML (above), so the child window reports it from its own messages (`WM_MOUSEMOVE`, the buttons', the wheels', `WM_MOUSELEAVE` through `TrackMouseEvent`), holds it while a button is down (`SetCapture`), sets the app's cursor (or the arrow) on its `WM_SETCURSOR`, and focuses the canvas on a press. The wheel goes to the focused window when Windows' "scroll inactive windows" is off: XAML then finds the canvas, which has a clear background for it. Keys reach the surface only while XAML's focus is on it, and three things left it elsewhere, for every window: XAML gives a Page's first focusable element focus when it loads, but the window's content is a `Grid`, so a window opened with nothing focused; after another app's window took activation for a moment (NVIDIA's overlay), XAML's own restore left focus on its root `ScrollViewer`; and a press on the `TitleBar` control reached that `ScrollViewer`, which focuses itself. So once XAML has handled a window's activation (a dispatcher item queued from `Activated`), and when a window is first shown (it's activated when made, before its content and before the handler exists), the backend focuses the control that had focus, or the first in the core's Tab order, when XAML's focus isn't on a node; and the title bar marks presses handled and isn't a tab stop, as a Windows caption leaves focus alone. Win32 focus matters too: keys go to the window that has it, which XAML keeps on its content island. A few seconds after the example starts presenting, with NVIDIA's overlay loaded into it (`nvspcap64.dll`), Win32 focus moves from the island to the XAML window itself, with no input, on Direct3D 12 and Vulkan alike, and before the child window took the pointer too; the text input example, which presents nothing, keeps it. Keys then reach nothing until the window is activated again (Alt+Tab twice), when XAML gives the island focus back. Found by watching the example's UI thread from outside (`GetGUIThreadInfo`). So each frame, a surface whose window has Win32 focus itself gives it to the island (`InputFocusController.TrySetFocus` for `XamlRoot.ContentIsland`), which gives it back to its focused element. XAML raises `LostFocus` on the canvas when the island loses Win32 focus, though the canvas stays its focused element; that doesn't end the keyboard grab, which ended the example's capture (the lock with it) each time the overlay showed. A stand-in for the overlay (a topmost window that activates and closes) no longer loses focus, and a click on the status text leaves it on the surface. The maintainer tried the example with the real overlay: the surface has key focus from the start, keys keep coming when the overlay shows (the watcher's log showed Win32 focus staying on the island), and the capture survives it. XAML runs accelerators on the focused element's path before its `KeyDown`, but those elsewhere (the menus') only if `KeyDown` goes unhandled ("Keyboard accelerators", Input event priority, on Microsoft Learn): keys with Control or Alt are reported and left unhandled, so a menu's shortcut still fires.
- **Pointer lock (`pointer_lock`):** the cursor hidden and held, its moves reported as `SurfaceInput::Motion` in points, accelerated as the cursor would be. Only in the active window; the platform ending it, or the window stopping being the active one, reports `PointerLockEnded`, which sets the app's signal back (2ksbox locks again on the next click). None of the toolkits has one:
  - AppKit: what games do: `CGAssociateMouseAndMouseCursorPosition(false)` and `NSCursor.hide`, the cursor warped to the surface's middle; the mouse's moves still come as events, with their deltas. It ends when the window resigns key.
  - Wayland (GTK and Kirigami): `zwp_locked_pointer_v1` (one-shot) on the window's surface with `zwp_relative_pointer_v1`, on a seat and pointer of our own (the compositor sends a client's pointer events to each of its pointers). Their events are dispatched on a thread of our own (libwayland lets several threads read a connection, as GTK and Qt do) and sent to the UI thread: GTK with `MainContext::invoke`, Kirigami through a queue drained when Qt's loop wakes. The cursor is hidden with the toolkit's own cursor (`none`, `Qt::BlankCursor`).
  - X11 (GTK and Kirigami): a pointer grab on the child window, confined to it, with a blank cursor; each move is reported as its distance from the middle, then the cursor is warped back. A click that asked for the lock holds the toolkit's implicit grab until its button is let go, so the grab is retried for two seconds. The grab takes the buttons and the wheel from the toolkit, so the lock's thread reports those too.
  - WinUI: `ShowCursor(FALSE)`, `ClipCursor` to the canvas, and `SetCursorPos` back to its middle after each move. It ends when the window is deactivated. Hiding the canvas while locked reports `PointerLockEnded` there; the other backends release it silently (Kirigami locks again when shown).
- **Raw motion (`SurfaceInput::RawMotion`):** while locked, each move comes twice: `Motion` in points, accelerated as the cursor would be, then `RawMotion` in the device's counts, before the host's acceleration. 2ksbox's player gives a PS/2 guest the raw one (winit's `DeviceEvent::MouseMotion`): the guest accelerates it itself, and would accelerate the host's acceleration. Counts aren't points, so they're a variant of their own.
  - AppKit: GameController's `GCMouse` (`mouseMovedHandler`, macOS 11), whose deltas Apple documents as "not affected by mouse sensitivity settings", with no permission (IOHID would need Input Monitoring). A small Swift program checked it by hand against `NSEvent`'s deltas while locked: slow moves gave `GCMouse` 3 to 4 times as much, fast ones half as much, so the ratio followed the speed, as acceleration does. Up is positive there, so y is turned round. Its handlers run on the main queue; a mouse has one handler, so the last surface to lock has it. Mice connecting while locked get it too.
  - Wayland: `zwp_relative_pointer_v1`'s `dx_unaccel` and `dy_unaccel`, in the same event as the accelerated move.
  - X11: XInput 2's `XI_RawMotion`, which only the root window gets and only for a client that said it speaks XInput 2: the lock's thread asks for 2.0 and selects it on the root, on our own connection, while the lock holds. Without XInput 2 there's no `RawMotion`; the lock and its `Motion` still work.
  - WinUI: Raw Input, the mouse registered to the surface's child window while the lock holds (no `RIDEV_INPUTSINK`: the lock only holds in the foreground window), `lLastX` and `lLastY` from `WM_INPUT`. Absolute moves (remote desktop, tablets, some virtual machines' mice) have no counts, so they give only `Motion`. A process has one Raw Input registration per device kind: ours replaces any other while locked, and removing it drops one the app made itself.
  - Run on AppKit and headless (headless reports both, one count to a point) only as far as the suite goes: a test can't move a real mouse, so while captured the example's circle follows the raw moves, one count to a point. The maintainer tried it on AppKit: its first version moved the circle by `Motion`, which still felt accelerated, as it should; moved by `RawMotion`, it doesn't. Wayland, X11 and WinUI are only type-checked. Unverified: that an X server delivers raw events to our root selection while our own client holds the grab (XInput 2.0 may not), whether libinput's Xorg driver already scales them, that `WM_INPUT` reaches a child window registered as the target, and that `SetCursorPos`'s warps give no raw input.
- **Keyboard grab (`keyboard_grab`):** focuses the surface, then gives it every key, the window's shortcuts and as many of the system's as the platform lets an app take. It ends (`KeyboardGrabEnded`) when the surface or its window loses focus, or the platform lets go.
  - AppKit: the key monitor takes every key before AppKit dispatches it (menus' key equivalents, Command-`), and the app's presentation options turn off process switching (Command-Tab) and hiding; AppKit allows the first only with the Dock hidden or hiding itself, so the Dock hides while the keyboard is grabbed. Mission Control, Spotlight and the Spaces shortcuts stay the system's: only an event tap, which needs an Accessibility permission, or private API takes those.
  - GTK: `gdk::Toplevel::inhibit_system_shortcuts` (the Wayland shortcuts inhibitor, a keyboard grab on X11), and a capture-phase key controller on the window that gives the surface every key; `shortcuts-inhibited` going false ends it.
  - Kirigami: on Wayland `zwp_keyboard_shortcuts_inhibitor_v1` of our own (Qt has no API; its `inactive` ends it), on X11 `QWindow::setKeyboardGrabEnabled`; the item accepts `ShortcutOverride`, so the window's shortcuts don't fire.
  - WinUI: a `WH_KEYBOARD_LL` hook, while the window is the foreground one and the canvas has focus, swallows every key and reports it itself: Alt+Tab, the Windows key, Alt+F4 and the app's accelerators. Control+Alt+Delete stays the system's.
- **Remapped keys and the first focus on Linux.** The maintainer ran the example under Sway with Caps Lock and Control swapped: the surface reported the keys where they are, not what the swap made them. The swap is the keymap's (`ctrl:swapcaps`), applied above the evdev code, where Windows's scan code map remaps below the code it reports. So on GTK and Kirigami, keys that type no character (modifiers, Caps Lock, Escape, arrows, function keys) go by their keysym (`KeyCode::from_keysym`): on GTK the key's unshifted keysym in the event's layout (`Display::map_keycode`), on Qt `nativeVirtualKey`, which has the shift level applied, so `Meta_L` (Shift+Alt on most layouts) is left out. Letters and digits keep their place, as a game's WASD needs, and `native` stays the evdev code. On Kirigami the surface also took no keys until a click or Tab: Qt Quick focuses nothing in a new window, where AppKit focuses its initial first responder and GTK its first control. The backend now focuses the first control in the window's order, once, if nothing has focus. Both backends' native suites give the same results as before, and the maintainer tried both fixes in the example on GTK and Kirigami under Sway.
- **The cursor over it** (`GpuSurface::cursor`, `Prop::Cursor`): the platform's own, none, or the app's image at its scale with its hotspot in points, e.g. the one a machine gives its pointer. It's set on the widget under the surface, whose pointer it is. `Hidden` is a blank cursor, not the platform's "hide the cursor", which is global and counted (`NSCursor.hide`). The lock's hidden cursor wins while it holds. No platform gives an image back, so the backends keep the prop.
  - AppKit: a cursor rect over the view (`resetCursorRects`), shown while the window is the key one; none is a clear 1 × 1 image.
  - GTK: the drawing area's `set_cursor`: `none` hides it, as GTK's video widget does, and an image is a texture. GTK 4.10 shows a texture one pixel per point, so an image at another scale is resampled to its size in points (GTK 4.16's `Cursor::from_callback` would keep the detail).
  - Kirigami: the input item's `setCursor`: `Qt::BlankCursor`, or a `QCursor` from a pixmap with its device pixel ratio.
  - WinUI: `WM_SETCURSOR`, in subclasses of the XAML window's windows: over the surface they set the app's cursor instead of letting XAML set the arrow. An `InputCursor` can't be made from pixels, and `ProtectedCursor` is only a subclass's. The image is redrawn at the display's scale (nearest pixel) as a 32-bit `HCURSOR` (`CreateIconIndirect`); none is a null cursor.
- **Keys held are let go** when the surface loses focus or its window stops being the active one: they're reported released right then, since their releases go to another window, and the app would think them held (2ksbox's player lifted every key on winit's `Focused(false)`). AppKit did it when focus left; now also on `NSWindowDidResignKeyNotification` while it's the first responder, since the first responder stays when the window stops being the key one. GTK forgot them silently when focus left; it reports them now, on focus `leave` and on the window's `is-active` going false. Kirigami on losing active focus, which Qt Quick takes away when the window deactivates, and on `activeChanged` too. WinUI on `LostFocus` and on `Window.Activated` (deactivated), since XAML's focus doesn't move then; a release it never saw go down is dropped, as on AppKit. On GTK and Kirigami a key is let go as the code its press was reported as, which for a remapped key is its keysym's, not its place's. `tests/gpu_surface.rs` holds a key down on AppKit with a real key event and posts the notification; without the observer, it failed.
- **Synthesized input** (tests): a click focuses the surface and reports the primary button, a key goes down and up, a scroll is reported in points. AppKit sends real `NSEvent`s through the view's own methods and Kirigami real Qt events for clicks and keys; GTK, WinUI and every scroll report what the platform would.
- **Not yet:** text input (typed characters, input methods) on the surface, and captures of what the app presented (AppKit's `cacheDisplayInRect:` doesn't read a Metal layer; the story shows where the surface sits).
- **Popups over the surface.** The surface is above its window's own surface (a subsurface, a child window), so what Qt Quick draws in the window's overlay goes under it: a `Select`'s list opened behind the example's surface on Kirigami, while GTK's popover, a popup surface of its own, showed above. Kirigami's select opens its list in a window of its own (`popupType: Popup.Window`, Qt 6.8 and later), and it showed above the surface under Sway. Context menus, tooltips and the global drawer are still drawn in the window: context menus as windows were misplaced (the menu moves to the window's overlay to pop up, and showed in the window's corner), and drawers can't be windows.
- **Linux tests' private displays have no surfaces.** GTK's tests run on Broadway and Qt's on its offscreen platform, neither Wayland nor X11, so no surface was made and five tests failed on both. There the backends now make a `NoSurface` (`mitsuami-linux`): the app is told of it and its size, as headless tells it, with a handle that has no window or display handle, and a lock or grab ends at once. `TestApp::has_surface_handles` says where handles come (not headless, nor on the private displays); the tests that check them read it. With `MITSUAMI_SHOW_WINDOWS=1` the tests run on the session's display, where they check the real surfaces.
- **The size follows the frame at once.** Run on Sway (`MITSUAMI_SHOW_WINDOWS=1`), `follows_its_frame` failed on both: the new size was reported after the window's next frame, which a settle doesn't wait for, and the app would draw its next frame at the old size. GTK reports it when the area is allocated (`resize`), and the backend's settle allocates surfaces at their new frame, as it does lists and header bars; Qt when the item's width or height changes, which `SetFrame` sets right away. WinUI failed it too, run natively: it places the child window before XAML's next frame (`CompositionTarget.Rendering`), so its settle now lays out and places each surface itself. On GTK a synthesized key also asked `has_focus`, which is false while a test's window isn't the active one; it asks `is_focus`, focus within its window, as Kirigami asks its window's focus item.
- **Run on AppKit and headless** (`tests/gpu_surface.rs`: headless gives a handle without window handles, and sizes at its scale factor; the AppKit run checks the view's layer is a `CAMetalLayer`). The example was run by hand on AppKit: it presents from its render thread, and follows resizes without stretching (it draws in points, with a circle that would show as an oval; its first version drew in fractions of the surface, which looked stretched at any size). From a terminal whose windows macOS says are hidden, wgpu skips every drawable, so it can't be checked from there. Input runs on AppKit and headless too (clicks, keys, Tab into the surface, scrolls; the lock and grab ending, headless only, since a test run's window may not be the key one; natively the test checks the props say what the platform did). The example takes input: the circle follows the pointer, a click captures the pointer and keyboard, Control+Option lets go; the maintainer tried it on AppKit: the capture worked, but Control+Option showed a frozen cursor, and left the lock and grab on. A modifier key's `flagsChanged:` event was asked whether it repeats, which only key downs and ups answer: AppKit raised an exception, its run loop swallowed it, and no modifier key was ever reported. Found by posting a click and the modifier keys to the app's own event queue (`NSApp.postEvent`, which needs no Accessibility permission, and goes through local monitors as real input does), and covered by `takes_modifier_keys`, which sends the view a real flag-change event. The example's first version (presenting and resizing, before input) was also run by hand on GTK and Kirigami under Sway, and works on both. The test suite runs natively on GTK and Kirigami, on their private displays and under Sway on Wayland and on X11 (Xwayland: `GDK_BACKEND=x11`, `QT_QPA_PLATFORM=xcb`); Kirigami on X11 takes about 15 seconds, and once `takes_keys_while_focused` got other keys there, which didn't happen again in five runs. Sway decorates Qt's windows itself, so Qt's `frameMargins` with its own decorations (GNOME, say) is still unverified. The example's first version was run on WinUI too, resized from a script, with its child window above XAML's content island at the canvas's place and size. On WinUI the example was then driven by injected input (`mouse_event`, `keybd_event` with scan codes) and tried by hand (below): the circle follows the pointer, a click captures, locked moves move it, keys come with and without the grab, Control+Alt lets go, and the wheel scrolls. Injected keys without a scan code come as `Unidentified`, as the scan code is what's reported. The native test suite passes there too (`follows_its_frame` once the settle placed surfaces). Unverified until they run with real input (the tests synthesize it, and a test window may not be the active one): the X11 child window's position with client-side decorations and `QT_SCALE_FACTOR`, and a Qt window's XID when first shown; that relative motion and the lock's `unlocked` arrive on our own thread, and GTK's `invoke` and Qt's wake are quick enough for motion; GTK's controller order (a capture-phase controller added to the window after GtkWindow's own, the legacy controller getting every button, `shortcuts-inhibited` notifying on both display servers); Qt's `ShortcutOverride` reaching the focused item before the shortcut map, an accepted Tab stopping Qt Quick's focus chain, `nativeScanCode` being the XKB code on xcb and Wayland, `setKeyboardGrabEnabled` taking the window manager's shortcuts; and on WinUI, a handled Tab stopping XAML's focus navigation, `KeyDown` coming for Alt and the Windows keys, and `ShowCursor(FALSE)` hiding the cursor (a screen capture doesn't show it). The cursor and the key releases are only type-checked on GTK, Kirigami and WinUI; unverified: texture cursors' size on HiDPI Wayland and X11, the hotspot's scale on Qt and whether an item's cursor applies over the input item when it takes no input, and on WinUI that the child window's `WM_SETCURSOR` shows the app's cursor over a surface that takes input, and over one that doesn't, that the island's input window gets `WM_SETCURSOR` in a subclass before XAML, that XAML doesn't set the cursor again on pointer moves, that a straight-alpha 32-bit cursor shows right, and that `EnumChildWindows` finds the island's window when the surface is attached. On AppKit the cursor's image, none and the arrow were checked through `native_state` only: tests can't rest a pointer on the view; the example is for trying it.

### M2 (GTK 4)

What the GTK 4 backend taught us:

- **Tests run on a private Broadway display.** The runner starts `gtk4-broadwayd` (bound to localhost) and points GDK at it, unless `MITSUAMI_SHOW_WINDOWS=1`. GTK needs a real, mapped window to capture and to track focus, and on the session display a tiling compositor would override window sizes. The display prints its address, so tests can be watched in a browser.
- **Windows get an explicit `HeaderBar`.** GTK's default size includes the titlebar. With a header bar of our own, its height is known, and the content gets exactly the size the core asks for.
- **Layout hosts allocate each child at its core frame**, and ask for exactly their own frame (window content hosts ask for nothing, so windows can shrink). Frames live in one map shared by all hosts, so a child keeps its frame when it moves to another parent (the core only resends frames that change). Leaves with an empty frame are hidden from GTK's allocation: controls can't be allocated smaller than their padding. A frame narrower or shorter than the child's minimum (a percentage width, a stretch) is allocated at the minimum and overflows, as in a GTK box: GTK warns otherwise ("Trying to measure GtkGizmo for width of 147, but it needs at least 152", a progress bar's trough in the measurements example's cards).
- **The Tab order is the window content host's `focus` vfunc.** Tab and Shift+Tab walk the core's order and wrap around; arrow keys keep GTK's geometric behaviour. The Tab conformance tests were confirmed to fail with GTK's own order.
- **Programmatic changes are muted.** GTK emits `toggled`, `notify::active` and `changed` for `set_active`/`set_text` too, so events are dropped while the backend applies commands.
- **Input is synthesized with keybinding signals** (answering the M1 open question): GTK 4 can't inject key events, so keys become the signals they are bound to, on the widgets that handle them: `insert-at-cursor`, `backspace` and `activate` on the entry's text widget, and `move-focus` on the window for Tab.
- **Button presses call `clicked` directly.** `gtk_widget_activate` would click only after the press animation, asynchronously.
- **Min-content text works here:** a wrapping `GtkLabel` reports its longest word as its minimum width.
- **Captures use the Cairo renderer**, whichever renderer the display uses, so baselines don't depend on the GPU. The content host carries the `background` style class, so captures include the window background.
- **Text styles map to GNOME's type scale** (`title-1`, `title-2`, `heading`, `caption`, `monospace`). GNOME has no callout size, so callouts use the body size.
- **Focus is tracked on the window** (`notify::focus-widget`), resolved to the nearest known node: an entry's focus sits on its inner text widget.
- **Scroll views are a `ScrolledWindow` around a `Viewport`.** Content hosts measure as their frame, so the viewport learns the content size. The adjustments are updated as soon as frames arrive, because a `ScrollTo` in the same commit needs the new range before GTK allocates.
- **Capture replies from the frame clock** (`after-paint` of the next frame), and the test executor lets the backend run while a test awaits. `TestHooks::settle` was added to the contract for this: platforms that complete work asynchronously catch up there.
- **Broadway frames stall without a browser** after a paint, until something new is drawn, so nothing may wait for two frames in a row.
- **The test display has portals off** (`GDK_DEBUG=no-portals`): otherwise file dialogs open on the real desktop, and its dark mode and fonts leak into tests.
- **Menus go in the header bar** (GNOME's primary menu button): one labelled section per app menu, then Quit (Ctrl+Q, which asks every window to close). Menus are covered in § Menus. Shortcuts are installed in every window. GTK's text widgets have their own Cut/Copy/Paste context menus, so there's no Edit menu.
- **Alerts** use `gtk::AlertDialog`: Escape chooses the last button. GTK has no alert styles, so `AlertStyle` is ignored.
- Not done yet: a reduced GTK 4.8 mode, and `gtk::Application` integration (single instance, app ID). `run` drives a plain GLib main loop.

### M4 on GTK

- **Custom widgets and native views on GTK** follow AppKit's shape: `mitsuami_gtk::NativeRender` (a `gtk::Widget` per render, measured by GTK unless the render measures itself), `NativeView::gtk(factory)`, and `mitsuami::gtk::gtk` for the bindings. GTK signals fire on programmatic updates, so custom events are muted while the backend applies props, like `Changed`.
- **Drawn widgets are a `DrawingArea` rasterized with Cairo.** Semantic colors come from the theme's named colors (`accent_color`, `borders`, …), with Adwaita's values as a fallback, so they follow the theme. Pointer events come from a click gesture; `SyntheticInput::Click` emits the gesture's own `pressed`/`released`, since GTK 4 can't inject pointer events.
- **Native views' accessibility actions** do what GTK's do: `activate` for Activate, and a step for spin buttons and ranges.
- **Only real platform controls count as native.** GTK has no rating control, so the example's rating is built ad hoc there, from flat buttons with `starred-symbolic` icons like GNOME Software's, and isn't labelled native. GTK's own widget in the example is `GtkLockButton`.
- **`GtkLockButton` is deprecated since GTK 4.10** and gone in GTK 5, but it's in every GTK 4. It shows a `GPermission`, and gtk4-rs can't subclass one, so the example registers a small one through GIO's C API: it reports what the props say, and acquiring or releasing just succeeds. The button's click is the request the app answers, like any controlled widget.
- **Focus requests wait for the structure.** `Ui::focus` right after building a node (a composed field focusing itself) used to reach the backend before the node was in a window, where no toolkit can focus it; headless didn't mind. Focus commands now go at the end of the batch's structure.


### M4 on WinUI

- **Custom widgets and native views on WinUI** follow the same shape: `mitsuami_winui::NativeRender` (any XAML element, from `mitsuami::winui::bindings` or bindings of the app's own), `NativeView::xaml(factory)`, and `WinUiCx` with an `Emitter`. windows-rs unsubscribes when an event's `EventRevoker` drops, so `cx.keep(revoker)` ties a subscription to the node.
- **Native renders and native views sit in a `Border`** that carries the core's frame, with the control inside. Many XAML controls size themselves (`RatingControl` sets its own `Width` once its template applies), which desynced frames when the frame went on the control. Accessibility props, focus and UI Automation go to the control inside.
- **Observe properties, not change events.** `RatingControl.ValueChanged` isn't raised for values a screen reader sets through UI Automation, so a rating set that way never reached the app. `WinUiCx::observe` registers a dependency-property callback instead: it sees every change, and it runs synchronously, so the backend's own prop updates are muted (events raised asynchronously, like `PipsPager`'s `SelectedIndexChanged` after creation, slip past muting).
- **Drawn widgets are XAML shapes built from markup.** The display list becomes a canvas of `Path`s loaded with `XamlReader`. Semantic colors become `{ThemeResource …}` brushes (`TextFillColorPrimaryBrush`, `AccentFillColorDefaultBrush`, …), so they follow the element's theme live. Every shape is a path, so strokes are centred on the outline as on the other platforms. Pointer presses and releases on the canvas become `Pointer` events; `SyntheticInput::Click` emits them directly.
- **Native views' accessibility actions** go through the control's UIA patterns: Invoke or Toggle for Activate, RangeValue for Increment/Decrement, Value (or RangeValue) for SetValue.
- **WinUI's controls in the example:** `PipsPager` is WinUI's own, and so is `RatingControl`, so the rating is native on Windows as well as on macOS. WinUI has no lock button, so the lock is composed there.

### KDE Plasma (Qt Quick and Kirigami)

The spike (`spikes/kirigami`) settled the route; the backend is `crates/mitsuami-kirigami`.

- **A C++ layer, not bindings.** Qt has no maintained Rust bindings for driving arbitrary QML items (`cxx-qt` exposes Rust objects to QML, the other way round). `cpp/shim.cpp` is a C API over `QObject*`: items are created from one line of QML each (`QQC2.Button { }`, compiled once per text), properties go through `QObject::setProperty`, and every signal reaches Rust through one callback with a key naming a Rust closure, dropped with its object. `build.rs` finds Qt with pkg-config and runs moc on the one header that declares QObjects. The crate builds empty without its `qt` feature, so the workspace builds where Qt isn't installed.
- **Picked by a feature.** `mitsuami`'s `kde` feature swaps GTK for Kirigami (and wins if `gtk` is on too); the test kit has its own `kde` feature; `platform!` has `kde` and `gtk` arms, settled when `mitsuami` is built.
- **Windows are a `Kirigami.ApplicationWindow` with one page,** whose title shows in Kirigami's toolbar; the page's content item is the layout host. The toolbar's height is only known once Kirigami's page stack is laid out, which Qt does when it polishes before a frame. Resizing the window until the content matched overshot (the host lags a polish behind) and shrank the window to a pixel. The backend now polishes the window's items first, measures the toolbar, and checks again at the first frame; windows are sized in whole pixels.
- **Breeze outside Plasma.** Controls use the `org.kde.desktop` style, which draws with the app's QStyle. On Plasma, the KDE platform theme picks that style; anywhere else (Sway, GNOME, the tests), Qt falls back to Fusion: square buttons, arrowed scroll bars, mnemonics always underlined. KDE apps like Kate then pick Breeze themselves (`KStyleManager`), and so does the backend, unless `QT_STYLE_OVERRIDE` names a style.
- **Items are born with their implicit size.** A Qt Quick item's size follows its implicit size until one is set, so the backend zeroes every frame on creation (the rule AppKit taught, §16).
- **Measuring is synchronous:** `implicitWidth`/`implicitHeight`, even for the desktop style's QStyle-drawn controls. Wrapping text measures its height by setting `width`; min-content is `Text.WordWrap` at width 1, which leaves the longest word as `contentWidth`.
- **User-only signals.** `toggled` and `textEdited` fire only for the user, so built-in controls need no muting. But a `SpinBox` stepped by a screen reader (`Increase`) changes its value without `valueModified`: native views observe `valueChanged`, and the backend mutes its own updates, as on WinUI.
- **Accessibility actions are Qt's own:** `Press`, `Toggle`, `Increase` through `QAccessible`, as Orca's would. They also focus the control, as a click does, unlike AppKit's press; the focus change is reported like any other. Disabled controls accept them and do nothing, so the backend checks `enabled` first.
- **Real input.** Qt can inject key and mouse events: typing, Backspace, Return, Tab and clicks on drawn widgets are real `QKeyEvent`s and `QMouseEvent`s, where GTK 4 needed keybinding signals.
- **The Tab order is an event filter** on the window: Qt Quick's chain follows item order within each parent.
- **Tests run on Qt's offscreen platform** (no display server, exact window sizes on a 1920×1080 screen, software rendering, so captures don't depend on the GPU). The desktop's settings stay out: no platform theme, a private `kdeglobals`, Plasma's default font. Light and dark are Breeze Light and Breeze Dark, switched the way KDE apps' color scheme menus do (`KDE_COLOR_SCHEME_PATH` and a palette change). The private `kdeglobals` turns animations off (`AnimationDurationFactor=0`): captures caught a switch's knob mid-slide.
- **Captions.** Without Plasma's platform theme, Kirigami's small font falls back to a 12 pt system font, larger than the body; captions are then 0.8 × the body, Plasma's ratio.
- **Teardown.** A backend can be dropped with the thread-locals that hold it as the process exits, after Qt's thread data or KDE's icon loader has gone: destroying a window then crashed. Dropped backends post their windows' deletion instead, a new backend flushes what earlier ones posted (their controls still held Kirigami's Alt-key mnemonics, which moved the underlines in the next test's capture), and at exit a C++ thread-local sentinel drops what is still queued.
- **Qt logs to the systemd journal when stderr isn't a terminal,** so QML warnings vanish from piped or captured output (a warning-free run that wasn't): `QT_FORCE_STDERR_LOGGING=1` puts them back on stderr.
- **Kirigami wants popups whole from the start.** A global drawer created without a parent reads the parent it doesn't have yet, and one attached to a finished window (or given its actions late) makes the hamburger button report a binding loop. Windows get their menu drawer in their own QML (menus are set before the commit that creates windows); a menu whose structure changes later still gets a drawer created in the window's overlay, and Kirigami's warning. A `PromptDialog` opened before its window's first frame loops over its position, so alerts wait for that frame. Kirigami's dialog binds its `y` to its `height`, and setting `y` lets Qt resize a popup that doesn't fit or whose implicit height has changed (`QQuickPopupPositioner::reposition`), so the binding loops whenever the dialog's implicit height changes as it slides in or out (a window shrinking, text rewrapping). Alerts bind `y` to the implicit height instead, which Qt places the popup by and never writes, and keep it inside the window. Checked in a QML reproduction that flips the implicit height while the dialog opens and closes (Kirigami's binding and the previous one loop, this one doesn't); the warning seen in `examples/menus.rs` wasn't reproduced directly.
- **Qt's own controls in the example:** the lock is a `DelayButton` and the pager a `PageIndicator`, both native; the rating is built ad hoc from tool buttons with Breeze's star icons, as Discover does.
- **Known gaps.** Plasma's look needs the Breeze widget style installed; without it the desktop style falls back to Fusion, which doesn't show default buttons. Breeze has no destructive button style. The desktop style's scroll bars take room from the view, which the core doesn't know about, so a vertical bar covers the content's right edge. Windows show no menu button when the app has no menus of its own, so Quit (Ctrl+Q) needs one. `FolderDialog` picks one folder.

## 17. Implementation notes (M0.5 WinUI spike)

The spike lives in `spikes/winui` and is not a workspace member. It builds a small UI and checks each operation the backend will need:

- It creates a window, a `Canvas` host, a `Button` and a wrapping `TextBlock`, and places them at our frames.
- It measures them, clicks through UI Automation, and captures the root to a PNG.

What it established for M3:

- **Bindings.**
  - `windows-bindgen` 0.100 generates everything from three sources: the `Microsoft.WindowsAppSDK.WinUI` and `.InteractiveExperiences` NuGet metadata, plus the built-in Windows metadata. The output is about 2.3k lines, against reactor's 29k.
  - `--compose` (needed for `Application`) requires `--minimal`. Filters therefore list members (`IUIElement::{Measure, get_DesiredSize}`), and calls go through `cast::<IUIElement>()` and friends rather than inherited methods.
  - Struct fields are snake_case (`Size { width, height }`).
- **Bootstrap.**
  - Framework-dependent bootstrap works the way reactor does it: `TryCreatePackageDependency` + `AddPackageDependency` on `Microsoft.WindowsAppRuntime.2_8wekyb3d8bbwe`, with a minimum version of 2.4. That resolves to any installed 2.x at or above it (2.5.1 on the dev machine).
  - The composed `Application` must merge `XamlControlsResources` and forward `IXamlMetadataProvider` to `XamlControlsXamlMetaDataProvider`. Otherwise controls have no templates.
- **Measure needs the live tree.**
  - Before insertion, a `Button` measures `0 × 19` because its template isn't applied yet. It still does once inserted, until the window has loaded.
  - Once the root has loaded, any element appended to a live `Canvas` measures correctly right away, before its own `Loaded`.
  - So the first render waits for the root's `Loaded`. After that, it's fine to measure right after `Insert`.
- **Our frame size wins over content.**
  - `SetFrame` is `Canvas.SetLeft/Top` + `Width`/`Height`, and XAML's `Measure` then returns that explicit size.
  - The backend's `measure` therefore sets `Width`/`Height` to NaN (Auto), measures, and restores the frame.
  - With that, measurement after a content or text change is synchronous and correct, with no layout pass in between.
- **Text.**
  - Wrapping under a definite width works: at width 120 the label measures `115.3 × 56`.
  - At width 0 it wraps per character (`0 × 782`). Min-content text needs its own approach, such as measuring the longest word, like on AppKit (§16).
- **Units.**
  - Measured values are logical and already snapped to device pixels at the rasterization scale. At 1.5, a width of `77.33` is 116 px.
  - `AppWindow` sizes (`ResizeClient`) are physical pixels and must be scaled.
- **Events.**
  - Handlers are Rust closures (`IButtonBase::Click(|_, _| …)`). The returned `EventRevoker` unsubscribes on drop, so the backend owns the revokers for each node.
  - `IInvokeProvider::Invoke` on the element's automation peer raises `Click` synchronously. That is what the test driver's `Activate` needs.
  - `DispatcherQueue::TryEnqueue` is the run-loop flush hook.
- **Capture.**
  - `RenderTargetBitmap::RenderAsync(root)` + `GetPixelsAsync` works.
  - Completions arrive on the UI thread, but `IAsyncAction::when` wants `Send`, so XAML objects travel through a thread-local.
  - `PrintWindow` yields black, because the content is drawn by composition.
  - The capture follows the system theme, so tests must force the light theme, as on AppKit.
- **Shutdown.** All XAML references must be released before `Application::Exit`. Dropping them later, for example from a thread-local destructor, fails fast with `STATUS_STACK_BUFFER_OVERRUN`.
- **Toolchain.**
  - windows-rs 0.100 links through `raw-dylib`. On `x86_64-pc-windows-gnu` that needs `dlltool`, which rustc doesn't find (the toolchain ships one under `self-contained/`, but rustc doesn't look there), and `windows-reactor-setup` rejects plain gnu anyway.
  - The Windows backend therefore targets `x86_64-pc-windows-msvc` (or gnullvm).
  - Cargo can't require a target per host (`rust-toolchain.toml` pins only the channel, `build.target` applies to every OS, `forced-target` is nightly-only). `mitsuami-winui` pulls its Windows dependencies only for `target_env = "msvc"`, and other Windows toolchains get a `compile_error!` that names the fix: `rustup set default-host x86_64-pc-windows-msvc`, or `cargo +1.96-x86_64-pc-windows-msvc`.

## 18. Implementation notes (M3)

What the WinUI backend (`crates/mitsuami-winui`) does beyond the spike, and why:

- **Bindings are generated once and checked in** (`src/bindings.rs`, about 7k lines), by the tool in `crates/mitsuami-winui/bindgen/` from the member list in its `filter.txt`. The crate needs neither network access nor `windows-bindgen` at build time. They are public as `mitsuami_winui::bindings`, the escape hatch for native access.
- **No `Application::Start`.** The backend composes the `Application`, then calls `WindowsXamlManager::InitializeForCurrentThread` (in that order: the other order fails), and pumps messages itself. `run()` is a plain `PeekMessage` loop that ticks before `MsgWaitForMultipleObjectsEx` sleeps until the next timer, like AppKit's run-loop observer. Ticks are also scheduled on the `DispatcherQueue`, which keeps running inside modal loops (live resizing) that bypass ours. Tests use the same pump.
- **Windows are live before anything is measured.** `Create(Window)` activates the window right away, layered and nearly transparent, and waits (pumping) for its content's `Loaded`. The window becomes visible after the first layout. Test windows stay that way: alpha 1 rather than 0, because the compositor skips fully transparent windows and XAML's rendering (captures) stalls. They are click-through and hidden from the taskbar.
- **XAML reports asynchronously; the core expects prompt reports.** `GotFocus`, `TextChanged`, `ViewChanged`, `SizeChanged` and `Checked` arrive after the call that caused them. The backend reports what it causes itself right away: focus it moves, values it sets for tests, scrolls it applies with `UpdateLayout`, and window sizes it sets. The late XAML event then finds its value already reported and is dropped. Focus moves XAML makes on its own (a focused control is disabled) are picked up when tests pump.
- **Title bar.** The Win32 caption ignores the app's theme, so windows extend their content into the title bar and host WinUI's `TitleBar` control, with the caption buttons set to follow the theme (`AppWindowTitleBar.PreferredTheme`). The content size then needs care: `ResizeClient` sizes the area below the caption strip while `ClientSize` and XAML's root include it, and Windows keeps a 1 px resize border inside the client area. `resize_client` corrects by what `ClientSize` reports and by the border measured off the live root.
- **Tab order.** `TabIndex` is scoped to each container, so it can't express a window-wide order across nested hosts. The window root handles Tab in `PreviewKeyDown` and moves along the core's order itself.
- **Theme resources come from markup.** The window root (menu-bar row, content host, background) is built with `XamlReader::Load`, so `{ThemeResource …}` follows the element's theme (tests force light) and live system theme changes. A brush looked up in code follows the app theme instead.
- **Completions and threads.** XAML operations (`RenderTargetBitmap`, `ContentDialog`) complete on the UI thread. The WinAppSDK file pickers and the clipboard complete on worker threads, so they hop back through the `DispatcherQueue`. `IAsyncOperation::when` wants `Send` closures, so replies wait in a thread-local.
- **Test hooks.** XAML reports asynchronously, so the WinUI backend implements `TestHooks::settle` (added for GTK, which needed the same) by pumping messages and picking up focus moves XAML made itself; the test executor calls it while a test awaits native work, such as a capture.
- **Known gaps.** Min-content text falls back to max-content, as on AppKit. `Destructive` buttons look like `Default`, because Fluent has no destructive style. Self-contained deployment (`windows-reactor-setup`) isn't wired up: apps need the Windows App Runtime 2.4+ installed. Windows have no app icon: the `TitleBar` control shows none, where the Win32 caption showed the executable's.

## 19. Open questions

- **App icon.** An app-level setting (`App::icon(…)`) rather than a `Capability` (§11): every backend can show one, but where and how differ. WinUI shows it in the `TitleBar` (`IconSource`) and on the taskbar (`AppWindow.SetIcon`). AppKit shows it in the Dock, normally from the bundle, with `applicationIconImage` for unbundled apps. GTK takes an icon name, and on Wayland only the `.desktop` file supplies one. Still open: the source format (one PNG set, or per-platform assets) and whether bundling tools (`cargo mitsuami`) own it.
- The exact Windows floor for Windows App SDK 2.4. The InteractiveExperiences metadata still ships a 10.0.17763 variant, which suggests 1809 holds.
- Visual baseline storage: git LFS is fine to start with. Revisit when baseline volume grows (3 platforms × variants × stories).
