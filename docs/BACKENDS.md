# Writing a mitsuami backend

This guide is for writing a platform backend: GTK 4 (`mitsuami-gtk`), WinUI 3 (`mitsuami-winui`), Qt Quick and Kirigami (`mitsuami-kirigami`), or any other.

**Working references:** the AppKit backend (`crates/mitsuami-appkit`) and the GTK 4 backend (`crates/mitsuami-gtk`). The Kirigami backend (`crates/mitsuami-kirigami`) shows a toolkit without Rust bindings: a small C++ layer, and QML created per node. Every rule here is enforced by the test suites, and many were learned the hard way.

**Background:** read [ARCHITECTURE.md](ARCHITECTURE.md) §2–§5 first.

## 1. The model in one paragraph

The core owns the widget tree, the layout and every decision. A backend is a **mirror**:

- It creates native widgets when told to, and places them where it's told.
- It reports what the user did.
- It answers questions: how big is this, what does it show, what does it look like.

The backend **never lays anything out** and **never calls back into the `Ui`** from native callbacks; it queues events instead. All coordinates are **logical units** (points / effective pixels / GTK logical px), **parent-relative**, with the **origin at the top-left**.

## 2. What you implement

| Piece | Where | Reference |
|---|---|---|
| `Backend` | `mitsuami_core::Backend` | `mitsuami-appkit/src/backend.rs` |
| `Services` (clipboard, dialogs, menus) | `mitsuami_core::services::Services` | `mitsuami-appkit/src/services.rs` |
| `TestHooks` on your shareable handle | `mitsuami_core::TestHooks` | `impl TestHooks for AppKitHandle` (`settle` is only needed where the platform works asynchronously: see `GtkHandle`) |
| `run(setup)`: the app's run loop | your crate | `mitsuami-appkit/src/app.rs` |
| `init_for_tests()` | your crate | same file |

You also touch three places outside your crate, each behind a `cfg(target_os = …)`:

1. **`crates/mitsuami/Cargo.toml`, `crates/mitsuami/src/app.rs`:** `App::run` calls your `run`.
2. **`crates/mitsuami-test/Cargo.toml`, `crates/mitsuami-test/src/driver.rs`:**
   - Add a `native()` that builds your backend for tests: offscreen unless `MITSUAMI_SHOW_WINDOWS=1`, recording commands, a fixed light appearance, and a private clipboard if the platform allows it.
   - Add your OS to `native_available()`.
3. **`crates/mitsuami-<name>/Cargo.toml`:** put native dependencies under `[target.'cfg(target_os = "…")'.dependencies]`, so the workspace still builds everywhere.

