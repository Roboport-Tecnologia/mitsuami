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
- **How a list sits in its surroundings is a semantic choice** (`List::list_style`), like `ButtonStyle`: `Plain` (edge to edge: sidebars, main content) or `Framed` (a bordered box on the content background: a list in a form or dialog), each drawn the platform's way: Breeze's scroll view frame on Kirigami, `ScrolledWindow` `has-frame` on GTK, the bezel border on AppKit, a card's border on WinUI. `Automatic`, the default, is `Plain` everywhere. Where platforms' habits differ (KDE frames more lists than GTK or macOS), the app picks per platform with `platform!`: `.list_style(platform! { kde => ListStyle::Framed, _ => ListStyle::Plain })`. A frame takes room from the rows, which backends report with `RowWidth`. Run on GTK and Kirigami; on AppKit and WinUI only type-checked so far.

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

### Responsive / adaptive

- `use_viewport()` returns a signal of the window size, like media queries.
- `use_container_size(node_ref)` works like container queries.
- `Platform::current()` is also available at runtime, but compile-time `platform!` (§6) is preferred.

---

## 4. Styling (what can and can't be styled)

A style has two halves:

1. **Layout:** the full flex and grid property set (`direction`, `gap`, `padding`, `margin`, `align_*`, `justify_*`, `grow`, `shrink`, `basis`, `grid_template_*`, `grid_area`, `position`, `inset`, `min/max/size`, `aspect_ratio`, `overflow`). It applies to every node.
2. **Semantic visual:**
   - Text styles: `TextStyle::{LargeTitle, Title, Headline, Body, Callout, Caption, Monospace}`. These map to `NSFont.preferredFont(forTextStyle:)`, the WinUI type ramp, and GTK/libadwaita style classes.
   - Button roles and styles: `ButtonRole::{Normal, Default, Cancel, Destructive}` (what the button does: Return clicks the default one, Escape the cancel one, where the platform does that) and `ButtonStyle::{Automatic, Bordered, Borderless}` (how it's drawn). Each platform maps them its own way (`keyEquivalent` and `bordered` on macOS, `suggested-action` / `destructive-action` and `has-frame` on GTK, `Accessible.defaultButton` and `flat` on Qt, `AccentButtonStyle` and `SubtleButtonStyle` on WinUI) and ignores what it has no equivalent for.
   - Past the semantic props, a widget's `.native(tweak)` sets raw platform settings (§6.4).
   - Semantic colors (`Color::Label`, `Color::SecondaryLabel`, `Color::Accent`, `Color::Separator`, …) that follow dark mode and high contrast.
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
- The widget's type names the native one (`Tweakable`): `Button` is `NSButton`, `gtk::Button`, a `QQC2.Button` item (`QmlObject`, set by property name) and XAML's `Button`, whose closure returns a `windows_core::Result`; `Checkbox` is `NSButton`, `gtk::CheckButton`, a `QQC2.CheckBox` item and XAML's `CheckBox`; `Switch` is `NSSwitch`, `gtk::Switch`, a `QQC2.Switch` item and XAML's `ToggleSwitch`; `Select` is `NSPopUpButton`, `gtk::DropDown`, a `QQC2.ComboBox` item and XAML's `ComboBox`; `Slider` is `NSSlider`, `gtk::Scale`, a `QQC2.Slider` item and XAML's `Slider`; `Progress` is `NSProgressIndicator`, `gtk::ProgressBar`, a `QQC2.ProgressBar` item and XAML's `ProgressBar`; `Spinner` is `NSProgressIndicator`, `gtk::Spinner`, a `QQC2.BusyIndicator` item and XAML's `ProgressRing`; `TextInput` is `NSTextField`, `gtk::Entry`, a `QQC2.TextField` item and XAML's `TextBox`; `PasswordInput` is `NSSecureTextField`, `gtk::PasswordEntry`, a `Kirigami.PasswordField` item and XAML's `PasswordBox`; `ScrollView` is `NSScrollView`, `gtk::ScrolledWindow`, a `QQC2.ScrollView` item (its `contentItem` is the `Flickable`) and XAML's `ScrollViewer`; `List` (its type's defaults make `Tweak<List>` name it) is the list view inside the scroll view: `NSTableView`, `gtk::ListView`, the QML `ListView` item and XAML's `ListView`; `Text` is `NSTextField`, `gtk::Label`, a `QQC2.Label` item and XAML's `TextBlock`.

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
| Image | NSImageView | Image | gtk::Picture | QML Image |
| ScrollView | NSScrollView | ScrollViewer | gtk::ScrolledWindow | QQC2.ScrollView |
| List (virtualised) | NSTableView | ListView | gtk::ListView | ListView |

**What comes next is driven by 2ksbox.** mitsuami was started to replace the Qt Quick launcher of 2ksbox (a Windows 98/XP emulator; `launcher-qt/qml` in that repo). Widgets are added as that launcher needs them, and only widgets every platform has a native control for: what one platform lacks is the app's to build, as a custom widget (§6.3). From the launcher so far:

- Built: `NumberInput` (its `SpinBox`), `Image` (its shader preview), tooltips (`.tooltip(text)` on any widget, for its elided status line).
- Built, though not widgets: windows opened while the app runs, modal or not (`Window`, §16; every secondary window of the launcher is an application-modal dialog over it); Escape closing its dialogs (the launcher's `Shortcut`s), as a modal `Window` does (§16); its header, whose download progress and status line are items of the window's `Toolbar` (§16); menus, which 2ksbox plans to add: submenus, check and radio items, reactive titles and items, roles, and a window's own menus in `view!` (§16).
- Left to the app: a separator line (WinUI has no separator control outside menus and app bars) and a disclosure header (Qt Quick has none; 2ksbox builds its own from a `ToolButton`).

**Idiomatic shell components (post-MVP).** These are where most of the "feels native" effect comes from:

- `AppShell`, `Sidebar` (source list / NavigationView / split view)
- `Toolbar` (NSToolbar / CommandBar / HeaderBar): items at its trailing end are built (§16)
- `MenuBar` (the global menu on macOS; an in-window menu or hamburger elsewhere): built (§16)
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
| Linux | GTK 4.10, or GTK 4.8 in reduced mode | Chosen at build time through a cargo feature (`gtk_v4_8`, `gtk_v4_10`, `gtk_v4_12`, …). libadwaita versions work the same way under the `adwaita` feature. |
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
- The image is `MITSUAMI_IMAGE`, which CI sets to its pinned runner image (`macos-15`, `ubuntu-24.04`, `windows-2025`), or for the KDE job to its Arch Linux container, pinned to a day of the Arch Linux Archive (`archlinux-2026.09.20`). Otherwise it's the OS and its version (`macos-26`, `ubuntu-24.04`, `windows-26100`). A scale other than 1× is appended (`macos-26@2x`), since captures are in physical pixels. A developer's baselines and CI's live side by side, and each machine only compares with its own.
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
| GTK flavour | Plain gtk4 core, with an optional `adwaita` feature for shell components and style classes |
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
- **Not run yet on GTK, WinUI and Kirigami:** written and type-checked on macOS; CI runs them.

### Slider and Progress

- **A step means what the platform's step means.** AppKit's is tick marks that the knob only stops at (`allowsTickMarkValuesOnly`); WinUI snaps drags of the knob to `StepFrequency` (not values set through automation or by the app) and steps by `SmallChange`; Qt's sliders only move by it from the keyboard (`snapMode` stays off unless asked). GTK's scales have no stepped mode at all, so the backend snaps the user's moves to the step in `change-value` (see below). Without a step each keeps its default: WinUI's is 1, which it snaps to, and Qt's `increase()` moves by 0.1. GTK needs one to move at all, so it gets a tenth of the range. Tests only check which way a step moves.
- **The value follows the range.** The platforms clamp the value to the range, so a range sent after the value would lose it: the core queues the value again after every range change.
- **A clamp while the range is set isn't the user's.** XAML reports it from inside the range's setter: a new `Slider` starts at 0 of 0–100, so a range of 120–480 moved it to 120 and WinUI reported that as a move, overwriting the app's value (the text example's width). The backend now expects the clamped value before it sets the range, for `Slider` and `NumberBox` alike.
- **Sliders and progress bars have no natural width on AppKit** (no intrinsic width): they're as wide as the layout makes them, stretched in a column. The others measure theirs, but WinUI's slider measures only its thumb (18 wide): XAML apps stretch sliders or size them. Give a slider in a row `grow` or a width.
- **Indeterminate progress animates as the platform animates it:** `NSProgressIndicator` and XAML's and Qt's bars on their own, GTK's by `pulse()` calls, which a 100 ms timer makes while the bar is indeterminate, as GTK apps do.
- **Stepping and setting do what assistive technology or the keyboard does:** VoiceOver's increment, and the action as for a drag; GTK's step on the adjustment; UIA's RangeValue pattern; and on Qt the keys' `increase()` or `decrease()`, then `moved()`, the user's signal.
- **Not run yet on GTK, WinUI and Kirigami:** written and type-checked on macOS; CI runs them.

### Button roles, styles and tweaks

- **`ButtonVariant` became a role and a style,** since what a button does and how it's drawn are separate choices (a borderless destructive button). Both are sent only if the app picks one, and backends keep them on the node: no toolkit reads every role back.
- **Cancel is Escape on AppKit only.** GTK, Qt and WinUI have no cancel button outside their dialogs, so it's a normal button there. Default is Return only on AppKit so far; GTK's default widget isn't set yet.
- **Qt's default button is `Accessible.defaultButton`,** which the desktop style turns into `QStyleOptionButton::DefaultButton`: Breeze tints it, except when flat. `highlighted` only draws the focus frame there, so it showed nothing. Screen readers hear the default button too. Qt Quick has no Return-clicks-default outside dialogs.
- **Default buttons draw their accent only in the key window** on AppKit, so test captures (never key) show them grey. A `bezelColor` tint didn't show in captures either, so the story's AppKit tweak is a large control size.
- **Tweaks run inside `apply`,** with events muted where the backend mutes them. `Tweakable` lives in each backend, which now depends on `mitsuami-widgets`.
- **Per-widget examples:** `examples/button.rs` shows every role in every style, a playground of the semantic props, and one tweak that differs per platform. Other widgets get one each as they're refactored.
- **Run on AppKit and Kirigami:** GTK and WinUI are only type-checked, and CI hasn't run them; `SubtleButtonStyle` on WinUI has never run.

### Checkbox: the mixed state and tweaks

- **`Checkbox::mixed`** shows the mixed state over `checked` (some of what the box stands for is checked, as in "Select all"). Every platform has it: `NSControlStateValueMixed`, GTK's `inconsistent`, Qt's partly checked `checkState`, XAML's null `IsChecked`. The a11y tree has `mixed` beside `checked`.
- **A click leaves the mixed state, and lands where the platform lands:** AppKit checks the box (run); Qt's cycle from partly checked goes to checked; XAML's toggle from null goes to unchecked, as far as its toggle rule goes; GTK flips `active` underneath. The box reports what it landed on, the core absorbs `Mixed(false)`, and the app works out `mixed` again. Headless checks the box. Tests only check that the box leaves the mixed state and reports where it went.
- **Clicks never go back into it.** AppKit only allows the mixed state while it's shown, and Qt's `tristate` (which a partly checked `checkState` turns on) goes off after a click, so user clicks can't cycle into it. GTK leaves `inconsistent` to the app, so the backend clears it on a user toggle, as GTK apps do.
- **`Checked` is kept underneath while mixed:** AppKit and Qt keep it on the node (the control has one state), GTK's `active` holds it, and WinUI keeps it in `shown_checked`; leaving the mixed state always emits, whatever it lands on.
- **Tweaks as for buttons.** The example and story tweaks: the box after its label on AppKit (`imagePosition` trailing, captured), GTK's `selection-mode` class (a round check where the theme has one), Qt's `spacing`, a round box on WinUI (`CornerRadius`).
- **Run on AppKit only:** GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them; where Qt and WinUI land from the mixed state is from their documented toggle rules, not seen.

### Switch: tweaks only

- **No semantic options past `checked`.** What the platforms offer isn't shared: control sizes on AppKit, GTK's `state` apart from `active` (for settings that take time to apply), on and off text on WinUI, a caption Qt's switch draws itself. So `Switch` only gets `.native(tweak)`, and the example's tweak is one of those per platform: a small control on AppKit (captured), a delayed state on GTK, the switch's own text on Qt, `OnContent`/`OffContent` on WinUI.
- **A tweak that connects a signal must guard itself,** since tweaks run again when props change: the GTK tweak marks the switch with a widget name.
- **Run on AppKit only:** GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them.

### Select: tweaks only

- **No semantic options past the options and the choice.** The nearest, a borderless select for toolbars, is AppKit's `bordered` and Qt's `flat` only: GTK's theme has no flat dropdown (GTK 4.14's `_common.scss` flattens dropdowns only inside `.toolbar`) and `GtkDropDown` has no API for one, and WinUI's `ComboBox` has no such style. Editable combo boxes are another widget on AppKit (`NSComboBox`) and missing from `GtkDropDown`. So `Select` only gets `.native(tweak)`.
- **The example's tweaks:** a borderless pop-up on AppKit (captured; it's measured narrower), search in the pop-up on GTK (`enable-search`), `flat` on Qt, a `Header` on WinUI (`IComboBox::put_Header` added to the bindings).
- **Tweaks run after the options too:** replacing them re-runs the tweak, like any prop change.
- **Run on AppKit only:** GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them.