A second toolkit on an OS is a cargo feature rather than a `cfg(target_os)`: Kirigami next to GTK on Linux is `mitsuami`'s `kde` feature (which wins over the default `gtk`), `mitsuami-test`'s `kde` feature, and the `kde` and `gtk` arms of `platform!`. A backend whose native libraries may be missing where the workspace builds keeps them behind a feature of its own (`mitsuami-kirigami`'s `qt`), so it builds empty without it.

Keep a shareable handle (`Rc<RefCell<State>>` inside the backend, with a cloneable handle outside). `Ui::new` takes ownership of the backend, but tests, services and the run loop still need access to it.

## 3. Commands

`Backend::apply(&mut self, batch: &[Command])` receives batches. **Order within a batch matters**, and the core guarantees:

- A node is created before it is inserted, and a subtree's root is removed before it is destroyed.
- Frames arrive in a second `apply` call per commit, after structure and props, because layout measures widgets that must already exist. Drawn widgets' `SetProp(Drawing)` comes in that second call too: drawing needs the size.
- `Create` carries the widget's initial props, reactive ones included.

Validate as you go. Panic on protocol violations such as an unknown node, a double insert, or a second child in a ScrollView. The headless backend does this too; it's how contract bugs surface.

| Command | What to do |
|---|---|
| `Create { id, kind, props }` | Create the native widget and apply `props`. **Give it a zero frame**: the core only sends frames that differ from the last one it sent, so a widget born with its own frame stays wrong (AppKit labels do this). |
| `SetProp { id, prop }` | Apply the prop (tables below). For `Value`, skip the update if the widget already shows it, so the caret and IME composition survive. |
| `Insert { parent, child, index }` | Attach at `index` among the parent's native children. **ScrollView**: its single child is the scrolled content (AppKit: `documentView`). **List**: the child is a row host, put in its row's cell (§8b). **Window**: a `ToolbarItem` goes in the window's toolbar, at the trailing end; items always come after the content, so its index among the items is `index` less the content's children. |
| `Remove { parent, child }` | Detach only. |
| `Destroy { id }` | Free the widget. It comes for every native node of a removed subtree, children first; the root has already been removed. Drop observers, signal handlers and targets. |
| `SetFrame { id, frame }` | Place the widget, relative to its native parent's top-left. Never sent for windows. A ScrollView's content frame is in content coordinates. A `ToolbarItem`'s is only its size: the toolbar places it; hide it while its size is empty. |
| `SetA11y { id, a11y }` | Set the accessible label, description and hidden state. |
| `SetWindowSize { id, size }` | Set the window's **content area** size (excluding title bar and menu bar). |
| `SetFocusOrder { window, order }` | Make Tab visit `order` in sequence, wrapping around. It is window-wide, across nested containers. The platform still decides *which* controls can take focus (disabled controls, macOS keyboard navigation settings). See §9. |
| `ScrollTo { id, offset }` | Scroll the ScrollView or List so `offset` is at its top-left. Already clamped (a List clamps it itself). The platform then **reports `Scrolled`**, as for a user scroll. |
| `ScrollToRow { id, row }` | Scroll the List just enough to show that row, with the platform's own "scroll to row". Report `Scrolled`, and the rows shown. |
| `Focus { id }` | Give the control keyboard focus. The focus change is reported through your focus tracking (§4), not by this command. |

### Widget kinds

| Kind | AppKit (done) | GTK 4 (done) | WinUI 3 (done) | Kirigami (done) |
|---|---|---|---|---|
| `Window` | `NSWindow` + flipped content host | `gtk::Window` + `HeaderBar` + host as child (done) | `Window` + `Canvas` as `Content` | `Kirigami.ApplicationWindow` with one `Kirigami.Page`, a plain `Item` as its content |
| `Container` (layout host) | flipped `NSView` subclass | `gtk::Widget` subclass that allocates children at given frames (or `gtk::Fixed`) | `Canvas` (`Canvas.Left/Top`, `Width/Height`) | `Item` (children at `x`/`y`/`width`/`height`) |
| `ToolbarItem` (a window's toolbar) | its host as an `NSToolbarItem`'s view, sized by constraints, after a flexible space; out of the toolbar while empty | a host packed at the end of the window's `HeaderBar`, hidden while empty | a `Canvas` in an `AppBarElementContainer` among a `CommandBar`'s `PrimaryCommands`, collapsed while empty | a host `Item` in the `displayComponent` of a `Kirigami.Action` in the page's `actions` (`KeepVisible`), hidden while empty |
| `Text` | `NSTextField` wrapping label | `gtk::Label` (wrap on, `xalign 0`) | `TextBlock` (`TextWrapping.Wrap`) | `QQC2.Label` (`WordWrap`) |
| `Button` | `NSButton` push | `gtk::Button` | `Button` | `QQC2.Button` |
| `TextInput` | `NSTextField` | `gtk::Entry` / `gtk::Text` | `TextBox` | `QQC2.TextField` |
| `PasswordInput` | `NSSecureTextField` | `gtk::PasswordEntry` (no peek icon, GTK's default) | `PasswordBox` (its reveal button, XAML's default) | `Kirigami.PasswordField` (with its show button) |
| `Checkbox` | `NSButton` checkbox | `gtk::CheckButton` | `CheckBox` | `QQC2.CheckBox` |
| `Switch` | `NSSwitch` | `gtk::Switch` | `ToggleSwitch` | `QQC2.Switch` |
| `Slider` | `NSSlider` (a step is tick marks) | `gtk::Scale` | `Slider` | `QQC2.Slider` |
| `NumberInput` | `NSTextField` with an `NSStepper` beside it (`mitsuami::appkit::NumberField`) | `gtk::SpinButton` | `NumberBox`, inline spin buttons | `QQC2.SpinBox`, editable |
| `Progress` | `NSProgressIndicator` bar | `gtk::ProgressBar` (pulsed by a timer while indeterminate) | `ProgressBar` | `QQC2.ProgressBar` |
| `Spinner` | spinning `NSProgressIndicator` (`displayedWhenStopped` off) | `gtk::Spinner` | `ProgressRing` | `QQC2.BusyIndicator` |
| `Image` | `NSImageView` | `gtk::Picture` | `Image` (a `BitmapImage`, or a `WriteableBitmap` for pixels) | QML `Image` (an image provider for pixels) |
| `GpuSurface` (§8c) | `NSView` whose backing layer is a `CAMetalLayer` | `gtk::DrawingArea` keeping the space, under a Wayland subsurface of our own (`mitsuami-wayland`) | `Canvas` keeping the space, under a child HWND | `Item` keeping the space, under a Wayland subsurface of our own |
| `Select` | `NSPopUpButton` (items added to its menu) | `gtk::DropDown` over a `gtk::StringList` | `ComboBox` of `ComboBoxItem`s | `QQC2.ComboBox` |
| `ScrollView` | `NSScrollView` | `gtk::ScrolledWindow` | `ScrollViewer` | `QQC2.ScrollView` around a `Flickable` |
| `List` (§8b) | view-based `NSTableView` in an `NSScrollView` | `gtk::ListView` over a `gio::ListStore` of keys | `ListView` over the keys (boxed strings), with `Canvas` cells | QML `ListView` over the keys, with `QQC2.ItemDelegate`s |
| `Custom` (native render) | the render's view (`NativeRender`) | the render's widget (`mitsuami_gtk::NativeRender`) | the render's element | the render's item |
| `Custom` (drawn) | `DrawnView`: flipped `NSView` that rasterizes the display list | a `gtk::DrawingArea` rasterized with Cairo | a `Canvas` with Win2D, or `Microsoft.UI.Composition` shapes | a `QQuickPaintedItem` painted with `QPainter` |
| `Native` | the app's `NSView` (`NativeView::appkit`) | the app's `gtk::Widget` (`NativeView::gtk`) | the app's `FrameworkElement` | the app's QML item (`NativeView::qml`) |

### Props

| Prop | Applies to | Notes |
|---|---|---|
| `Title` | Window | |
| `Text` | Text | |
| `MaxLines` | Text | At most this many lines, the last cut off with the platform's ellipsis; `None`: all. AppKit `maximumNumberOfLines` (the cell truncating its last visible line), GTK `lines` with `ellipsize` end (GTK only limits ellipsizing labels), Qt `maximumLineCount` with `elide` right, XAML `MaxLines` with `TextTrimming` `CharacterEllipsis`. Measure the label as limited. Report it back. |
| `Label` | Button, Checkbox, Switch, Select, Slider, NumberInput, Progress, Image, GpuSurface | Only buttons and checkboxes show it; for the others it's the accessible name. |
| `Value` | TextInput, PasswordInput | Don't re-set a value the widget already shows. Report a password field's text back too: the mirror check compares it, though the a11y tree never shows it. |
| `Placeholder` | TextInput, PasswordInput | |
| `ReadOnly` | TextInput | Shows the text, still selectable, but not editable: AppKit `editable` off (`selectable` stays on), GTK and XAML `editable` / `IsReadOnly`, Qt `readOnly`. Keyboard focus is the platform's: AppKit's read-only fields take it from a click, and from Tab only with Full Keyboard Access. The app can still set `Value`. Report it as the field shows it. |
| `Checked` | Checkbox, Switch | Setting it programmatically **must not** emit `Changed` (§4). While a checkbox is mixed, keep it for when it isn't, and report it back. |
| `Mixed` | Checkbox | Sent only if the app gave one. Shows the mixed state over `Checked` (AppKit `NSControlStateValueMixed`, GTK `inconsistent`, Qt `checkState` partly checked, WinUI `IsChecked` null). A user click leaves it, landing where the platform lands, and emits `Changed(Bool)`; the core absorbs that as `Mixed(false)`. Don't let later clicks cycle back into it: turn off AppKit's `allowsMixedState` and Qt's `tristate` after a click; clear GTK's `inconsistent` on a user toggle, as GTK apps do. Setting it **must not** emit `Changed`. Report it as the native control shows it. |
| `Options` | Select | The options' texts, in order; texts may repeat. Replacing them keeps the chosen index if it's still an option, else chooses the first (none without options), as the core does: it sends `SelectedIndex` only when that changes it. Measure the select as the platform sizes it (widest or chosen option). |
| `Range { min, max }` | Slider, NumberInput | The core sends `Number` after it, since the platform may clamp the value to it. |
| `Orientation` | Slider | Horizontal or Vertical; sent only if the app chose one. Larger values are up: GTK runs vertical scales top down, so set `inverted` there, as GTK apps do. Measure as the platform does (AppKit's vertical sliders have no natural height, like its horizontal ones' width). Report it as the control shows it. |
| `Step` | Slider, NumberInput | Use it as the platform uses a step (tick marks, snapping, keyboard increments; what a spin box's buttons add). A platform with no stepped slider snaps the user's moves to it, as its apps do (GTK: in `change-value`, with a mark at each step that a tweak can turn off). `None`: the platform's default. Keep it on the node if the platform can't read it back. |
| `Number` | Slider, NumberInput | The value; a `NumberInput`'s is a whole number in an `i32`. Setting it **must not** emit `Changed`. |
| `Progress` | Progress | From 0 to 1, or `None` for work of unknown length: the platform's indeterminate, animated bar. |
| `Modal { owner, modality }` | Window | Sent once, right after `Create`, before the window is shown. Show it modal the platform's way: `Window` blocks its owner (AppKit: a sheet on it), `Application` the whole app (AppKit: `runModalForWindow`, started outside the tick). Keep it above its owner (transient, owned). Destroying it ends the sheet or modal state and gives the owner back. Keep it on the node and report it. |
| `Tooltip` | any node | The platform's tooltip on the view the pointer rests on (`toolTip`, `set_tooltip_text`, `ToolTipService`, the attached `QQC2.ToolTip` on hover); empty removes it. Make it reach assistive technology as the description unless the node has its own (AppKit and GTK do that themselves). Report it for every node, `""` when none. |
| `ContextMenu` | any node | The platform's context menu, on the view the pointer rests on (as `Tooltip`), shown its own way: a right-click, a long press, the menu key. Build its items as your menu bar's (ids, check marks, radio groups, submenus, separators, shortcuts shown); roles mean nothing there. Empty removes it. A view without one lets its container's show, as the platform does. A control whose native menu is taken (AppKit's pop-up button: its menu is its options) keeps it on the node. Report it once the core has sent one, read back where you can. |
| `Image` | Image | A file (the platform decodes it; unreadable: show nothing, measure zero) or `Pixels`: straight RGBA8, sRGB, at `scale` pixels to a point. Measure it at its size in points (`Pixels::size`), not its pixel count. Keep it on the node: no platform gives the pixels back. |
| `ImageFit` | Image | `Contain` or `Stretch`, sent only if the app chose; otherwise the platform's default fit. |
| `Running` | Spinner | Spins while true (`startAnimation`, `spinning`, `IsActive`, `running`). Stopped, it shows nothing but keeps its size. Report it back (AppKit can't read it: keep it on the node). |
| `SelectedIndex` | Select | The chosen option. `None` only without options. Setting it **must not** emit `Changed`. |
| `Enabled` | controls | |
| `TextStyle` | Text (and controls) | Map to the platform type ramp: GTK style classes (`title-1`, `heading`, `caption`, `monospace`); WinUI text styles (`TitleTextBlockStyle`, …); Kirigami's `Heading` sizes and its small and fixed-width fonts. |
| `ButtonRole` | Button | Normal, Default, Cancel or Destructive; sent only if the app chose one. Default: Return clicks it and it shows as the default (AppKit `keyEquivalent` `"\r"`, GTK `suggested-action`, Qt `Accessible.defaultButton`, WinUI `AccentButtonStyle`). Cancel: Escape clicks it (AppKit `keyEquivalent` Escape); the others show a normal button. Destructive: GTK `destructive-action`, AppKit `hasDestructiveAction`; Qt and WinUI have no such style. Keep it on the node: no toolkit tells every role apart. |
| `ButtonStyle` | Button | Automatic, Bordered or Borderless; sent only if the app chose one. Automatic draws as Bordered. Borderless: AppKit `bordered` off, GTK `has-frame` off, Qt `flat`, WinUI `SubtleButtonStyle`. On WinUI one XAML style carries both: Borderless wins over Default. Report back what was sent. |
| `Tweak` | built-in widgets | The app's raw settings, a payload of your own (see §8a). Keep it on the node, report it back, and run it after the node's other props: on `Create` after all of them, and after every later `SetProp`, so what it sets wins. |
| `ScrollAxes` | ScrollView | Which scrollbars / scroll directions exist. |
| `ScrollBars` | ScrollView | Whether the scrolling axes show scroll bars, as the platform shows them (overlay or not). Without, it still scrolls by wheel, trackpad and touch: AppKit turns the scrollers off, GTK's policy is `External`, Qt's `AlwaysOff`, XAML's visibility `Hidden` (not `Disabled`, which stops scrolling). There is no "always shown": on macOS that's the user's setting. AppKit can't tell the axes from hidden scrollers, so it keeps them on the node. |
| `Rows` | List | The rows' keys, in order (§8b). |
| `EstimatedRowHeight` | List | How high rows are likely to be, for platforms that size rows before showing them. |
| `Row` | Container | This container is the host of that row of its List. |
| `SelectionMode` | List | None, Single or Multiple. It can change while the list shows: keep what the platform keeps of the selection, never more than the new mode holds (none, or one row), and **report `Changed(Rows)`** if rows were let go. |
| `ListStyle` | List | Automatic, Plain or Framed; sent only if the app chose one. Automatic draws as Plain. Report back what was sent: no toolkit tells Automatic from Plain. |
| `Selected` | List | The selected rows. Setting it **must not** emit `Changed`. |
| `Custom` | Custom | The widget's props and definition. Native render: call its `update` when they differ. Drawn: just keep them for `native_state`. See §8a. |
| `Drawing` | Custom (drawn) | The display list to rasterize. Redraw. |
| `Native` | Native | Your own payload type: on `Create`, the factory; later, re-apply its updates. |

## 4. Events

Native callbacks **only** call `events.emit(id, event)` on the `EventSink` given to `init`. The `Ui` drains the queue at a safe time, so never call `Ui` methods from a callback: you may be inside a `Ui` borrow.

| Event | Emit when | Don't emit when |
|---|---|---|
| `Click` | a button is pressed (mouse, keyboard or accessibility) | |
| `Changed(Text)` | the user (or assistive technology) edits a text or password field | the core set the value. **GTK `changed` and WinUI `TextChanged` and `PasswordChanged` fire on programmatic sets**, so block or ignore them during `SetProp`. Qt's `textEdited` is the user's only. |
| `Changed(Bool)` | the user toggles a checkbox or switch | the core set `Checked`. **GTK `toggled`/`notify::active` and WinUI `Checked`/`Unchecked`/`Toggled` fire on programmatic sets**, so guard them. Qt's `toggled` is the user's only (`checkedChanged` is anyone's). |
| `Changed(Number)` | the user (or assistive technology) moves a slider, or steps a spin box or commits a number typed into it (Return, or leaving the field: not every keystroke), rounded to a whole number and kept in its range | the core set `Number` or `Range`. **GTK `value-changed` and WinUI `ValueChanged` fire on programmatic sets and clamps**, so guard them. Qt's `moved` is the user's only. |
| `Changed(Index)` | the user (or assistive technology) chooses a select's option | the core set `SelectedIndex` or `Options`. **GTK `notify::selected` and WinUI `SelectionChanged` fire on programmatic sets**, so guard them. Qt's `activated` is the user's only (`currentIndexChanged` is anyone's). |
| `Submit` | **Return/Enter** in a text or password field (GTK `activate`; WinUI `KeyDown` with `Enter`) | editing ends in other ways: Tab, a click elsewhere, focus loss. AppKit's field action does fire then; that was a real bug. |
| `FocusIn` / `FocusOut` | keyboard focus moves, **from any source** (click, Tab, code): out for the old control first, then in for the new | |
| `Scrolled(offset)` | a ScrollView's or List's offset changes, by the user **or** by `ScrollTo` | |
| `Changed(Rows)` | the user (or assistive technology) changes a List's selection, including rows deselected because they were removed or the `SelectionMode` can't hold them | the core set `Selected` |
| `RowShown(key)` | a List realises a row (it's in view, or about to be) | it already had |
| `RowHidden(key)` | a List lets go of a row it had shown | a reload shows it again right away: report only the difference |
| `RowActivated(key)` | a List row is double-clicked, or Enter is pressed on it | |
| `RowWidth(width)` | a List gives its rows a width other than its own (legacy scroll bars, insets): once it's known, and when it changes | |
| `WindowResized(size)` | the window's content area changes size (report the content size, excluding any menu bar you placed in the window) | |
| `WindowCloseRequested` | the user asks to close a window. **Don't close it**: the app decides, and the core sends `Destroy`. | |
| `MetricsChanged` | text scale, scale factor, theme or contrast changes | |
| `Remeasure` | a widget's natural size changed on its own, e.g. an image the platform decoded in the background | |
| `Pointer(event)` | primary button down / up on a **drawn** custom widget, in its coordinates | |
| `Custom(value)` | a native render or native view emits (through your `Emitter`) | |
| `SurfaceReady(handle)` | a `GpuSurface`'s native surface exists (§8c): once, before any `SurfaceResized` | |
| `SurfaceResized(size)` | a `GpuSurface`'s size in pixels or its scale changes; set it on the handle too (`set_size` says whether it changed) | its size is empty |
| `ContextMenuItem(id)` | the user (or assistive technology) chooses an item of the node's context menu | the core set `ContextMenu`. XAML and Qt toggle a check item themselves when it's clicked: put back the app's state before reporting it. |

Focus tracking needs one global observer, not per-widget guesses. Examples: AppKit uses KVO on `NSWindow.firstResponder`, GTK can use `notify::focus-widget` on the window, Qt Quick has the window's `activeFocusItemChanged`, and WinUI uses a bubbling `GotFocus` on the window's root. WinUI raises it asynchronously, so its backend also reports the focus moves it makes itself right away, and drops the late event. Map the focused native object to the nearest known node by walking up its parents. Composite widgets (a text field's inner editor, a scrolled window's viewport) put focus on children you didn't create.

## 5. Measuring

`measure(id, request) -> Size` is called **synchronously during layout**, after the current batch's structure and props have been applied. It's only called for leaves; containers are never measured.

- `known_width` / `known_height`: already fixed. Measure the other axis given them, and return the known value unchanged.
- `available_width` / `available_height`: `Definite(w)` (wrap text to `w`), `MinContent` (the narrowest sensible width, e.g. the longest word) or `MaxContent` (no wrapping).
- Return logical units, rounded up (`ceil`), so text is never clipped by a fraction.
- Text inputs often have no intrinsic width; AppKit uses 200. Use something sensible and consistent.

Platform hints:
- **GTK:** `widget.measure(Orientation, for_size)` gives the minimum and natural sizes. Use natural for max-content and minimum for min-content.
- **Qt Quick:** `implicitWidth`/`implicitHeight`, right as soon as an item exists or its text changes, with no polish in between. Wrapping text: set `width` and read `implicitHeight`. Min-content: `Text.WordWrap` at width 1 leaves the longest word as `contentWidth` (`Text.Wrap` would break it). Restore the frame's width afterwards.
- **WinUI:** `element.Measure(available)` then `DesiredSize`. Elements must be in a live tree: before that, a `Button` measures `0 × 19`. The frame's `Width`/`Height` must be lifted to Auto (NaN) for the call, because `Measure` returns an explicit size.

Known gap on AppKit: min-content falls back to max-content. If your platform gives min-content cheaply (GTK does), implement it properly.

## 6. Metrics

`metrics()` returns:
- font sizes for each `TextStyle` from the platform type ramp (body 13pt on macOS, around 14–15 on WinUI and GNOME);
- the spacing tokens `xs…xl` in the platform's design language (AppKit: 4/6/8/12/20; pick yours from the GNOME HIG or Fluent);
- the scale factor, dark mode, high contrast and reduced motion.

Emit `MetricsChanged` when any of these change.

## 7. Test hooks: perform, synthesize, native_state, capture

These make one test suite run against every backend.

- **`perform(id, action)`**: do what assistive technology would.
  - `Activate`: press the button or toggle the control. Prefer the platform's accessibility press (AppKit `accessibilityPerformPress`; its return value lies for offscreen windows, so the result is ignored).
  - `SetValue(text)`: set the field's text, then emit `Changed(Text)` yourself, because an assistive technology edit is a user edit. WinUI's `PasswordBox` has no settable Value pattern: set `Password`. On a select, choose the first option with that text, as picking it from the pop-up would, and report `Changed(Index)` (`Unsupported` if there's none). Don't open the pop-up: that starts a modal loop.
  - `Increment` / `Decrement` on a slider: step it as the platform's accessibility or keyboard does (VoiceOver's increment, a GTK step, UIA RangeValue by `SmallChange`, Qt's `increase()` and `moved`), which reports `Changed(Number)`. `SetValue(text)` on a slider moves it to that number, as a drag would. On a `NumberInput`, `Increment` / `Decrement` do what its buttons do (VoiceOver's increment on the stepper, GTK's `spin`, UIA RangeValue by `SmallChange`, Qt's `increase()` and `valueModified`), and `SetValue(text)` commits that number as if typed, rounded.
  - `Focus`: move keyboard focus to the control.
  - `Select` on a List's row host: select that row (the only selected one), as a screen reader's select does, and report `Changed(Rows)` on the List. `Unsupported` if the list's `SelectionMode` is None.
  - `Activate` on a row host: report `RowActivated` on the List.
  - `ContextMenuItem(id)`: choose that item of the node's context menu, as a screen reader does once it has shown the menu, through the item's own path (AppKit's `performActionForItemAtIndex:`), which reports `ContextMenuItem(id)`. Never open the menu. `Disabled` for a disabled item or control (disabled controls show no menu), `Unsupported` if there's no such item. The test kit finds the node the way a right-click would: the nearest one up the tree with a menu.
  - Return `ActionError::Disabled` for disabled controls, `ReadOnly` for `SetValue` on a read-only field, and `Unsupported` for actions that don't apply.
- **`synthesize(id, input)`**: behave as close to real input as the platform allows.
  - `Key(Char | Backspace | Enter | Tab)` on text and password fields must go through the platform's text-editing path, so the real signals fire. AppKit drives the field editor (`insertText:`, `doCommandBySelector:`), a secure one for password fields. WinUI's `PasswordBox` has no caret or selection to edit through, so its backend edits `Password` at the end, where typing into a focused box goes, and `PasswordChanged` reports it.
  - If the field wasn't focused, focus it and **put the caret at the end**: focusing selects all, and the first keystroke would replace everything.
  - Any key on a read-only field: `ActionError::ReadOnly`, before focusing it. Nothing can be typed into one anywhere, and AppKit's can't take keyboard focus, so no platform delivers the keys.
  - Enter or Space on buttons, Space on toggles.
  - `Scroll { dx, dy }` scrolls a ScrollView or List like a scroll wheel would, clamped.
  - `Up`, `Down`, `Home`, `End` and `Enter` on a List go through the list's own key handling: they move the selection and scroll to it, or activate the selected row.
  - `Click(point)` on **drawn** custom widgets: a real down/up pair through your drawn view's event handlers. `Unsupported` elsewhere; native controls often track the mouse in a modal loop.
- **`native_state(id)`**: **read back from the widget** what it actually shows: text, title, value, placeholder, checked, enabled, frame, children (in native order), focused, and scroll offset. Only cache what the platform can't report (AppKit caches the text style and variant). After every settle, the test harness compares this with the core and fails on any difference. This check has caught every serious backend bug so far. A window's children include its toolbar items, after its content; a `ToolbarItem` reports the rect the toolbar gave it, in the coordinates of the window's content (above it, so at a negative y), and `Rect::ZERO` while it's hidden, whether it's empty or the toolbar put it in an overflow menu.
- **`capture(id, reply)`**: offscreen RGBA8 at backing scale, rows top to bottom. Reply when the image is ready, right away if possible. Examples:
  - AppKit: `cacheDisplayInRect:toBitmapImageRep:`, which replies immediately. The test window is never key, so captures show the unfocused-window look (grey default buttons).
  - GTK: `gtk::WidgetPaintable` + snapshot + `render_texture` (Cairo renderer), then download. Replies from the frame clock's `after-paint`, once the widget is mapped and laid out.
  - WinUI: `RenderTargetBitmap.RenderAsync`, then `GetPixelsAsync`, replying from the completion.
  - Qt Quick: `QQuickWindow::grabWindow`, which renders right away (in software on the offscreen platform), cropped to the node.

**`TestHooks::close_window`** clicks a window's close button the way the user would, through the platform (`performClose:`, `gtk::Window::close`, `QQuickWindow::close`, a posted `WM_CLOSE` on WinUI: XAML's `Window.Close` skips `Closing`), so the platform reports `WindowCloseRequested` itself. Never close the window: whether it closes is the app's call, and the core destroys it if so.

**`TestHooks::settle`** runs after every settle and while a test awaits something the platform completes (a capture). Use it to let the platform catch up without blocking: GTK presents windows there and dispatches what its main context has ready (allocations, adjustments, focus). AppKit does everything synchronously and leaves it empty.

## 8. Services

Implement `Services`. **Never block**: reply later, from the platform's completion callback.

| Service | AppKit | GTK 4 | WinUI 3 | Kirigami |
|---|---|---|---|---|
| clipboard read (async reply) | `NSPasteboard` (replies immediately) | `gdk::Clipboard::read_text_async` | `Clipboard.GetContent().GetTextAsync()` | `QClipboard` (replies immediately) |
| clipboard write (async reply, can fail) | `NSPasteboard` (replies immediately) | `gdk::Clipboard::set_text` | `Clipboard.SetContent` (throws while another process holds the clipboard: reply `Err`) | `QClipboard` (replies immediately) |
| alert | `NSAlert` sheet on the parent | `gtk::AlertDialog::choose` | `ContentDialog` (one at a time per window) | `Kirigami.PromptDialog` in the window's overlay |
| open / save | `NSOpenPanel` / `NSSavePanel` sheets with `UTType` filters | `gtk::FileDialog` (`open`/`open_multiple`/`save`) with `gtk::FileFilter` | `FileOpenPicker` / `FileSavePicker` (need the window handle: `InitializeWithWindow`) | Qt Quick's `FileDialog` / `FolderDialog` (Plasma's own through its platform theme) |
| menus | the global `NSMenu` bar: app menu, the app's File, Edit, the rest; a window's own menus while it's main | the header bar's primary menu (a `gio::Menu` section per menu) in each window | a `MenuBar` in each window | a `Kirigami.GlobalDrawer` shown as a menu (`isMenu`) in each window |
| submenus | `NSMenuItem.submenu` | `gio::Menu::append_submenu` | `MenuFlyoutSubItem` | nested `Kirigami.Action`s |
| check / radio items | `NSMenuItem.state` | a stateful action (boolean; a radio item's holds its id, its target) | `ToggleMenuFlyoutItem` / `RadioMenuFlyoutItem` (`GroupName`) | `checkable` actions; a radio group in one `QQC2.ActionGroup` |
| roles (About, Settings, Quit) | the app menu, AppKit's titles and shortcuts | the last section: Settings, About, Quit | where the app put them | the end of the drawer: Settings, About, Quit |

- **`parent: None`** means the active window: AppKit uses the key window, then the main window. Only fall back to app-modal if there is no window.
- **Menus inside the window** (GTK without a global menu, WinUI): the menu bar takes space the core doesn't know about. Put it above your content host, and report the **remaining** content size in `WindowResized`.
- Keep the platform's standard menus (Quit, Edit with Cut/Copy/Paste/Undo) and leave their enabling to the platform. The app's own items follow its `enabled` state.
- **`set_menu(None, …)` is the app's menus; `set_menu(Some(window), …)` is that window's own,** shown with the app's, and an empty bar removes them. Menus in each window show `MenuBarData::for_window`: a modal window (a dialog, `Prop::Modal` in its `Create`) shows only its own, without your Quit. A window's menus usually arrive before its `CreateWindow` is applied, and may arrive after it's destroyed: keep them by `NodeId`.
- **Update in place when you can:** if `MenuBarData::same_structure` holds, only enabled and checked states changed; rebuilding would close an open menu.
- **Items with a role** go where the platform puts them: `MenuBarData::take_role` takes them out of the app's menus, tidying separators. A Quit item replaces your own Quit. Where the platform has no place for them, leave them.
- **Check and radio items** are drawn by the platform. If it toggles an item itself on a click, put the app's state back: the core sends the new state when the app changes it, and only the user's choice may call `activate`.
- `Shortcut::primary` is Ctrl on GTK, Qt and WinUI.

## 8a. Escape hatches: custom widgets, native views and tweaks

The core does the shared work; a backend supplies three things. AppKit's are in `mitsuami-appkit/src/custom.rs`, GTK's in `mitsuami-gtk/src/custom.rs`, WinUI's in `mitsuami-winui/src/custom.rs`. If your controls may size themselves or skip change events for some sources, see ARCHITECTURE.md §16, "M4 on WinUI".

1. **A `NativeRender` trait** for custom widgets, in your crate, shaped like AppKit's: `type View`, `create(props, cx)`, `update(view, old, new)`, and optional `measure`, `read` (read the props back from the widget, for the mirror check) and `perform`. Provide `native::<W>() -> Renderer<W>`: wrap a type-erased render in an `Opaque` and pass it to `Renderer::native`. On `Create` of `Custom(_)`, `find_prop!(props, Custom)`: if `custom.native()` is `Some`, downcast it to your render and create the view; otherwise the widget is drawn.
2. **A drawn view** that rasterizes `DisplayList`s (fill and stroke of rects, rounded rects, ellipses and paths) with the platform's 2D API. Resolve the semantic `Color`s at draw time, so they follow the appearance. Report primary-button `Pointer` down/up in the widget's coordinates, and support `SyntheticInput::Click`. Don't measure drawn widgets (the core does); `native_state` reports `Custom` and `Drawing` as last received.
3. **`NativeView::<platform>(factory)`** for app-supplied widgets, with a payload type of your own in `Prop::Native`. Run the factory on `Create`, and apply the updates on `Create` and on each `SetProp`.
4. **`tweak` and `tweak_with`** for raw settings of built-in widgets: `tweak(|b: &NSButton| …)` returns a `Tweak<W>` (the core's), typed through a `Tweakable` trait that names your control for each widget (`Button` → `NSButton`, `gtk::Button`, the QML item, XAML's `Button`; `Checkbox` → `NSButton`, `gtk::CheckButton`, the QML item, XAML's `CheckBox`; `Switch` → `NSSwitch`, `gtk::Switch`, the QML item, XAML's `ToggleSwitch`; `Select` → `NSPopUpButton`, `gtk::DropDown`, the QML item, XAML's `ComboBox`; `Slider` → `NSSlider`, `gtk::Scale`, the QML item, XAML's `Slider`; `NumberInput` → `mitsuami::appkit::NumberField` (its `field()` and `stepper()`), `gtk::SpinButton`, the QML item, XAML's `NumberBox`; `Image` → `NSImageView`, `gtk::Picture`, the QML item, XAML's `Image`; `Progress` → `NSProgressIndicator`, `gtk::ProgressBar`, the QML item, XAML's `ProgressBar`; `Spinner` → `NSProgressIndicator`, `gtk::Spinner`, the QML item, XAML's `ProgressRing`; `TextInput` → `NSTextField`, `gtk::Entry`, the QML item, XAML's `TextBox`; `PasswordInput` → `NSSecureTextField`, `gtk::PasswordEntry`, the QML item, XAML's `PasswordBox`; `ScrollView` → `NSScrollView`, `gtk::ScrolledWindow`, the QML item, XAML's `ScrollViewer`; `List` → `NSTableView`, `gtk::ListView`, the QML `ListView`, XAML's `ListView`: the list view, not the scroll view the node stands for, so run a list's tweak on that; `Text` → `NSTextField`, `gtk::Label`, the QML item, XAML's `TextBlock`). Wrap a closure over the node's view in an `Opaque` and pass it to `Tweak::new`; `tweak_with` makes it again from a value, whenever that changes. The backend runs it as `Prop::Tweak` says (§3).

Also:
- A context (`AppKitCx`) for factories: the main-thread marker, an `Emitter` that queues `UiEvent::Custom(AnyValue::new(event))` for the node, and a way to hear a control's actions whose targets live as long as the node.
- **`perform` on custom widgets:** call the render's `perform`. Return `Unsupported` when it doesn't handle an action; the core then emits the event the widget's shared definition maps it to. **On native views:** perform on the accessibility element the way the screen reader would. That can be a child of the view (AppKit: an `NSStepper`'s cell).
- Put the widget-specific `objc2`-style bindings on your crate's public API (AppKit re-exports `objc2`, `objc2_app_kit`, `objc2_foundation`), so apps use the same versions.

## 8b. Lists

A `List` is the platform's list control, and the platform virtualises it: it scrolls, decides which rows to realise, recycles them, and draws and handles the selection. The core builds what's in the rows.

- **The data is `Prop::Rows`:** the rows' keys, in order. When `Rows` changes, the selection must follow the rows, not their indexes: diff the keys into native inserts, removes and moves, or reload and select the rows that stayed selected (AppKit reloads). Report `Changed(Rows)` if selected rows went.
- **Report the rows you realise:** `RowShown(key)` when the platform prepares a row (a cell asked for, a delegate created, a container realised), `RowHidden(key)` when it recycles it. The core mounts each shown row as a `Container` with `Prop::Row(key)`, inserted as the List's native child, and disposes it when hidden. The List's native children are those hosts, in row order. A reload that shows the same rows again must not hide and show them: report only the difference, or their state goes.
- **Cells:** give a realised row an empty cell until its host arrives (`Insert`), then put the host in it. A host's `SetFrame` is its size, at origin 0; make the row that high, and place it where the platform places rows.
- **Heights:** rows the platform hasn't shown yet need a height from somewhere if it sizes rows up front (AppKit): `EstimatedRowHeight` if the app gave one, or else the first row measured. **Keep the heights of rows you've measured** when you let them go, and keep the estimate steady: guesses that change as rows come and go move rows in and out of view, and the rows shown flip back and forth.
- **Row width:** the core lays rows out at the list's width. If rows get another width (legacy scroll bars, a scroll bar's own column as on Breeze, a `Framed` list's border, insets), report it with `RowWidth`.
- **Platforms keep rows of their own.** Which rows the platform realises is its call, and the tests allow for it: AppKit prepares a few around the view, GTK about 200, and GTK keeps its cursor row and selected rows bound wherever it scrolls. Where rows go when rows are inserted above the view is its call too (GTK keeps the rows in view where they were).
- **Build rows before drawing.** Report rows as soon as the platform decides them, ideally inside the `apply` or scroll that changed them, so the core builds them in the same run-loop turn: platforms that realise rows in a layout pass (AppKit) run it at the end of `apply` and in `settle`.
- **Callbacks come at any time.** A table can ask for cells, heights and counts in the middle of your own `apply` (a reload, a scroll, a resize). Keep the list's data (keys, heights, hosts, cells) in a small `Rc<RefCell<…>>` of its own that the data source reads, never your backend's main state or the `Ui`, and only `emit` from there.
- **`native_state` of a row host** reports the rect the platform gave that row, in the list's content; the core uses its position (that's where frames inside rows, visibility and `scroll_into_view` come from), and the mirror check compares its size with the host's. The List reports its `Rows`, `SelectionMode` and `Selected` as the native control shows them, and its `ListStyle` as last set.
- **Focus:** the List itself takes focus (it's in the Tab order), as the native control does.

## 8c. GPU surfaces

A `GpuSurface` is a native surface the app presents to with its own GPU API, from its own thread. The backend makes the native surface, places it over its widget and reports its size; it never draws in it.

- **Hand it out as a `SurfaceHandle`:** implement `NativeSurface` (the `raw-window-handle` window and display handles) for your native surface and wrap it with `SurfaceHandle::new`, then report `SurfaceReady(handle)`. Make it when the platform can: AppKit on `Create`, Wayland once the window has a surface (when the widget is mapped), WinUI once the node is in a window.
- **The handle owns the surface:** it lives until the app's last handle is dropped, after the node is destroyed too, since a GPU surface made on it must not outlive it. `Destroy` only stops it showing and reporting, and must keep the window from taking it down (WinUI moves its child window under `HWND_MESSAGE`). The last handle may be dropped on any thread: free what must be freed on the UI thread there (AppKit's main queue, WinUI's `WM_CLOSE`). Don't let the widget hold a handle to itself past its node, or it never goes.
- **Follow the widget:** where the surface is a window of its own (a subsurface, a child window), place it after each of the window's frames, so layout, scrolling and resizes all move it, and hide it while the widget has no size or isn't shown. It takes no input, so the toolkit keeps the pointer.
- **Report its size in pixels and its scale** as `SurfaceResized`, and set it on the handle first (`SurfaceHandle::set_size`), so a render thread that reads it sees it as soon as the app does.
- **Measure it as nothing:** it's as large as the layout makes it. `native_state` reports its `Label`.

## 9. Tab order

`SetFocusOrder` gives the window-wide order. Platforms differ in how to impose it:

- **AppKit:** turn off `autorecalculatesKeyViewLoop` and link `nextKeyView` into a loop.
- **GTK 4:** there is no "next widget" pointer. The window's content host overrides the `focus` vfunc: for Tab and Shift+Tab it `grab_focus`es the next widget in the order that accepts focus, wrapping around; other directions keep GTK's behaviour.
- **WinUI:** `TabIndex` is scoped to each container, so it can't express a window-wide order across nested hosts. Handle Tab in `PreviewKeyDown` on the window's root and focus the next control in the order yourself, as on GTK.
- **Qt Quick:** its focus chain follows item order within each parent. An event filter on the window handles Tab and Shift+Tab along the core's order, skipping disabled and hidden items; popups (dialogs, menus) keep Qt's own chain.

The conformance tests use three controls arranged so that reading order and position on screen disagree (RTL rows, absolute positioning, `tab_index`). An order based on position fails them, as it should.

## 10. The run loop

`run(setup)` owns the platform's main loop (see `mitsuami-appkit/src/app.rs`):

1. Create the platform app, your backend and `Ui::new(backend)`. Install the standard menus with `ui.set_menu(MenuBar::new())`.
2. `ui.set_commit_scheduler(wake)`: called when something changed; make the loop turn soon.
3. `ui.set_waker(Arc<dyn Fn() + Send + Sync>)`: **thread-safe** wake-up, called when background work completes a task.
4. `setup(&ui)` (the app creates its windows), then `ui.tick()`, **then** show the windows, so nobody sees an unlaid-out frame.
5. Call `ui.tick()` whenever the loop is about to sleep, and again after each wake-up. After each tick, re-arm **one** timer for `ui.time_to_next_timer()`.
6. Stop when `ui.windows()` is empty.

Platform hints:

| Step | AppKit (done) | GTK 4 | WinUI 3 (done) | Kirigami (done) |
|---|---|---|---|---|
| tick before sleeping | `CFRunLoopObserver` (BeforeWaiting, common modes) | an idle source that is re-added when scheduled (`glib::idle_add_local_once`), guarded by a "scheduled" flag | our own `PeekMessage` loop (no `Application::Start`) ticks before it sleeps; ticks are also scheduled with `DispatcherQueue.TryEnqueue`, which runs inside modal loops (live resizing) | the event dispatcher's `aboutToBlock` |
| thread-safe wake | `CFRunLoopWakeUp` | `glib::MainContext::default().invoke(...)` to schedule the tick | `PostThreadMessageW(WM_NULL)` to the UI thread | `QAbstractEventDispatcher::wakeUp` |
| timer | one `CFRunLoopTimer`, re-armed | a `glib::timeout_add_local_once` replaced on re-arm | the timeout of `MsgWaitForMultipleObjectsEx` | one single-shot `QTimer`, re-armed |
| stop | `stop:` **plus an empty posted event** (otherwise it waits for the next real event) | `app.quit()` | leave the loop, then release XAML objects while XAML still runs | `QCoreApplication::quit` (`quitOnLastWindowClosed` off: the core decides) |

Never call `tick()` from inside a widget callback; the loop calls it.

## 11. Sync and async in the contract

The contract is async wherever **any** platform might complete the work later. Reply-based methods take a `Reply<T>` (a `FnOnce(T)`). Call it exactly once, from a completion handler if needed, or right away when the platform is synchronous.

| Async (reply) | Why |
|---|---|
| `capture` | WinUI renders to bitmaps asynchronously |
| `clipboard_text`, `set_clipboard_text` | GTK and WinUI clipboards are async; writes can fail |
| `alert`, `open_file`, `save_file` | the user answers later |

| Sync | Why it can stay sync |
|---|---|
| `measure` | layout needs the answer now; every platform measures synchronously |
| `native_state`, `metrics` | plain property reads |
| `apply`, `set_menu` | instructions with no answer |
| `perform`, `synthesize` | the `Result` only says whether the action was accepted; its effects arrive later as events |

If your platform can only do one of the sync ones asynchronously, raise it: we change the contract rather than blocking the UI thread.

## 12. Testing your backend

```sh
MITSUAMI_NATIVE=1 cargo test                    # all app suites on your backend
MITSUAMI_NATIVE=1 cargo test -p mitsuami --test conformance   # the contract
MITSUAMI_SHOW_WINDOWS=1 MITSUAMI_NATIVE=1 cargo test          # watch it
cargo run -p mitsuami --example run_loop_smoke  # timers, background wake-up, exit (must exit by itself)
cargo run -p mitsuami --example showcase        # look at it
```

- **`tests/conformance.rs` is the contract.** Get it green first; the other suites mostly follow.
- **Platforms that report asynchronously** implement `TestHooks::pump`: tests call it while settling, and while a test awaits native work (a capture). WinUI needs it. Report what your backend causes itself right away, rather than waiting for the platform's event, so settles stay deterministic.
- **On Kirigami**, the backend and the test kit need their features: `MITSUAMI_NATIVE=1 cargo test -p mitsuami -p mitsuami-kirigami --features mitsuami/kde,mitsuami-test/kde,mitsuami-kirigami/qt`.
- **On Windows**, build with the MSVC toolchain: `cargo +1.96-x86_64-pc-windows-msvc test` if your default host is gnu.
- **Mirror checks** run after every settle, comparing native and core children, props, frames, focus and scroll offsets. A failure names the node and the difference.
- **Visual baselines** are stored per backend and machine image in `tests/visual/<name>/<image>/` (ARCHITECTURE.md §12). The first run creates them; look at them. CI records its own: a failing run uploads them, and `.github/scripts/accept-snapshots.sh <run id>` accepts them.
- **Headless-only tests** (fake metrics, simulated system changes) are skipped in native runs.
- **Your real services** aren't exercised by app tests, which use a scripted fake. Copy `mitsuami-appkit/tests/services.rs`: a private clipboard if possible, the real menu structure plus an activation (submenus, check marks, roles, a window's own menus), an alert answered through its real button, and a cancelled file dialog.

## 13. Suggested order

1. Window + Container + Text, `SetFrame`, `measure` → the counter layout test passes natively.
2. Button, events, `perform`, `native_state` → the counter suite passes.
3. TextInput, Checkbox and Switch, including the "no events on programmatic set" guards and Enter-only submit → the forms suite passes.
4. Focus tracking, `SetFocusOrder`, `synthesize` → the Tab and focus conformance tests pass.
5. ScrollView → the scrolling conformance tests pass.
6. `run()` with tick, waker and timer → `run_loop_smoke` exits by itself; the showcase works.
7. Services + native services checks.
8. `capture` + visual baselines.
9. The drawn view, then `NativeRender` and `NativeView` (§8a) → the `escape_hatches` suite passes. The example has one widget from each platform (rating: AppKit, lock: GTK, pips pager: WinUI). Add the native render for yours (`examples/escape_hatches/<widget>/<os>.rs`), and for any other of the three your platform has as a real control (WinUI has a rating control too), and name it in the widget's `Render` impl with `native::<W>()`. For the others, prefer an ad hoc render built the way your platform's apps build that widget (`ad_hoc::<W>()`) over the drawn one; only a real platform control counts as native.
10. `List` (§8b) → the `lists` and `contacts` suites pass.

When something in the contract doesn't fit your platform, **change the contract rather than working around it**, and update the other backends and this guide. Capture and the clipboard became async for exactly this reason.