### Slider: orientation and tweaks

- **`Slider::orientation`** is the one semantic option every platform's slider has: AppKit `vertical`, GTK's `orientation`, Qt's `orientation`, XAML's `Orientation` (bindings added). Tick marks (not on Qt's), a drawn value (GTK only) and reversed direction (not on AppKit or Qt) aren't shared, so they're tweaks.
- **Up is more, everywhere.** AppKit, Qt and WinUI put the minimum at the bottom; GTK puts it at the top, and GTK apps set `inverted` to turn that round, so the backend does too.
- **Length comes from the layout.** A vertical slider is as tall as its container makes it (AppKit's have no natural height, as its horizontal ones have no natural width); headless measures one 20 × 160.
- **The example's tweaks:** a circular slider on AppKit (captured, sized at its natural size), `draw-value` on GTK, `snapMode` on Qt, tick marks on WinUI (`TickFrequency`, `TickPlacement` added to the bindings).
- **Run on AppKit only:** GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them.

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
- **Run on AppKit only:** GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them.

### Spinner

- **A widget of its own, not a style of `Progress`:** every platform has a spinner for work of unknown length, but on GTK, Qt and WinUI it's another control than the bar, and GTK's and Qt's show no value.
- **`running`, on by default.** Stopped, every platform's spinner shows nothing (AppKit's with `displayedWhenStopped` off, which the backend sets) and keeps its size, so nothing around it moves; apps hide it with `Show` to give the room back.
- **It reads as a progress bar without a value** (`Role::ProgressBar`), as ARIA has it and as GTK and WinUI report theirs; AppKit reports a busy indicator natively. Named by its label; takes no focus.
- **Sized as the platform sizes it:** AppKit's regular spinner is 32 pt, GTK's 16 px, Breeze's two grid units, WinUI's 16 (its style's minimum); headless measures 16 × 16. A `ProgressRing` has no size until XAML loads it and applies its template, at the frame after it's added: the backend asks the core to measure it again on `Loaded`, and waits for it in `settle`. Before, spinners on WinUI stayed 0 × 0 until something else moved them.
- **The example's tweaks:** a small spinner on AppKit (captured), a 32 px size request on GTK, a larger implicit size on Qt, a determinate ring on WinUI (`IsIndeterminate`, `Value` added to the bindings), which only WinUI's spinner can show.
- **Run on AppKit only:** GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them.

### TextInput: read-only and tweaks

- **`TextInput::read_only`** is the one semantic option every platform's text field has: AppKit `editable` off (still `selectable`), GTK's `editable`, Qt's `readOnly`, XAML's `IsReadOnly`. The text can be selected and copied, and the app can still set it. The a11y tree gets `read_only`.
- **Focus is the platform's.** GTK, Qt and WinUI keep read-only fields in the Tab order; AppKit's refuse keyboard focus (`acceptsFirstResponder` is false) unless Full Keyboard Access is on, and take it from a click. The core's focus order still lists them: AppKit skips views that can't become key.
- **Nothing can be typed into one.** `synthesize` returns `ActionError::ReadOnly` for any key, before focusing, and `perform(SetValue)` too: AppKit can't deliver keys to a field it won't focus, and WinUI's backend edits through the selection, which `IsReadOnly` doesn't stop.
- **Where typing goes after the app sets the text is the platform's.** A field focused when the window opened keeps its caret where the set left it: GTK's `set_text` leaves it at the start, AppKit at the end. The tests accept either.
- **Tweaks for the rest.** A length limit is GTK's, Qt's and WinUI's (AppKit needs a formatter); icons in the field are GTK's; a header is WinUI's. Secure entry is another control on AppKit and WinUI (`NSSecureTextField`, `PasswordBox`), so it is a widget of its own.
- **The example's tweaks:** a borderless field on AppKit (captured; macOS 26 draws a rounded bezel as it draws the default one, so that tweak showed nothing), a search icon on GTK, `maximumLength` on Qt, a header on WinUI (`ITextBox.Header` added to the bindings).
- **Run on AppKit and GTK:** Kirigami and WinUI are only type-checked, and CI hasn't run them.

### PasswordInput

- **A widget of its own**, not a `TextInput` option: AppKit and WinUI make password fields from other controls (`NSSecureTextField`, `PasswordBox`), and GTK's `PasswordEntry` isn't a `gtk::Entry`. Qt uses `Kirigami.PasswordField`, KDE's own, a `QQC2.TextField` that echoes bullets. It has a text field's props and events: `Value`, `Placeholder`, `Enabled`, `Changed(Text)`, `Submit`. No read-only: `PasswordBox` has none.
- **Hidden as each platform hides it.** Bullets everywhere; a button that shows the text on Qt and WinUI, their default; GTK's peek icon is off, GTK's default. Showing it is up to the platform and the app's tweaks.
- **A text field to assistive technology**, as every platform exposes one (AppKit's secure subrole, Qt's and UIA's password flag): role `TextField`, named by its label or placeholder, with `password` set and no value. `native_state` still reports the text, for the mirror check.
- **WinUI edits at the end.** `PasswordBox` has no caret or selection API, so synthesized keys append to or trim `Password`, where typing into a focused box goes, and it has no settable Value pattern, so `SetValue` sets `Password`.
- **The example's tweaks:** no bullets on AppKit (`echosBullets` off on the cell; captured), the peek icon on GTK, `showPassword` on Qt, asterisks on WinUI (`PasswordChar`; `PasswordBox` bindings added).
- **Run on AppKit only:** GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them.

### ScrollView: scroll bars and tweaks

- **`ScrollView::scroll_bars`** is the one semantic option every platform's scroll view has: hide the bars and keep scrolling, by wheel, trackpad and touch, as a strip of photos does. AppKit turns its scrollers off, GTK's policy is `External`, Qt's scroll bar policy `AlwaysOff`, XAML's visibility `Hidden`. Shown, they're the platform's own, overlay or not. "Always shown" isn't shared: on macOS it's the user's setting.
- **Shift+wheel scrolls sideways on WinUI too.** AppKit, GTK and Qt do it, and so do Windows' own apps (Explorer, Edge), but XAML's `ScrollViewer` doesn't (microsoft-ui-xaml#8553, closed as not planned). The backend fills it in: a `PointerWheelChanged` handler on the scroll view's content, which sees the wheel before the scroll viewer, scrolls sideways by XAML's 48 px a notch when Shift is down and the content is wider than the view. The content gets a clear background, as a host with a tooltip does: a `Canvas` without one is only hit where its children are, and the wheel passed it by between them. Scrolls apply at XAML's next layout, so quick notches add up from where the last one sent the view. Not covered by tests: wheel input can't be synthesized here; to be tried by hand in the scroll_view example.
- **AppKit keeps the axes on the node.** It read them back from its scrollers, which hidden bars turn off; synthesized scrolls use the node's axes too.
- **Tweaks for the rest.** Elasticity and borders are AppKit's, classic (non-overlay) scroll bars GTK's, the wheel's step Kirigami's, inertia and zoom WinUI's. A border narrows the visible area, which the core doesn't know about: content can lose a point or two at its edges.
- **The example's tweaks:** a bezel border on AppKit (captured), `overlay-scrolling` off on GTK, one line per wheel notch on Qt (`WheelHandler.verticalStepSize`; run on Kirigami), `IsScrollInertiaEnabled` off on WinUI (bindings added).
- **Run on AppKit only:** GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them.

### List: tweaks only

- **No semantic options past rows, selection and `ListStyle`** that every platform shares. Alternating row colours are AppKit's (GTK, Qt and WinUI have none built in); separators GTK's; single-click activation GTK's and WinUI's; wrapping key navigation Qt's; selection that doesn't follow focus WinUI's.
- **The selection mode can change while the list shows** (`List::selection_mode` takes a signal; the example has a switch). The selection keeps what the platform keeps, at most what the new mode holds, and the rows let go are reported as the user's, as for removed rows. XAML's `ListView` clears its selection on every mode change, even to one that holds more, and WinUI keeps that. AppKit's table keeps its selection when it stops allowing several rows, or any, so the backend keeps `selectedRow` (the row selected last) for Single and none for None. GTK has no mode, only selection models to swap: the backend carries over what the new model holds (the first row for Single), as AppKit does. The swap also takes the focused row out of the view while the window keeps it as its focus, so keys went nowhere; the backend gives focus back to the list, as AppKit's table keeps it. Qt's selection is the QML view's own, and keeps the first row. Run on WinUI, GTK and headless; AppKit and Kirigami are only type-checked.
- **Tweaks reach the list view,** not the scroll view the node stands for: the table on AppKit, the `gtk::ListView`, the QML `ListView`, XAML's `ListView` (the node's element already). The scroll view is its parent (AppKit `enclosingScrollView`).
- **`Tweak<List>`:** `List`'s type parameters have defaults (`List<(), (), RowRender<()>>`), so the widget can be named without its item and key types.
- **The example's tweaks:** alternating rows on AppKit (captured), separators on GTK (a line per row, which GTK's rows add to their padding: row placement is GTK's, as before), `keyNavigationWraps` on Qt, `SingleSelectionFollowsFocus` off on WinUI (bindings added).
- **Run on AppKit only:** GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them.

### Text: a line limit and tweaks

- **`Text::max_lines`** is the one semantic option every platform's label has: at most so many lines, the last cut off with the platform's ellipsis (AppKit `maximumNumberOfLines`, GTK `lines` with `ellipsize`, Qt `maximumLineCount` with `elide`, XAML `MaxLines` with `TextTrimming`). The label is measured as limited, and still read out in full. 0 lifts the limit, as on AppKit and WinUI. Qt's eliding labels also drop the lines past their height, so Kirigami measures them with the height lifted: a label's frame is 0 high until its first layout, and measured as one line from it.
- **Not selectable text, yet.** AppKit, GTK and WinUI labels can be made selectable (`selectable`, `selectable`, `IsTextSelectionEnabled`), but Qt's `QQC2.Label` can't; KDE's `Kirigami.SelectableLabel` is another control, a `TextEdit`. It would be a prop that makes the Kirigami backend create a different item.
- **Tweaks for the rest.** Colours, style classes, rich text and letter spacing are each platform's own.
- **The example's tweaks:** the secondary label colour on AppKit (captured), `dim-label` on GTK, Markdown (`textFormat`) on Qt, `CharacterSpacing` on WinUI (bindings added, with `MaxLines` and `TextTrimming`).
- **Run on AppKit only:** GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them.

### NumberInput

- **A spin box for a whole number, where every platform has one:** `NumberBox`, `gtk::SpinButton`, `QQC2.SpinBox`. AppKit has no single control, and its apps put an `NSStepper` beside a text field, so the backend does that (`mitsuami::appkit::NumberField`, a flipped view with both; the stepper holds the number, range and increment, and the field shows it). It came from 2ksbox's launcher (memory in MB, disk size in GB), and Separator and Disclosure, which it also uses, didn't qualify: WinUI has no standalone separator and Qt Quick no disclosure.
- **Whole numbers in an `i32`, because Qt's `SpinBox` holds an `int`:** a rule the core enforces since one platform makes the alternative impossible. Decimals typed or set are rounded: GTK with `digits` 0, Qt by its validator, WinUI in `ValueChanged` (`NumberBox` takes decimals; an emptied box, NaN, gets the last number back), AppKit when the field commits.
- **Typing reports when the edit is committed,** as every platform commits one: Return, or leaving the field (AppKit's cell `sendsActionOnEndEditing`). Buttons and arrow keys report at once. Text that isn't a number puts the number back.
- **What happens past an end is the platform's.** AppKit's stepper wraps round (`valueWraps` is on by default); GTK, Qt and WinUI stop. Numbers typed past an end are clamped everywhere. `stops_or_wraps_at_its_ends_as_the_platform_does` expects each.
- **Held buttons repeat, as every platform's do** (`NSStepper.autorepeat`, GTK's and Qt's buttons, WinUI's `RepeatButton`s), reporting each step. On AppKit a stepper made in code steps once and stops (seen on macOS 26, and in a bare AppKit app too), unless its cell also sends its action on periodic events, so `NumberField` sets `sendActionOn` to mouse down, dragged and periodic. Not tested: the test kit can't hold a native button down.
- **Inline spin buttons on WinUI.** `NumberBox` hides them by default; `Inline` is its documented spin-box mode, and a tweak can pick `Compact` or `Hidden`.
- **Sized as the platform sizes it:** GTK for its range's widest number, Qt for its text, WinUI by `Measure`, AppKit for the range's longest number in the field plus the stepper; so `Range` and `Number` re-measure a `NumberInput` (`Prop::affects_measure` now takes the kind). Headless measures one 96 wide.
- **A spin button to assistive technology** (`Role::SpinButton`), named by its label, with the number as its value. On AppKit the field and the stepper both get the label, as VoiceOver finds them separately.
- **Not tested: typing keys into one.** `synthesize` has no `NumberInput` path yet on any backend (each would drive the field inside); the suite uses assistive technology's `SetValue`, `Increment` and `Decrement`.
- **The example's tweaks:** `valueWraps` off on AppKit, `wrap` on GTK and Qt, `Compact` spin buttons on WinUI (`NumberBox` bindings added).
- **Run on AppKit, WinUI and headless;** GTK and Kirigami are only type-checked, and CI hasn't run them.

### Image

- **A picture from a file or from pixels in memory,** in each platform's image view: `NSImageView`, `gtk::Picture`, XAML's `Image`, a QtQuick `Image` (`Kirigami.Icon` is for themed icons, which is another widget if it ever comes). 2ksbox's shader preview is pixels the app renders, so pixels aren't a detour through a file.
- **Pixels are straight RGBA8, sRGB, with a scale** (pixels to a point), so an app can render at the window's scale factor and have each pixel shown as one. Each backend makes its own image from them: an `NSBitmapImageRep` retagged sRGB, a `gdk::MemoryTexture`, a `WriteableBitmap` (premultiplied BGRA, converted), and on Qt a `QImage` served by a `QQuickImageProvider` registered as `mitsuami`, Qt's way to give QML images from memory. Every set of pixels gets its own `image://` URL there, and `cache` is off, so QML never shows a stale one. `Pixels` holds an `Arc`, so props clone cheaply, and prints its size, not its bytes.
- **A file is read when it's set.** AppKit, GTK and Qt (`asynchronous: false`) decode it right away; WinUI decodes in the background, so its `ImageOpened` and `ImageFailed` send the new `UiEvent::Remeasure` and the core measures it again. A file that changes under the same path isn't read again: set other pixels, or another path. A missing or unreadable file shows nothing and measures zero. Headless reads a PNG's size from its header, and gives other files no size.
- **Its natural size is the image's in points:** pixels over their scale, or the file's size as the platform reads it (AppKit honours a PNG's resolution; the others count pixels).
- **Only the fits every platform has:** `Contain` and `Stretch`. Aspect fill is missing from `NSImageView`, and "only shrink" from Qt and XAML, so those are tweaks. The fit is sent only if the app picks one, since the defaults differ: AppKit shrinks proportionally but never enlarges, GTK and XAML contain, Qt stretches.
- **Smoothing isn't shared** (Qt's `smooth` is the only switch), so pixel-sharp scaling is a tweak on Qt; elsewhere, pixels made at the window's scale aren't scaled at all.
- **An image to assistive technology** (`Role::Image`), named by its label (GTK's `alternative-text`, Qt's `Accessible.Graphic`); without one, decorative. It takes no focus. No platform gives an image's source back, so backends keep it on the node for the mirror check.
- **`draws_what_it_is_given` checks the pixels on screen:** a capture of the window, blue and red where the fixture has them, so a mirrored or swapped-channel image fails.
- **The example's tweaks:** a photo frame on AppKit (`imageFrameStyle`), `content-fit` cover on GTK, `smooth` off on Qt, `UniformToFill` on WinUI (`Image`, `Stretch`, `BitmapImage`, `WriteableBitmap`, `Uri` and `IBufferByteAccess` added to the bindings).
- **Run on AppKit, WinUI and headless;** GTK and Kirigami are only type-checked, and the shim's image provider only compiled against Qt 6 headers on macOS (not linked). An unpackaged WinUI app loads a `BitmapImage` from an absolute `file:///` URI; paths with spaces or `#` are unverified. WinUI's `settle` waits for files being decoded (`ImageOpened` or `ImageFailed`), as it waits for spinners to load: `expect` only retries while the app has tasks, and XAML's decoding isn't one.

### Tooltips

- **A prop, not a widget:** `.tooltip(text)` on any widget or container (`ElementBuilder`), sent as `Prop::Tooltip`; empty removes it. 2ksbox puts one on a status line cut off at one line, holding the whole text.
- **Shown as each platform shows them,** with its delay, placement and look: `NSView.toolTip`; `set_tooltip_text`; `ToolTipService` on WinUI; on Qt the attached `QQC2.ToolTip` (Breeze draws it), visible while hovered after `Qt.styleHints.mousePressAndHoldInterval`, as Kirigami apps do it. Qt Quick has no tooltip without that binding: controls use their own `hovered`, labels, images and container hosts a `HoverHandler`.
- **On the view under the pointer:** a list's table or list view rather than the scroll view around it, and on AppKit a `NumberInput`'s field and stepper as well as their host.
- **Containers:** a WinUI host is a Canvas with no background, which never gets the pointer, so it gets a clear background while it has a tooltip (and takes the pointer over its empty areas meanwhile). A box's tooltip also shows over children without one of their own: on AppKit (tried by hand), on Qt (hover is passive), and GTK looks for a tooltip up the widget tree.
- **Read as the description** unless the app gave one: the core's tree does that, AppKit reads a tooltip as the view's help and GTK as its description on their own, Qt gets `Accessible.description`, WinUI `AutomationProperties.HelpText`.
- **Custom renders, drawn and native items on Kirigami** keep the tooltip on the node but don't show it: their QML is the app's.
- **Not tested: showing on hover,** since nothing here can rest a pointer on a native widget. The suite checks that each backend's native widgets and containers carry it, and the description; `examples/tooltip.rs` is for trying it by hand.
- **Run on AppKit (hover tried by hand) and headless;** GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them.

### Windows opened while the app runs

- **`Window` is a view, declared anywhere in the tree,** next to the state that opens it: shown while its `open` value is true, like `Show`, which is how 2ksbox's launcher drives its windows (a model's `open` flag). Its content is built when it opens and disposed when it closes, so each opening starts fresh; it closes with the scope that declared it, so a window declared in another window's content closes with that one. In the tree it's a fragment that takes no room; the native window is top-level. `App::window` stays for the windows open at startup; the app ends when the last window closes, whichever kind.
- **The close button asks, and the app decides.** `bind(flag)` closes it by clearing the flag; `on_close_request` lets the app keep it open (to ask about unsaved changes, as `examples/windows.rs` does); without either the close button does nothing, since the flag is the app's. Platforms deliver the request their own way (`windowShouldClose:`, `close-request`, `closing`, `AppWindow.Closing`), always vetoed, and the core destroys the window when the app closes it.
- **Modal windows are the app's choice of two** (`.modal(Modality::…)`, or `.modality(value)` read at each opening): `Window` blocks the window it's declared in, `Application` the whole app. The maintainer chose to offer both rather than pick for macOS, where they look different: `Window` is a sheet on its window (the macOS idiom for a dialog about a window), `Application` a separate window centred on its owner, run in `NSApp.runModalForWindow` (what Qt does for 2ksbox's dialogs today). Elsewhere: Qt's `WindowModal` and `ApplicationModal` with a `transientParent`; GTK's `modal` with `transient_for`, which blocks the whole app either way (GTK has no window-modal; GNOME attaches modal dialogs to their parent); WinUI's owned window with `OverlappedPresenter.IsModal`, which disables the owner, plus the app's other windows disabled for `Application`, as Win32 apps do.
- **Only modal windows belong to a window:** the one they're declared in, found through `CurrentWindow`, which each window's scope provides (`App::window`, `Window`, the test kit's mount). A window declared inside a modal window belongs to that one. One declared outside every window (rare) has no owner, so a `Window` modality becomes `Application`: Qt's window-modal blocks nothing without one. Plain windows belong to none, so they aren't kept above another window, as owned windows are on Qt, GTK and WinUI. Every `Window` builds in a scope of its own, even when always open, so its `CurrentWindow` doesn't leak to what's declared after it.
- **Qt ignores a modality set on a window already shown,** and `Kirigami.ApplicationWindow` shows itself when created (`visible: true`), so modal windows didn't block anything. The backend's windows start hidden, as they were meant to, and show after their first layout. The tests read the modality back, which Qt keeps even when it ignores it, so this was found by hand and checked against `QGuiApplication::modalWindow()` in a small program.
- **Alerts and file dialogs without a window go on the focused one, and Escape closes only the focused modal window.** Qt's `active` is true for the focused window's transient parents, and so for their other transient children: a modal window's owner and its other dialogs all read as active. Taking the first active window put alerts on the owner, which the modal window blocks, and Qt matches window shortcuts by `active`, so two modal windows' Escape shortcuts were ambiguous and neither fired. The shim sets each window's `mitsuamiFocused` from `QGuiApplication::focusWindow()`, which alerts and the Escape shortcut use. Synthesized keys go through the shortcuts first (`qt_sendShortcutOverrideEvent`, as QTest does), after activating their window.
- **The same on WinUI: the active window** (`GetActiveWindow`). The backend took the first window with a focused control, but every XAML window keeps one while inactive, so the alert could land on a modal window's owner, which the modal window disables. Without an active window (the app in the background), a window that isn't disabled, focused if one is. The test kit answers alerts itself, so this was checked by hand in the windows example.
- **Known gap: GTK's window-modal windows aren't tied to their window.** They're separate modal windows, `transient_for` their owner, which block the whole app. Only GNOME's compositor attaches them, so they move with their owner there; on sway they move on their own, as any GTK app's do. GNOME apps (Text Editor) now use libadwaita's dialogs, drawn in the parent window, which move with it everywhere and block only that window, like a macOS sheet. `Modality::Window` as an `adw::Dialog` and alerts as `adw::AlertDialog` would close the gap, at the cost of a libadwaita dependency, a dialog clipped to its owner (a bottom sheet in a narrow window), and title, size and close requests wired through the dialog. Left for later.
- **On WinUI it's applied when the window is shown** (windows are activated hidden at creation, for a live XAML tree): `GWLP_HWNDPARENT` to the owner, centred on it, not minimizable or maximizable, then `IsModal`; for `Application` the app's other windows are disabled with `EnableWindow`, leaving alone those another modal window already disabled, and re-enabled on destroy, before the owner comes back to the front. Unverified until it runs: `IsModal` set after the window was activated, and the owner re-enabled when a modal window closes.
- **A sheet has no close button,** so its content must offer a way out: a `ButtonRole::Cancel` button, which Escape presses on AppKit. The modal loop can't start inside a tick (it's nested), so AppKit schedules it on the main run loop after the tick that shows the window; the UI keeps ticking inside it, since the app's observer runs in the modal panel mode too. Destroying the window ends its sheet (`endSheet:`) or its loop (`abortModal`, the call for code outside event handling, with an empty event to wake the loop). Tests never show windows, so they check the modal prop the backends carry, not the sheet or the loop: `examples/windows.rs` lets you pick each and try them.
- **Escape asks a modal window to close** (tried by hand on AppKit), through the close request, as every platform's dialogs close on Escape (AppKit's Cancel button, `QDialog`, `GtkDialog`, Win32's `IDCANCEL`); plain windows ignore it, as they do everywhere. Each goes through the platform's own path, so a focused control that uses Escape (an open pop-up, completion) gets it first: on AppKit a key equivalent (a `ButtonRole::Cancel` button takes it), then `cancelOperation:` up the responder chain to the window's delegate, which a field editor passes on; on GTK a bubble-phase `ShortcutController` calling `close()`, as `GtkDialog` has; on Qt a `Shortcut` on `StandardKey.Cancel`, as 2ksbox's launcher has; on WinUI a `KeyboardAccelerator`, which fires only when the focused control didn't handle the key. Synthesized Escape takes the real path on AppKit and Qt (a key event); GTK 4 can't inject key events and WinUI's backend drives controls, so theirs run the window's shortcut or accelerator action directly, and a focused control can't take Escape first in their tests. Unverified until they run: that a focused `QQC2.TextField` doesn't claim Escape (`ShortcutOverride`) before the window's `Shortcut`, and that an accelerator on WinUI's root `Grid` fires wherever focus is.
- **`TestHooks::close_window`** clicks the close button through the platform, so the request goes the way a user's does: `performClose:`, `gtk::Window::close`, `QQuickWindow::close()` (which sends nothing to a window never shown; test windows are shown by then), and on WinUI a posted `WM_CLOSE`, where the close button and Alt+F4 end up, since `Window.Close()` closes without raising `Closing` (that a posted `WM_CLOSE` raises it is unverified). `TestApp::close_window` and `TestApp::window_titled` use it; locators already search every window.
- **In `view!`, the title is an attribute and the children are the content:** `<Window title="Machine" bind=editing>…</Window>`. `Window::new` takes the title; the tag starts without one, and the title is a type parameter, `()` until set, as `#[component]` does for required props.
- **`on_open` runs at each opening, before the content is built,** to start a form from what's saved. The builder API can do it in `content`'s closure, but `view!`'s children are views, so it needs a handler: `<Window title="Advanced" bind=open @open=move || draft.set(…)>`. A window's content is one view, as `content` takes; several children don't make a `View`, so they go in a `Column`.
- **Nested dialogs are windows declared in a window's content,** which is how the tests and `examples/windows.rs` write a cancellable dialog: "Advanced…" in a machine window opens a `Modality::Window` dialog on it, with a draft started in `on_open` that OK applies to the machine window's form and Cancel, Escape or the close button drop. It belongs to the machine window and closes with it.
- **Run on AppKit and headless** (sheets and the modal loop tried by hand in `examples/windows.rs`, since tests never show windows); GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them.

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
- **Run on AppKit, GTK, WinUI and headless;** Kirigami is only type-checked, and CI hasn't run it. Unverified until they run: that Kirigami 6 resolves `DisplayHint.KeepVisible`, and that `ActionToolBar` honours `visible` and makes each `displayComponent` once; that WinUI's `IsInOverflow` tells an item moved to its overflow menu. On WinUI, `its_items_are_above_the_content` fails: in a test's first window the XAML root is still laid out wider than the window (1265 for 800), so the bar puts its items past the window's right edge. The example's window is laid out right.

### Menus

- **Menus stay a service, not widgets.** Every platform builds its menus from data and calls back with the item chosen, and on macOS the bar isn't in any window. So a `MenuBar` is data (`MenuBarData`) the core sends again whenever it changes, not nodes in the tree: menus have no frames, no Tab order and nothing for the mirror check to read back.
- **What's built:** submenus; check items (`MenuItem::bind` toggles a signal, `checked` only shows it) and radio items (`radio((signal, value))`, a pair so `view!` can write `radio=(zoom, Zoom::Large)`); reactive titles, visibility (`visible`, on items and menus) and lists of items (`Menu::children_with`); roles (`MenuRole::{About, Settings, Quit}`); and a window's own menus, a `MenuBar` in its content, written in `view!` (`<MenuBar>`, `<Menu title=…>`, `<MenuItem>`, `<MenuSeparator/>`). `MenuItem::new` takes only the title now, with `on_select` for the handler, as `Button` has `on_click`.
- **Radio items next to each other form a group,** as a separator ends one on every platform. The core keeps a group exclusive: choosing an item sets the signal, and every item's check comes from it. `MenuData::radio_groups` names each group by its first item, for the platforms that group natively.
- **Ids are unique across bars and stay put while the structure does:** each bar gives the item at each position the same id on every rebuild. So when only enabled and checked states change (`MenuBarData::same_structure`), GTK, WinUI and Kirigami update the items in place, and an open menu stays open. Any other change rebuilds that window's menus. AppKit rebuilds its bar every time, as it did before.
- **A window's menus are shown with the app's** (`MenuBarData::merged`): a window menu titled like an app menu joins it after a separator, and the others follow. On GTK, WinUI and Kirigami that's what the window shows. On macOS, where the bar is the app's, a window's menus are there while it's the main window (`NSWindowDidBecomeMain`/`ResignMain`), the window menu commands act on. A window's menus reach the services while its content is built, before the backend has applied its `CreateWindow`, so backends keep them by `NodeId` and use them when the window is made; Kirigami builds its drawer inline then, as it needs to. A `MenuBar` outside any window is the app's, as `set_menu` is; installing the app's menus again replaces them.
- **Roles move items only where the platform has a place for them** (`MenuBarData::take_role` takes them out, with separators and menus left empty). AppKit puts About, Settings and Quit in the app menu with its own titles and shortcuts ("About <app>", "Settings…" ⌘,, "Quit <app>" ⌘Q), as Qt's menu roles do. GTK puts them in the primary menu's last section in GNOME's order (Settings, About, Quit), Settings getting Ctrl+, if the app gave it none. Kirigami ends the drawer with them (Settings with KDE's Ctrl+Shift+,, About, Quit, with their theme icons), as KDE apps do. On Windows there's no standard place, so they stay where the app put them. An app's Quit replaces the platform's (AppKit's `terminate:`, GTK's and Kirigami's "ask every window to close"), titled and bound as the platform's is; on Windows it's an ordinary item.
- **Check marks are the platform's:** AppKit's item `state` (a check mark for radio items too, as AppKit's menus show a choice); a boolean stateful action on GTK, and for a radio item an action of its own holding its id while chosen, with the id as the item's target, which GTK draws as a radio; `ToggleMenuFlyoutItem` and `RadioMenuFlyoutItem` (`GroupName` from the group's first id) on WinUI; `checkable` actions on Kirigami, each radio group in one exclusive `QQC2.ActionGroup`. XAML and Qt toggle an item themselves when it's clicked, so their backends put back the app's state before reporting the choice; only the user's click reports one.
- **Run on AppKit and headless** (`tests/menus.rs`, and the AppKit services test, which posts the main-window notifications itself since test windows are never main); `examples/menus.rs` is for trying them by hand. GTK, Kirigami and WinUI are only type-checked, and CI hasn't run them. Unverified until they run: that GTK draws per-item radio actions as radio items and that a shortcut's target reaches one; that Kirigami's menu draws radio indicators and keeps an `ActionGroup` exclusive, that `QKeySequence` reads "Ctrl+Shift+,", and whether reinstalling a finished window's drawer still warns about a binding loop; that XAML flips `IsChecked` before it raises `Click`.

### M2 (GTK 4)

What the GTK 4 backend taught us:

- **Tests run on a private Broadway display.** The runner starts `gtk4-broadwayd` (bound to localhost) and points GDK at it, unless `MITSUAMI_SHOW_WINDOWS=1`. GTK needs a real, mapped window to capture and to track focus, and on the session display a tiling compositor would override window sizes. The display prints its address, so tests can be watched in a browser.
- **Windows get an explicit `HeaderBar`.** GTK's default size includes the titlebar. With a header bar of our own, its height is known, and the content gets exactly the size the core asks for.
- **Layout hosts allocate each child at its core frame**, and ask for exactly their own frame (window content hosts ask for nothing, so windows can shrink). Frames live in one map shared by all hosts, so a child keeps its frame when it moves to another parent (the core only resends frames that change). Leaves with an empty frame are hidden from GTK's allocation: controls can't be allocated smaller than their padding.
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
- Not done yet: the `adwaita` feature, a reduced GTK 4.8 mode, and `gtk::Application` integration (single instance, app ID). `run` drives a plain GLib main loop.

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
- **Tests run on Qt's offscreen platform** (no display server, exact window sizes, software rendering, so captures don't depend on the GPU). The desktop's settings stay out: no platform theme, a private `kdeglobals`, Plasma's default font. Light and dark are Breeze Light and Breeze Dark, switched the way KDE apps' color scheme menus do (`KDE_COLOR_SCHEME_PATH` and a palette change). The private `kdeglobals` turns animations off (`AnimationDurationFactor=0`): captures caught a switch's knob mid-slide.
- **Captions.** Without Plasma's platform theme, Kirigami's small font falls back to a 12 pt system font, larger than the body; captions are then 0.8 × the body, Plasma's ratio.
- **Teardown.** A backend can be dropped with the thread-locals that hold it as the process exits, after Qt's thread data or KDE's icon loader has gone: destroying a window then crashed. Dropped backends post their windows' deletion instead, a new backend flushes what earlier ones posted (their controls still held Kirigami's Alt-key mnemonics, which moved the underlines in the next test's capture), and at exit a C++ thread-local sentinel drops what is still queued.
- **Qt logs to the systemd journal when stderr isn't a terminal,** so QML warnings vanish from piped or captured output (a warning-free run that wasn't): `QT_FORCE_STDERR_LOGGING=1` puts them back on stderr.
- **Kirigami wants popups whole from the start.** A global drawer created without a parent reads the parent it doesn't have yet, and one attached to a finished window (or given its actions late) makes the hamburger button report a binding loop. Windows get their menu drawer in their own QML (menus are set before the commit that creates windows); a menu whose structure changes later still gets a drawer created in the window's overlay, and Kirigami's warning. A `PromptDialog` opened before its window's first frame loops over its position, so alerts wait for that frame.
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
