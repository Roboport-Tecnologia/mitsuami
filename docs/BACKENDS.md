# Writing a mitsuami backend

This guide is for writing a platform backend: what the core sends, what it expects back, and the rules the test suites enforce. The four backends follow it: AppKit (`crates/mitsuami-appkit`), GTK 4 (`crates/mitsuami-gtk`), WinUI 3 (`crates/mitsuami-winui`) and Qt Quick with Kirigami (`crates/mitsuami-kirigami`), which shows a toolkit without Rust bindings: a small C++ layer, and QML created per node. AppKit's is the reference the others were written from. Many of the rules here were learned the hard way; [ARCHITECTURE.md](ARCHITECTURE.md) §13–§15 has how each platform does each widget, and why.

**Background:** read [ARCHITECTURE.md](ARCHITECTURE.md) §3–§6 first.

## 1. The model

The core owns the widget tree, the layout and every decision. A backend is a **mirror**:

- It creates native widgets when told to, and places them where it's told.
- It reports what the user did.
- It answers questions: how big is this, what does it show, what does it look like.

The backend **never lays anything out** and **never calls back into the `Ui`** from native callbacks; it queues events instead. All coordinates are **logical units** (points, effective pixels, GTK's logical pixels, Qt's device-independent pixels), **relative to the native parent**, with the **origin at the top-left**.

Native wins (ARCHITECTURE.md §1): widgets behave, size and respond as the platform's own do. Props are passed through for the platform to use as it uses them, and ignored where it has no equivalent. Don't make your platform look like another.

## 2. What you implement

| Piece | Where | Reference |
|---|---|---|
| `Backend` | `mitsuami_core::Backend` | `mitsuami-appkit/src/backend/` |
| `Services` (clipboard, dialogs, menus) | `mitsuami_core::services::Services` | `mitsuami-appkit/src/services.rs` |
| `TestHooks` on your shareable handle | `mitsuami_core::TestHooks` | `impl TestHooks for AppKitHandle` |
| `run(info, setup)`: the app's run loop | your crate | `mitsuami-appkit/src/app.rs` |
| `init_for_tests()` | your crate | same file |

You also touch three places outside your crate:

1. **`crates/mitsuami/Cargo.toml`, `crates/mitsuami/src/app.rs`:** `App::run` calls your `run`, behind a `cfg(target_os = …)`.
2. **`crates/mitsuami-test/Cargo.toml`, `crates/mitsuami-test/src/driver.rs`:**
   - Add a `native(appearance)` that builds your backend for tests: offscreen unless `MITSUAMI_SHOW_WINDOWS=1`, recording commands, the appearance the test asks for (light, or a story's dark variant) whatever the system's, and a private clipboard if the platform allows it.
   - Add your platform to `native_available()`.
3. **`crates/mitsuami-<name>/Cargo.toml`:** put native dependencies under `[target.'cfg(target_os = "…")'.dependencies]`, so the workspace still builds everywhere.

A second toolkit on an OS is a cargo feature rather than a `cfg(target_os)`: Kirigami next to GTK on Linux is `mitsuami`'s `kde` feature (which wins over the default `gtk`), `mitsuami-test`'s `kde` feature, and the `kde` and `gtk` arms of `platform!`. A backend whose native libraries may be missing where the workspace builds keeps them behind a feature of its own (`mitsuami-kirigami`'s `qt`), so it builds empty without it.

Keep a shareable handle (`Rc<RefCell<State>>` inside the backend, with a cloneable handle outside). `Ui::new` takes ownership of the backend, but tests, services and the run loop still need to reach it.

## 3. Commands

`Backend::apply(&mut self, batch: &[Command])` receives batches. **Order within a batch matters**, and the core guarantees:

- A node is created before it's inserted, and a subtree's root is removed before it's destroyed.
- `Create` carries the widget's initial props, reactive ones included: a custom widget or native view can't be made without its props.
- Structure and props come first, ending with `SetFocusOrder`, then any `Focus` commands, once the nodes are in their windows.
- Frames come in a second `apply` per commit, because layout measures widgets that must already exist. Drawn widgets' `SetProp(Drawing)` comes there too (drawing needs the size), and so does `SetWindowSize` for a window whose height follows its content.

Validate as you go. Panic on protocol violations such as an unknown node, a double insert, or a second child in a `ScrollView`. The headless backend does this too; it's how contract bugs surface.

| Command | What to do |
|---|---|
| `Create { id, kind, props }` | Create the native widget and apply `props`. **Give it a zero frame**: the core only sends frames that differ from the last one it sent, so a widget born with its own frame stays wrong (AppKit labels do this; Qt Quick items follow their implicit size). |
| `SetProp { id, prop }` | Apply the prop (§3.2). For `Value`, skip the update if the widget already shows it, so the caret and IME composition survive. |
| `Insert { parent, child, index }` | Attach at `index` among the parent's native children. **ScrollView:** its single child is the scrolled content (AppKit: `documentView`). **List:** the child is a row host, put in its row's cell (§10). **Tabs:** the child is a page host, the page at `index` (§11.3). **Window:** a `ToolbarItem` or a `Sidebar` is window chrome (§11). |
| `Remove { parent, child }` | Detach only. |
| `Destroy { id }` | Free the widget. It comes for every native node of a removed subtree, children first; the root has already been removed. Drop observers, signal handlers and targets. |
| `SetFrame { id, frame }` | Place the widget, relative to its native parent's top-left. Never sent for windows or sidebars. A `ScrollView`'s content frame is in content coordinates. A `ToolbarItem`'s, a row host's and a `Tabs` page's frame is only its size: the platform places them (§10, §11). |
| `SetA11y { id, a11y }` | Set the accessible label, description and hidden state. |
| `SetWindowSize { id, size }` | Set the window's **content area** size (without the title bar, menu bar and toolbar, or a sidebar beside it), no smaller than its `MinSize`. Ignore it while the window is in full screen, which keeps the screen's size. The platform reports what it gives as `WindowResized`. |
| `SetFocusOrder { window, order }` | Make Tab visit `order` in sequence, wrapping around. It's window-wide, across nested containers. The platform still decides *which* controls can take focus (disabled controls, macOS's keyboard navigation setting). See §14. |
| `ScrollTo { id, offset }` | Scroll the `ScrollView` or `List` so `offset` is at its top-left. Already clamped (a `List` clamps it itself). The platform then **reports `Scrolled`**, as for a user scroll. |
| `ScrollToRow { id, row }` | Scroll the `List` just enough to show that row, with the platform's own "scroll to row". Report `Scrolled`, and the rows shown. |
| `Focus { id }` | Give the control keyboard focus. The focus change is reported through your focus tracking (§4), not by this command. |

### 3.1 Widget kinds

`WidgetKind::Fragment` (`Show`, `For`) is the core's only and never sent: its children are spliced into its parent.

| Kind | AppKit | GTK 4 | WinUI 3 | Kirigami |
|---|---|---|---|---|
| `Window` | `NSWindow` + flipped content host | `gtk::Window` with an `adw::HeaderBar` as its titlebar, the host as its child | `Window` whose `Content` is a `Grid`: a `TitleBar`, the menu bar and toolbar rows, then the `Canvas` host | `Kirigami.ApplicationWindow` with one `Kirigami.Page`, a plain `Item` as its content |
| `Container` (layout host) | flipped `NSView` subclass | `gtk::Widget` subclass that allocates children at given frames | `Canvas` (`Canvas.Left/Top`, `Width/Height`) | `Item` (children at `x`/`y`/`width`/`height`) |
| `ToolbarItem` (§11.1) | its host as an `NSToolbarItem`'s view, sized by constraints, after a flexible space; out of the toolbar while empty | a host packed at the end of the window's header bar, hidden while empty | a `Canvas` in an `AppBarElementContainer` among a `CommandBar`'s `PrimaryCommands`, collapsed while empty | a host `Item` in the `displayComponent` of a `Kirigami.Action` in the page's `actions` (`KeepVisible`), hidden while empty |
| `Sidebar` (§11.2) | a source-list `NSTableView` in the sidebar item of an `NSSplitViewController`, the window's content in the other | a `gtk::ListBox` (`navigation-sidebar`) in the sidebar page of an `adw::NavigationSplitView` | a `NavigationView`, the content host its `Content` | a `Kirigami.ScrollablePage` of `ItemDelegate`s, a first page in the window's page row |
| `Tabs` (§11.3) | `NSTabView` (top tabs), each page host in a plain view as its `NSTabViewItem`'s | an `adw::InlineViewSwitcher` over an `adw::ViewStack`, each page host a titled page (a `gtk::Notebook`, titled by `gtk::Label`s, with `TabsStyle::TabBar` or before libadwaita 1.7), in a box that stands for the node | a `Canvas` with the `SelectorBar` at its top-left and the page hosts below it, the shown one `Visible`, the others `Collapsed` | Kirigami's `NavigationTabBar` (or, with `TabsStyle::TabBar`, a `QQC2.TabBar` of `TabButton`s) over a plain item holding the page hosts |
| `Text` | `NSTextField` wrapping label | `gtk::Label` (wrap on, `xalign 0`) | `TextBlock` (`TextWrapping.Wrap`) | `QQC2.Label` (`WordWrap`); `Kirigami.SelectableLabel` when it's made `Selectable` |
| `Button` | `NSButton` push | `gtk::Button` (an `adw::ButtonContent` with an icon and caption) | `Button` (a `StackPanel` of a `FontIcon` and its caption with an icon) | `QQC2.Button` |
| `ToggleButton` | `NSButton` of the push-on-push-off type | `gtk::ToggleButton` (an `adw::ButtonContent` with an icon and caption) | `ToggleButton` (content as a `Button`'s) | checkable `QQC2.Button` |
| `MenuButton` | pull-down `NSPopUpButton`, its first item the title | `gtk::MenuButton` over a `gio::Menu` | `DropDownButton` with a `MenuFlyout` | `QQC2.Button` (`Accessible.ButtonMenu`) that pops up a `QQC2.Menu` |
| `TextInput` | `NSTextField` | `gtk::Entry` / `gtk::Text` | `TextBox` | `QQC2.TextField` |
| `PasswordInput` | `NSSecureTextField` | `gtk::PasswordEntry` (no peek icon, GTK's default) | `PasswordBox` (its reveal button, XAML's default) | `Kirigami.PasswordField` (with its show button) |
| `SearchInput` | `NSSearchField` (its action is the search) | `gtk::SearchEntry` (`search-changed` after its 150 ms delay, `activate`) | `AutoSuggestBox` (`QueryIcon` Find, no `ItemsSource`) | `Kirigami.SearchField` (`autoAccept`'s `accepted`, a short pause after typing) |
| `TextArea` | `NSTextView` in a bezelled `NSScrollView` (`scrollableTextView`) | `gtk::TextView` (wraps at words, 6 px margins) in a framed `gtk::ScrolledWindow` | `TextBox` (`AcceptsReturn`, `TextWrapping.Wrap`, vertical scroll bar `Auto`) | `QQC2.TextArea` (wraps) in a `QQC2.ScrollView` |
| `Checkbox` | `NSButton` checkbox | `gtk::CheckButton` | `CheckBox` | `QQC2.CheckBox` |
| `Switch` | `NSSwitch` | `gtk::Switch` | `ToggleSwitch` | `QQC2.Switch` |
| `Slider` | `NSSlider` (a step is tick marks) | `gtk::Scale` | `Slider` | `QQC2.Slider` |
| `NumberInput` | `NSTextField` with an `NSStepper` beside it (`mitsuami::appkit::NumberField`) | `gtk::SpinButton` | `NumberBox`, inline spin buttons | `QQC2.SpinBox`, editable |
| `Progress` | `NSProgressIndicator` bar | `gtk::ProgressBar` (pulsed by a timer while indeterminate) | `ProgressBar` | `QQC2.ProgressBar` |
| `Spinner` | spinning `NSProgressIndicator` (`displayedWhenStopped` off) | `gtk::Spinner` | `ProgressRing` | `QQC2.BusyIndicator` |
| `Separator` | `NSBox` of type separator | `gtk::Separator` | `Border` in `DividerStrokeColorDefaultBrush`, 1 epx across | `Kirigami.Separator` |
| `Image` | `NSImageView` | `gtk::Picture` | `Image` (a `BitmapImage`, or a `WriteableBitmap` for pixels) | QML `Image` (an image provider for pixels) |
| `Icon` | `NSImageView` of an SF Symbol (else a named image) | `gtk::Image` of a themed icon | `FontIcon` (a Segoe Fluent Icons glyph) | `Kirigami.Icon` |
| `GpuSurface` (§12) | `NSView` whose backing layer is a `CAMetalLayer` | `gtk::DrawingArea` keeping the space, under a Wayland subsurface or an X11 child window of our own (`mitsuami-linux`) | a focusable element keeping the space, under a child HWND | `Item` keeping the space, under a Wayland subsurface or an X11 child window of our own |
| `Select` | `NSPopUpButton` (items added to its menu) | `gtk::DropDown` over a `gtk::StringList` | `ComboBox` of `ComboBoxItem`s | `QQC2.ComboBox` |
| `RadioGroup` | `NSStackView` (vertical) of radio `NSButton`s with one target and action | vertical `gtk::Box` (`RadioGroup` role) of `gtk::CheckButton`s in one group | `RadioButtons` of strings | `ColumnLayout` of `QQC2.RadioButton`s (`autoExclusive`) |
| `ScrollView` | `NSScrollView` | `gtk::ScrolledWindow` | `ScrollViewer` | `QQC2.ScrollView` around a `Flickable` |
| `List` (§10) | view-based `NSTableView` in an `NSScrollView` | `gtk::ListView` over a `gio::ListStore` of keys | `ListView` over the keys (boxed strings), with `Canvas` cells | QML `ListView` over the keys, with `QQC2.ItemDelegate`s |
| `Group` | a layout host with an `NSBox` behind its children (the title inside at the top) | a layout host with a `heading` label and a libadwaita `card` under it behind its children | a `Canvas` with a `BodyStrongTextBlockStyle` heading and a card `Border` under it behind its children | a host with a `QQC2.GroupBox` behind the item its children go in |
| `Custom` (native render) | the render's view (`NativeRender`) | the render's widget | the render's element, in a `Border` | the render's item |
| `Custom` (drawn) | `DrawnView`: flipped `NSView` that rasterizes the display list | a `gtk::DrawingArea` rasterized with Cairo | a `Canvas` of XAML `Path`s (markup through `XamlReader::Load`, theme brushes) | a `QQuickPaintedItem` painted with `QPainter` |
| `Native` | the app's `NSView` (`NativeView::appkit`) | the app's `gtk::Widget` (`NativeView::gtk`) | the app's `FrameworkElement` (`NativeView::xaml`), in a `Border` | the app's QML item (`NativeView::qml`) |

### 3.2 Props

A prop the app didn't give isn't sent ("sent only if the app chose"), so the platform's default stays. **Setting a prop must never emit `Changed`** (§4). `native_state` reports every prop the core has; keep on the node what the platform can't read back.

**Windows**

| Prop | Notes |
|---|---|
| `Title` | The window's title. On a `Group`, its heading; empty: none, and the group takes `group_insets` instead of `titled_group_insets`. Report it back, `""` when none. |
| `Modal { owner, modality }` | Sent once, right after `Create`, before the window is shown. Show it modal the platform's way: `Window` blocks its owner (AppKit: a sheet on it), `Application` the whole app (AppKit: `runModalForWindow`, started outside the tick). Keep it above its owner (transient, owned). Destroying it ends the sheet or modal state and gives the owner back. Keep it on the node and report it. |
| `FullScreen` | Full screen the platform's own way (AppKit's own Space, `gtk::Window::fullscreen`, Qt's `WindowFullScreen` state, WinUI's full-screen presenter), and back to the window as it was. A window not shown yet takes it when it's shown. Don't report `FullScreenChanged` for it, even where it's applied later (Wayland, AppKit's animation); if the platform refuses, report the state the window kept. Report what the window shows, or while it's hidden or in a transition, what it's going to. |
| `Maximized` | Sent only if the app chose. The platform's maximize, and back to the size it had (AppKit's zoom, `isZoomed`; GTK `maximize`; Qt's `WindowMaximized` state; WinUI's `OverlappedPresenter` `Maximize` and `Restore`). A window not shown yet takes it when it's shown where maximizing would show it (WinUI). In full screen it isn't maximized. Don't report `MaximizedChanged` for it, even where it's applied later (GTK, Qt on Wayland). Report what the window shows, or while it's pending or hidden, what it will. |
| `Resizable` | Sent only if the app chose. The user can't resize a window without it; the app still can (`SetWindowSize`). AppKit's `resizable` style, GTK `resizable` (with `HeightFollowsContent`, neither is), WinUI's presenter `IsResizable` (its own presenter, kept through full screen), and on Qt a minimum and maximum at the window's size, moved with every `SetWindowSize`, as KWin holds fixed-size windows. `TestHooks::resize_window` does nothing to a window that isn't resizable. Report it back, and the other props as the app gave them while this one holds the platform's. |
| `MinSize` | The smallest **content** size the user can make it, with the window's chrome added where the platform's minimum is the whole window's (AppKit `contentMinSize`, a size request on GTK's content host, Qt's `minimumWidth` and `minimumHeight`, WinUI's `PreferredMinimumWidth` and `PreferredMinimumHeight`). A window smaller when it's set grows to it: not every platform does that itself. Cap it at what a window filling its screen's visible area would have as content, and again when the window moves to another screen. Report the app's value back while the platform holds the capped one. |
| `HeightFollowsContent` | The user can't change the window's height, which the core sets from the content with `SetWindowSize`; they still resize its width. A minimum and maximum height at the height it has, moved with every `SetWindowSize` (AppKit `contentMinSize` and `contentMaxSize`, Qt `minimumHeight` and `maximumHeight`, WinUI `PreferredMinimumHeight` and `PreferredMaximumHeight`); GTK 4 has no maximum, so there the window isn't resizable at all. `SetWindowSize` still changes the height. Report it back, and the app's `MinSize` while the lock holds the platform's minimum. |

**Text**

| Prop | Applies to | Notes |
|---|---|---|
| `Text` | Text | |
| `TextStyle` | Text | Map to the platform's type ramp: AppKit's `preferredFont(forTextStyle:)`, GTK style classes (`title-1`, `heading`, `caption`, `monospace`), WinUI text styles (`TitleTextBlockStyle`, …), Kirigami's `Heading` sizes and its small and fixed-width fonts. |
| `Selectable` | Text | The user can select and copy its text: AppKit `selectable`, GTK `selectable`, XAML `IsTextSelectionEnabled`. Sent in the `Create` only (the widget API takes it when it's built): Qt's selectable label is another item, `Kirigami.SelectableLabel`, which has no line limit or elision, so there `MaxLines` is kept. Report it back. |
| `MaxLines` | Text | At most this many lines, the last cut off with the platform's ellipsis; `None`: all. AppKit `maximumNumberOfLines` (the cell truncating its last visible line), GTK `lines` with `ellipsize` end (GTK only limits ellipsizing labels), Qt `maximumLineCount` with `elide` right, XAML `MaxLines` with `TextTrimming` `CharacterEllipsis`. Measure the label as limited. Report it back. |
| `TextColor` | Text, Icon | The label's or icon's colour; an icon in full colour keeps its own. Symbolic icons take it: AppKit `contentTintColor`, GTK's CSS `color` (the labels' classes, and a class with a display-wide rule for a fixed colour, since an image has no Pango attributes), `Kirigami.Icon`'s `color`, the `FontIcon`'s `Foreground`. A semantic one is the platform's own, so it follows dark mode, high contrast and the accent without the core sending it again: AppKit's catalogue colours; GTK's style classes (`dim-label`, `accent`, `error`, `warning`, `success`); `Kirigami.Theme` bindings; XAML `{ThemeResource}` brushes. `Rgba` is fixed. Report the colour sent where the shown one can't be told apart (a resolved brush, a class-less theme colour). |
| `FontWeight` | Text | The weight, in place of the text style's: Regular, Medium, Semibold, Bold (400 to 700), or the nearest the font has. Keep it when `TextStyle` changes, whichever comes first. Report it as the nearest of the four. |
| `Italic` | Text | Italics, kept when `TextStyle` changes. |
| `TextAlign` | Text | Left, center or right across the frame. The core has resolved the app's start and end against the text's direction, so don't mirror it: GTK flips `xalign` in a right-to-left widget, so an aligned label is set left-to-right. |

**Buttons**

| Prop | Applies to | Notes |
|---|---|---|
| `Label` | Button, ToggleButton, MenuButton, Checkbox, Switch, Select, RadioGroup, Slider, NumberInput, Progress, Image, Icon, GpuSurface | Only buttons and checkboxes show it; for the others it's the accessible name, and for a button that shows only its icon. |
| `ButtonRole` | Button | Normal, Default, Cancel or Destructive. Default: Return clicks it and it shows as the default (AppKit `keyEquivalent` `"\r"`, GTK `suggested-action`, Qt `Accessible.defaultButton`, WinUI `AccentButtonStyle`). Cancel: Escape clicks it (AppKit `keyEquivalent` Escape); the others show a normal button. Destructive: GTK `destructive-action`, AppKit `hasDestructiveAction`; Qt and WinUI have no such style. Keep it on the node: no toolkit tells every role apart. |
| `ButtonStyle` | Button, ToggleButton, MenuButton | Automatic, Bordered or Borderless. Automatic draws as Bordered. Borderless: AppKit `bordered` off, GTK `has-frame` off, Qt `flat`, WinUI `SubtleButtonStyle` (a toggle or menu button: a style of its own clearing the fill and border at rest, as Fluent has no subtle one). On WinUI one XAML style carries both role and style: Borderless wins over Default. Report back what was sent. |
| `Icon` | Icon, Button, ToggleButton, MenuButton | A name in the platform's own set: an SF Symbol, a themed icon's name, a Segoe Fluent Icons glyph. A button shows it before its caption, placed and sized as the platform places a button's icon; empty: none. A name the set lacks shows as the platform shows one (nothing, its missing-icon icon, the font's fallback). Measure the icon or button with it. Report it back, or keep it on the node where the platform can't (AppKit's symbol images have no name). |
| `IconSize` | Icon | In points: a symbol's point size on AppKit, the side of a square elsewhere (GTK `pixel_size`, `FontSize`, Kirigami's width and height). Otherwise the platform's size for an icon on its own. |
| `IconOnly` | Button, ToggleButton, MenuButton | Shows the icon without the caption, which stays the accessible name (AppKit `imagePosition` image only, GTK's icon button with the caption as its accessible label, Qt `display: IconOnly`, WinUI the glyph alone with the caption as its automation name). Without an icon, the caption shows. Report it as the button shows it, and as given while there's no icon. |
| `Menu` | MenuButton | Its menu, built as a context menu is (§8.1), and shown the platform's way when the button is clicked. Items report `MenuItem(id)`. Keep it apart from the node's `ContextMenu`, which a menu button can have too. Report it back as `ContextMenu` is. |

**Text fields**

| Prop | Applies to | Notes |
|---|---|---|
| `Value` | TextInput, PasswordInput, SearchInput, TextArea | Don't set again a value the widget already shows. Report a password field's text back too: the mirror check compares it, though the a11y tree never shows it. A text area's lines end in `\n`: XAML's text box ends them in `\r`, so read it back with `\n`. |
| `Placeholder` | TextInput, PasswordInput, SearchInput, TextArea | AppKit's and GTK's text views have none: keep it on the node and report it back. A search field shows the platform's own unless one is sent. |
| `ReadOnly` | TextInput, TextArea | Shows the text, still selectable, but not editable: AppKit `editable` off (`selectable` stays on), GTK and XAML `editable` / `IsReadOnly`, Qt `readOnly`. Keyboard focus is the platform's: AppKit's read-only fields take it from a click, and from Tab only with Full Keyboard Access. The app can still set `Value`. Report it as the field shows it. |
| `InputPurpose` | TextInput | Text, Email, Url or Phone, which the platform uses as it uses one (an on-screen keyboard, autofill, input methods). AppKit `contentType` (`NSTextContentType`), GTK `input-purpose`, XAML `InputScope` (one `InputScopeName`), Qt `inputMethodHints` (the dialable, email and URL characters' hints). It checks nothing typed. Report it back. |
| `Lines` | TextArea | How many lines of its text the area is tall at its natural size, in its own font and line spacing, inside its insets and frame. Measure it as that, a text field's width wide; it doesn't grow with its text, which scrolls. Keep it on the node where the platform has no such property. |
| `LineWrap` | TextArea | Every platform's text area wraps by default. Without, a line is as long as its text and the area scrolls sideways: AppKit's text view horizontally resizable in a container as wide as it likes (Apple's recipe), with a horizontal scroller, fitted to its text as it changes; GTK `wrap-mode` `None` with an automatic horizontal policy; XAML `TextWrapping.NoWrap` with an `Auto` horizontal scroll bar; Qt `wrapMode` `NoWrap` (the scroll view scrolls its content's width). Report it back. |
| `Enabled` | controls | Disabling a text area also clears its selection, collapsed to its start: a disabled one shows none. |

**Choices and values**

| Prop | Applies to | Notes |
|---|---|---|
| `Checked` | Checkbox, Switch, ToggleButton | A toggle button shows it pressed (AppKit's state on, GTK's and XAML's toggle buttons, Qt's `checked`). While a checkbox is mixed, keep it for when it isn't, and report it back. |
| `Mixed` | Checkbox | Shows the mixed state over `Checked` (AppKit `NSControlStateValueMixed`, GTK `inconsistent`, Qt `checkState` partly checked, WinUI `IsChecked` null). A user click leaves it, landing where the platform lands, and emits `Changed(Bool)`; the core absorbs that as `Mixed(false)`. Don't let later clicks cycle back into it: turn off AppKit's `allowsMixedState` and Qt's `tristate` after a click; clear GTK's `inconsistent` on a user toggle, as GTK apps do. Report it as the native control shows it. |
| `Options` | Select, RadioGroup | The options' texts, in order; texts may repeat. Replacing them keeps the chosen index if it's still an option, else chooses the first (none without options), as the core does: it sends `SelectedIndex` only when that changes it. Measure the select as the platform sizes it (widest or chosen option). On a radio group, a button for each, down a column; replacing them keeps the chosen index if it's still an option, else none is chosen. |
| `SelectedIndex` | Select, RadioGroup, Sidebar, Tabs | The chosen option. `None` only without options (a radio group: `None` for none, every button off). A sidebar's chosen item, counting across its sections; `None` for none. A tab view's page shown; `None` only without pages. |
| `Range { min, max }` | Slider, NumberInput | The core sends `Number` after it, since the platform may clamp the value to it. |
| `Number` | Slider, NumberInput | The value; a `NumberInput`'s is a whole number in an `i32`. |
| `Step` | Slider, NumberInput | Use it as the platform uses a step (tick marks, snapping, keyboard increments; what a spin box's buttons add). A platform with no stepped slider snaps the user's moves to it, as its apps do (GTK: in `change-value`, with a mark at each step that a tweak can turn off). `None`: the platform's default. Keep it on the node if the platform can't read it back. |
| `WrapAround` | NumberInput | Otherwise the platform's (AppKit's stepper wraps, the others stop). Stepped past one end, the other: `NSStepper.valueWraps`, GTK `wrap`, `NumberBox.IsWrapEnabled`, Qt `wrap`. Numbers typed past an end are still clamped. Report it back. |
| `Orientation` | Slider, Separator | A slider's: Horizontal or Vertical, sent only if the app chose one. Larger values are up: GTK runs vertical scales top down, so set `inverted` there, as GTK apps do. Measure as the platform does (AppKit's vertical sliders have no natural height, like its horizontal ones' width). A separator's: which way it runs, always sent. Measure it 0 (or the platform's minimum) along its length and as thick as the platform draws it across. Report it as the control shows it: from the widget where it has one (GTK), from the node otherwise. |
| `Progress` | Progress | From 0 to 1, or `None` for work of unknown length: the platform's indeterminate, animated bar. |
| `Running` | Spinner | Spins while true (`startAnimation`, `spinning`, `IsActive`, `running`). Stopped, it shows nothing but keeps its size. Report it back (AppKit can't read it: keep it on the node). |

**Images**

| Prop | Applies to | Notes |
|---|---|---|
| `Image` | Image | A file (the platform decodes it; unreadable: show nothing, measure zero) or `Pixels`: straight RGBA8, sRGB, at `scale` pixels to a point. Measure it at its size in points (`Pixels::size`), not its pixel count. Keep it on the node: no platform gives the pixels back. |
| `ImageFit` | Image | `Contain` or `Stretch`; otherwise the platform's default fit. |

**Containers, scroll views and lists**

| Prop | Applies to | Notes |
|---|---|---|
| `ScrollAxes` | ScrollView | Which directions scroll. |
| `ScrollBars` | ScrollView | Whether the scrolling axes show scroll bars, as the platform shows them (overlay or not). Without, it still scrolls by wheel, trackpad and touch: AppKit turns the scrollers off, GTK's policy is `External`, Qt's `AlwaysOff`, XAML's visibility `Hidden` (not `Disabled`, which stops scrolling). There is no "always shown": on macOS that's the user's setting. AppKit can't tell the axes from hidden scrollers, so it keeps them on the node. |
| `Rows` | List | The rows' keys, in order (§10). |
| `EstimatedRowHeight` | List | How high rows are likely to be, for platforms that size rows before showing them. |
| `Row` | Container | This container is the host of that row of its `List`. |
| `SelectionMode` | List | None, Single or Multiple. It can change while the list shows: keep what the platform keeps of the selection, never more than the new mode holds (none, or one row), and **report `Changed(Rows)`** if rows were let go. |
| `Selected` | List | The selected rows. |
| `ListStyle` | List | Automatic, Plain or Framed. Automatic draws as Plain. Report back what was sent: no toolkit tells Automatic from Plain. |
| `FileDrop` | Container, Group | Take files and folders dragged from the file manager: the platform's drop target on the host (AppKit `registerForDraggedTypes` with file URLs, a `gtk::DropTarget` for `gdk::FileList`, XAML's `AllowDrop` and drag events, a Qt Quick `DropArea`), showing a copy only while `FileDrop::accepted` keeps some of the dragged files, and reporting `DropHover` and `FilesDropped`. Only local paths. `None` removes it and ends any hover. Keep it on the node and report it. |

**Window chrome** (§11)

| Prop | Applies to | Notes |
|---|---|---|
| `Sections` | Sidebar | Its items (a title, an icon's name in the platform's own set), in sections with an optional title. Comes before `SelectedIndex`. Replacing them keeps the selected item if it's still there. Keep them on the node for `native_state`. |
| `SidebarShown` | Sidebar | Maybe before the sidebar is in its window. Shown or hidden the platform's way: AppKit collapses its split view item; WinUI closes the `NavigationView`'s pane (icons only in a wide window, as `Auto` shows it; hidden whole, with the menu button in the title bar, while an item has no icon); Qt takes the sidebar's page out of the page row, and puts it back ahead of the content's. libadwaita's split view always shows its sidebar while it isn't collapsed: collapsed, hidden is the content's page shown (`show-content`); wide, keep it for when it collapses. The content keeps its size, as when the sidebar comes and goes. Report what it shows, or before it's in a window, what the app asked. |
| `TabTitles` | Tabs | Its pages' titles, in page order: one per page once a batch is in (pages and titles may come in either order within it). Comes before `SelectedIndex`. |
| `TabIcons` | Tabs | Sent only if a tab has one: an icon's name per tab, in the platform's own set, empty for none. Show it where the platform's tabs do: a view stack page's `icon-name` (the inline switcher shows labels and icons), a `SelectorBarItem`'s `Icon` (a `FontIcon`), the tabs' actions' and buttons' `icon.name` on Qt. AppKit's `NSTabView` draws only titles (an item's `image` is for a tab view controller's toolbar style), and a `gtk::Notebook`'s tabs are text: keep them on the node there. Report them back. |
| `TabsStyle` | Tabs | How it shows its tabs, where the platform has more than one way: navigation tabs (`Automatic`, `Navigation`: libadwaita's inline view switcher, Kirigami's `NavigationTabBar`) or a tab bar (`TabBar`: a `gtk::Notebook`, a `QQC2.TabBar`). It can change while the tab view shows; keep the page shown and the focus. Other platforms have one way: keep it on the node and report it. If the strip's height changes with it, return the tab view's own insets from `tab_insets(id)`. |

**Any node**

| Prop | Applies to | Notes |
|---|---|---|
| `Tooltip` | any node | The platform's tooltip on the view the pointer rests on (`toolTip`, `set_tooltip_text`, `ToolTipService`, the attached `QQC2.ToolTip` on hover); empty removes it. Make it reach assistive technology as the description unless the node has its own (AppKit and GTK do that themselves). Report it for every node, `""` when none. |
| `ContextMenu` | any node | The platform's context menu, on the view the pointer rests on (as `Tooltip`), shown its own way: a right-click, a long press, the menu key (§8.1). Empty removes it. A view without one lets its container's show, as the platform does. A control whose native menu is taken (AppKit's pop-up button: its menu is its options) keeps it on the node. Report it once the core has sent one, read back where you can. |

**Escape hatches** (§9)

| Prop | Applies to | Notes |
|---|---|---|
| `Tweak` | built-in widgets | The app's raw settings, a payload of your own. Keep it on the node, report it back, and run it after the node's other props: on `Create` after all of them, and after every later `SetProp`, so what it sets wins. |
| `Custom` | Custom | The widget's props and definition. Native render: call its `update` when they differ. Drawn: just keep them for `native_state`. |
| `Drawing` | Custom (drawn) | The display list to rasterize. Redraw. |
| `Native` | Native | Your own payload type: on `Create`, the factory; later, apply its updates again. |

**GPU surfaces** (§12)

| Prop | Notes |
|---|---|
| `TakesInput` | It takes focus (a click, Tab) and reports its keys and pointer as `SurfaceInput`. Report it back. |
| `PointerLock` | Hide and hold the cursor, reporting its moves. Report what's in effect: `false` after the platform ended it. |
| `KeyboardGrab` | Focus the surface and give it every key, the system's shortcuts too as far as the platform lets an app. Report what's in effect. |
| `Cursor` | The cursor over it: the platform's own, none, or an image at its scale with its hotspot in points. Keep it on the node if the platform can't give it back. |

## 4. Events

Native callbacks **only** call `events.emit(id, event)` on the `EventSink` given to `init`. The `Ui` drains the queue at a safe time (`Ui::process_events`, and right after a `perform`), so never call `Ui` methods from a callback: you may be inside a `Ui` borrow.

**Programmatic sets never report.** GTK and WinUI change signals fire for them, so guard them: GTK mutes events while applying commands, WinUI compares with a `shown_*` value it keeps. Qt's user-only signals (`toggled`, `textEdited`, `activated`, `moved`) are used instead of its anyone's ones.

| Event | Emit when | Don't emit when |
|---|---|---|
| `Click` | a button is pressed (mouse, keyboard or accessibility) | |
| `Changed(Text)` | the user (or assistive technology) edits a text, password or search field or a text area, a search field's clear button included | the core set the value. **GTK `changed` (a text buffer's too) and WinUI `TextChanged` and `PasswordChanged` fire on programmatic sets.** Qt's `textEdited` is the user's only; a `TextArea` has no such signal, so the backend marks its own sets (`mitsuamiSetting`). AppKit's `textDidChange:` is the user's only. |
| `Changed(Bool)` | the user toggles a checkbox, switch or toggle button | the core set `Checked`. **GTK `toggled`/`notify::active` and WinUI `Checked`/`Unchecked`/`Toggled` fire on programmatic sets.** Qt's `toggled` is the user's only (`checkedChanged` is anyone's). |
| `Changed(Number)` | the user (or assistive technology) moves a slider, or steps a spin box or commits a number typed into it (Return, or leaving the field: not every keystroke), rounded to a whole number and kept in its range | the core set `Number` or `Range`. **GTK `value-changed` and WinUI `ValueChanged` fire on programmatic sets and clamps.** Qt's `moved` is the user's only. |
| `Changed(Index)` | the user (or assistive technology) chooses a select's option, a radio group's option, a sidebar's item, or a tab | the core set `SelectedIndex`, `Options` or `TabTitles`, or pages came or went; or the user clicked the radio button already chosen. **GTK `notify::selected`, `switch-page` and `toggled`, and WinUI `SelectionChanged` fire on programmatic sets.** **AppKit sends a radio button's action again on a click on the chosen one.** Qt's `activated` and `toggled` are the user's only (`currentIndexChanged` and `checkedChanged` are anyone's). |
| `Changed(Rows)` | the user (or assistive technology) changes a `List`'s selection, including rows deselected because they were removed or the `SelectionMode` can't hold them | the core set `Selected` |
| `Submit` | **Return/Enter** in a text or password field (GTK `activate`; WinUI `KeyDown` with `Enter`). Never from a text area, where Return starts a new line. | editing ends in other ways: Tab, a click elsewhere, focus loss. AppKit's field action does fire then; that was a real bug. |
| `Search(text)` | a search field asks for a search, with the platform's timing: its signal once typing pauses (AppKit's action, GTK `search-changed`, Kirigami `accepted`) or at once (WinUI `TextChanged`), on Return (AppKit's action, GTK `activate`, Kirigami `accepted`, WinUI `QuerySubmitted`), and when cleared. After `Changed(Text)` when both come | the core set the text. **GTK's `search-changed` and Kirigami's `accepted` fire for programmatic sets**, after their delay: report one only after a user edit, on Return, or on clearing. |
| `FocusIn` / `FocusOut` | keyboard focus moves, **from any source** (click, Tab, code): out for the old control first, then in for the new | |
| `Scrolled(offset)` | a `ScrollView`'s or `List`'s offset changes, by the user **or** by `ScrollTo` | |
| `RowShown(key)` | a `List` realises a row (it's in view, or about to be) | it already had |
| `RowHidden(key)` | a `List` lets go of a row it had shown | a reload shows it again right away: report only the difference |
| `RowActivated(key)` | a `List` row is double-clicked, or Enter is pressed on it | |
| `RowWidth(width)` | a `List` gives its rows a width other than its own (legacy scroll bars, insets, a frame): once it's known, and when it changes | |
| `WindowResized(size)` | the window's content area changes size (report the content size, without any menu bar or toolbar you placed in the window) | |
| `WindowCloseRequested` | the user asks to close a window. **Don't close it**: the app decides, and the core sends `Destroy`. | |
| `FullScreenChanged(on)` | the user puts a window in full screen or takes it out, the platform's way (AppKit's title bar button, the window manager's key), or the platform refuses the app's `FullScreen` | the core set `FullScreen`, even once the platform applies it later. **GTK's `notify::fullscreened`, Qt's `windowStateChanged` and WinUI's `AppWindow.Changed` fire for the app's own**: compare with what the app asked for. |
| `MaximizedChanged(on)` | the user maximizes a window or restores it, the platform's way (the title bar's button, a double-click on it, the window manager's key; AppKit's zoom), or a resize by the user undoes it | the core set `Maximized`, even once the platform applies it later. **GTK's `notify::maximized`, Qt's `windowStateChanged` and WinUI's `AppWindow.Changed` fire for the app's own**: compare with what the app asked for. |
| `SidebarShownChanged(shown)` | the user shows or hides a sidebar, the platform's way (AppKit: the divider dragged away, the toolbar's toggle; WinUI: the pane's button, a click beside a pane over the content; libadwaita, collapsed: an item chosen, the back button), or the platform does for the window's width (AppKit, WinUI's `Auto`) | the core set `SidebarShown`. AppKit's split view item's `collapsed` (KVO), WinUI's `PaneOpened` and `PaneClosed`, libadwaita's `notify::show-content` fire for the app's own: compare. |
| `MetricsChanged` | text scale, scale factor, theme or contrast changes, or a strip or heading measured once loaded differs from the metrics' | |
| `Remeasure` | a widget's natural size changed on its own, e.g. an image the platform decoded in the background | |
| `ContextMenuItem(id)` | the user (or assistive technology) chooses an item of the node's context menu | the core set `ContextMenu`. XAML and Qt toggle a check item themselves when it's clicked: put back the app's state before reporting it. |
| `MenuItem(id)` | the user (or assistive technology) chooses an item of a menu button's menu | the core set `Menu`; as `ContextMenuItem`. |
| `DropHover(bool)` | files the node takes are dragged over it (`true`), or leave it or are dropped (`false`) | the core set `FileDrop(Some)`. Report them in pairs: keep a hover flag per node. |
| `FilesDropped(paths)` | files are dropped on it: the ones `FileDrop::accepted` keeps, in the drag's order | none is kept: the drop is refused. |
| `Pointer(event)` | primary button down / up on a **drawn** custom widget, in its coordinates | |
| `Custom(value)` | a native render or native view emits (through your `Emitter`) | |
| `SurfaceReady(handle)` | a `GpuSurface`'s native surface exists (§12): once, before any `SurfaceResized` | |
| `SurfaceResized(size)` | a `GpuSurface`'s size in pixels or its scale changes; set it on the handle too (`set_size` says whether it changed) | its size is empty |
| `SurfaceInput(input)` | a key, a pointer move, the pointer leaving, a button or a scroll on a `GpuSurface` that takes input; while locked, the pointer's moves (§12) | it doesn't take input |
| `PointerLockEnded` | the platform ended a `GpuSurface`'s pointer lock (its window stopped being the active one, the compositor let go), or couldn't lock | the app turned it off, or the node went |
| `KeyboardGrabEnded` | the platform ended a `GpuSurface`'s keyboard grab (the surface or its window lost focus, the compositor let go), or couldn't grab | the app turned it off, or the node went |

**Focus tracking** needs one global observer, not per-widget guesses: AppKit uses KVO on `NSWindow.firstResponder`, GTK `notify::focus-widget` on the window, Qt Quick the window's `activeFocusItemChanged`, and WinUI a bubbling `GotFocus` on the window's root. WinUI raises it asynchronously, so its backend also reports the focus moves it makes itself right away, and drops the late event. Map the focused native object to the nearest known node by walking up its parents: composite widgets (a text field's inner editor, a scrolled window's viewport) put focus on children you didn't create.

## 5. Measuring

`measure(id, request) -> Size` is called **synchronously during layout**, after the current batch's structure and props have been applied. It's called for leaves; containers are never measured, except a `Tabs` and a `Group`: measured with nothing known and max-content space, they give their size empty (a tab view's strip of tabs and border, a group's insets and at least its heading's width), which the core keeps them at least as big as.

- `known_width` / `known_height`: already fixed. Measure the other axis given them, and return the known value unchanged.
- `available_width` / `available_height`: `Definite(w)` (wrap text to `w`), `MinContent` (the narrowest sensible width, e.g. the longest word) or `MaxContent` (no wrapping).
- Return logical units, rounded up (`ceil`), so text is never clipped by a fraction.
- Text inputs often have no intrinsic width; AppKit uses 200. Use something sensible and consistent.

Platform hints:

- **AppKit:** `fittingSize` / `intrinsicContentSize`, with `preferredMaxLayoutWidth` for wrapping text.
- **GTK:** `widget.measure(Orientation, for_size)` gives the minimum and natural sizes. Use natural for max-content and minimum for min-content.
- **Qt Quick:** `implicitWidth`/`implicitHeight`, right as soon as an item exists or its text changes, with no polish in between (a layout sizes itself when polished, so polish one first). Wrapping text: set `width` and read `implicitHeight`. Min-content: `Text.WordWrap` at width 1 leaves the longest word as `contentWidth` (`Text.Wrap` would break it). Restore the frame's width afterwards.
- **WinUI:** `element.Measure(available)` then `DesiredSize`. Elements must be in a live tree: before that, a `Button` measures `0 × 19`. The frame's `Width`/`Height` must be lifted to Auto (NaN) for the call, because `Measure` returns an explicit size. A control whose template loads later (a `ProgressRing`, an image decoded in the background) reports `Remeasure` once it has its size.

Known gap on AppKit and WinUI: min-content falls back to max-content. If your platform gives min-content cheaply (GTK and Qt do), implement it properly.

## 6. Metrics

`metrics()` returns:

- font sizes for each `TextStyle` from the platform's type ramp (body 13 pt on macOS, around 14–15 on WinUI and GNOME);
- the spacing tokens `xs…xl` in the platform's design language (AppKit: 4/6/8/12/20; pick yours from the GNOME HIG, Fluent or Kirigami's units);
- the scale factor, dark mode, high contrast and reduced motion;
- `tab_insets`: how far in from a `Tabs`' edges its page area is (the tab strip on top, the border elsewhere), as the platform's tab view lays out its pages. The core sizes pages with it; measure it from a real tab view once, if the platform doesn't say.
- `group_insets` and `titled_group_insets`: where a `Group` puts its content, without and with a heading: its border and the margins the platform gives content in a group, and the heading's room, inside the box or above it. The core adds them to the group's padding. Measure them from a probe, as for tabs.

Where one node's differ from the metrics', say so per node: `group_insets(id)` for a group whose box a tweak changed (its heading's place, its border), asked after measuring it; `tab_insets(id)` for a tab view whose strip isn't the metrics' (a `TabsStyle`). `None` keeps the metrics'.

Emit `MetricsChanged` when any of these change, including when a strip or heading you estimated before one loaded (WinUI's `SelectorBar`, a group's heading) measures differently.

## 7. Test hooks: perform, synthesize, native_state, capture

These make one test suite run against every backend.

### 7.1 `perform(id, action)`: do what assistive technology would

- `Activate`: press the button or toggle the control. Prefer the platform's accessibility press (AppKit `accessibilityPerformPress`; its return value lies for offscreen windows, so the result is ignored).
- `SetValue(text)`:
  - on a text field: set its text, then emit `Changed(Text)` yourself, because an assistive technology edit is a user edit; on a search field, then `Search(text)`, and drop the platform's own search that follows. WinUI's `PasswordBox` has no settable Value pattern: set `Password`.
  - on a select: choose the first option with that text, as picking it from the pop-up would, and report `Changed(Index)` (`Unsupported` if there's none). Don't open the pop-up: that starts a modal loop.
  - on a radio group: click its first button with that option, as the user does, which reports `Changed(Index)` unless it was chosen already; `Unsupported` if there's none.
  - on a slider: move it to that number, as a drag would. On a `NumberInput`: commit that number as if typed, rounded.
  - on a sidebar: select its first item with that title, as a click does, and report `Changed(Index)`; `Unsupported` if there's none.
  - on a tab view: show the page of its first tab with that title, as a click on the tab does, and report `Changed(Index)` if it wasn't shown; `Unsupported` if there's none.
- `Increment` / `Decrement` on a slider: step it as the platform's accessibility or keyboard does (VoiceOver's increment, a GTK step, UIA RangeValue by `SmallChange`, Qt's `increase()` and `moved`), which reports `Changed(Number)`. On a `NumberInput`, do what its buttons do (VoiceOver's increment on the stepper, GTK's `spin`, UIA RangeValue by `SmallChange`, Qt's `increase()` and `valueModified`).
- `Focus`: move keyboard focus to the control. On a sidebar, to its list (its selected item, where items take focus); on a radio group, its chosen button, or its first, as Tab does; on a tab view, its tab strip (its selected tab).
- `Select` on a `List`'s row host: select that row (the only selected one), as a screen reader's select does, and report `Changed(Rows)` on the `List`. `Unsupported` if the list's `SelectionMode` is None. `Activate` on a row host: report `RowActivated` on the `List`.
- `ContextMenuItem(id)`: choose that item of the node's context menu, as a screen reader does once it has shown the menu, through the item's own path (AppKit's `performActionForItemAtIndex:`), which reports `ContextMenuItem(id)`. Never open the menu. `Disabled` for a disabled item or control (disabled controls show no menu), `Unsupported` if there's no such item. The test kit finds the node the way a right-click would: the nearest one up the tree with a menu.
- `MenuItem(id)` on a menu button: choose that item of its menu as `ContextMenuItem` does, reporting `MenuItem(id)`, without opening the menu. `Activate` on it is `Unsupported`: pressing it opens the menu, which is modal.
- `ScrollIntoView`: the core does it (it knows where everything is), so accept it and do nothing.
- Return `ActionError::Disabled` for disabled controls, `ReadOnly` for `SetValue` on a read-only field, `Unsupported` for actions that don't apply, and `UnknownNode` for a node you don't have.

### 7.2 `synthesize(id, input)`: behave as close to real input as the platform allows

- `Key(Char | Backspace | Enter | Tab)` on text, password and search fields and text areas must go through the platform's text-editing path, so the real signals fire. AppKit drives the field editor (`insertText:`, `doCommandBySelector:`), a secure one for password fields, and a text area's text view itself; Qt sends real key events; GTK 4 can't inject keys, so it emits the keybinding signals keys are bound to (`insert-at-cursor`, `backspace`, `activate`, `move-focus`).
  - If the field wasn't focused, focus it and **put the caret at the end**: focusing selects all, and the first keystroke would replace everything.
  - Any key on a read-only field: `ActionError::ReadOnly`, before focusing it. Nothing can be typed into one anywhere, and AppKit's can't take keyboard focus, so no platform delivers the keys.
  - Enter in a search field searches (WinUI's backend reports it: only a real key raises `QuerySubmitted`). In a text area, Enter is a new line everywhere, and Tab is the platform's: AppKit, GTK (while the view `accepts_tab`) and Qt insert a tab, WinUI moves focus on.
  - WinUI's `PasswordBox` has no caret or selection to edit through, so its backend edits `Password` at the end, where typing into a focused box goes, and `PasswordChanged` reports it.
- `Key(Escape)` takes the platform's own path: a `Cancel` button's key equivalent on AppKit, then a modal window's close request (§8.2). Where keys can't be injected (GTK) or controls are driven directly (WinUI), run the window's own Escape shortcut or accelerator.
- Enter or Space on buttons, Space on toggles.
- `Scroll { dx, dy }` scrolls a `ScrollView` or `List` as a scroll wheel would, clamped.
- `Key(Up | Down | Home | End | Enter)` on a `List` goes through the list's own key handling: move the selection and scroll to it, or activate the selected row.
- `Click(point)` on **drawn** custom widgets: a real down/up pair through your drawn view's event handlers. `Unsupported` elsewhere: native controls often track the mouse in a modal loop.
- `DragFiles(paths)`, `DragLeave` and `DropFiles(paths)` on a host with a `FileDrop`: run the functions your drag handlers call, with the paths a real drag's data would give them (a drop enters first, as a real one does). `Unsupported` on a node without one. Nothing here can drag from a file manager, so reading the paths from a real drag is only tried by hand (`examples/file_drop.rs`).
- On a `GpuSurface` that takes input: `Click` focuses it and reports the primary button down and up there, `Key` the key down and up, `Scroll` one scroll in points (§12).

### 7.3 `native_state(id)`: read back what the widget shows

**Read back from the widget** what it actually shows: its props (text, title, value, placeholder, checked, enabled, and every other prop the core has), its frame, its parent and children (in native order), whether it's focused, and its scroll offset. Only keep on the node what the platform can't report. After every settle, the test kit compares this with the core and fails on any difference: the mirror check. It has caught every serious backend bug so far.

- A window's children include its toolbar items, after its content, then its sidebar.
- A `ToolbarItem` reports the rect the toolbar gave it, in the coordinates of the window's content (above it, so at a negative y), and `Rect::ZERO` while it's hidden, whether it's empty or the toolbar put it in an overflow menu.
- A `Sidebar` reports the rect its pane has in the content's coordinates (beside it, so at negative x), `Rect::ZERO` while it isn't shown (collapsed).
- A `Tabs` page reports where the tab view put it, at the size the core gave it, and `Rect::ZERO` while another page is shown.
- A `List` row host reports the rect the platform gave that row, in the list's content (§10).

### 7.4 `capture(id, reply)`: an offscreen screenshot

RGBA8 at backing scale, rows top to bottom. Reply when the image is ready, right away if possible, or with a `CaptureError` (`UnknownNode`, `Unsupported`, `Failed`).

- AppKit: `cacheDisplayInRect:toBitmapImageRep:`, which replies at once, after laying the window out (`layoutSubtreeIfNeeded`). The test window is never key, so captures show the unfocused-window look (grey default buttons).
- GTK: `gtk::WidgetPaintable`, a snapshot and `render_texture` (the Cairo renderer, so captures don't depend on the GPU), then download. Replies from the frame clock's `after-paint`, once the widget is mapped and laid out.
- WinUI: `RenderTargetBitmap.RenderAsync`, then `GetPixelsAsync`, replying from the completion.
- Qt Quick: `QQuickWindow::grabWindow`, which renders right away (in software on the offscreen platform), cropped to the node.

### 7.5 `TestHooks`

- **`name`**: a short name for snapshot and baseline paths (`"appkit"`, `"gtk"`, `"winui"`, `"kirigami"`).
- **`resize_window(window, size)`** resizes a window's content the way the user would, so the platform reports `WindowResized`: no smaller than its `MinSize`, as a drag goes, even where the platform's call alone (AppKit's `setContentSize:`) would, and not at all if it isn't `Resizable`.
- **`close_window(window)`** clicks a window's close button the way the user would, through the platform (`performClose:`, `gtk::Window::close`, `QQuickWindow::close`, a posted `WM_CLOSE` on WinUI: XAML's `Window.Close` skips `Closing`), so the platform reports `WindowCloseRequested` itself. Never close the window: whether it closes is the app's call, and the core destroys it if so.
- **`take_command_log()`** returns the commands applied since the last call (record them when the test kit asks you to), for command snapshots; **`node_count()`** the live native nodes, as a leak detector.
- **`app_info(window)`** reads back what the window shows of the app's id, name and icon (§13), as a `NativeAppInfo`: `None` for what the platform has no place for, the icon as an image's size or a theme name.
- **`settle()`** runs after every settle and while a test awaits something the platform completes (a capture). Use it to let the platform catch up without blocking, and report what that brings. Every backend has work there: AppKit lays out tables and toolbars, which make their views in a layout pass offscreen windows never get, and runs its run loop while a search field waits on a timer; GTK presents windows and dispatches what its main context has ready (allocations, adjustments, focus), and allocates lists, header bars and surfaces itself, since frames stall on its test display; Kirigami polishes windows, so Qt has placed what it lays out; WinUI pumps messages, picks up focus moves XAML made itself, and waits for templates and images to load.

## 8. Services

Implement `Services`. **Never block**: reply later, from the platform's completion callback.

| Service | AppKit | GTK 4 | WinUI 3 | Kirigami |
|---|---|---|---|---|
| clipboard read (async reply) | `NSPasteboard` (replies at once) | `gdk::Clipboard::read_text_async` | `Clipboard.GetContent().GetTextAsync()` | `QClipboard` (replies at once) |
| clipboard write (async reply, can fail) | `NSPasteboard` (replies at once) | `gdk::Clipboard::set_text` | `Clipboard.SetContent` (throws while another process holds the clipboard: reply `Err`) | `QClipboard` (replies at once) |
| alert | `NSAlert` sheet on the parent | `gtk::AlertDialog::choose` | `ContentDialog` (one at a time per window) | `Kirigami.PromptDialog` in the window's overlay |
| open / save | `NSOpenPanel` / `NSSavePanel` sheets with `UTType` filters | `gtk::FileDialog` (`open`/`open_multiple`/`save`) with `gtk::FileFilter` | the Windows App SDK's pickers (`Microsoft.Windows.Storage.Pickers`: `FileOpenPicker`, `FolderPicker`, `FileSavePicker`), created with the window's `WindowId` | Qt Quick's `FileDialog` / `FolderDialog` (Plasma's own through its platform theme) |
| menus | the global `NSMenu` bar: the app menu, the app's File, Edit, the rest; a window's own menus while it's main | the header bar's primary menu (a `gio::Menu` section per menu) in each window | a `MenuBar` in each window | a `Kirigami.GlobalDrawer` shown as a menu (`isMenu`) in each window |
| submenus | `NSMenuItem.submenu` | `gio::Menu::append_submenu` | `MenuFlyoutSubItem` | nested `Kirigami.Action`s |
| check / radio items | `NSMenuItem.state` | a stateful action (boolean; a radio item's holds its id, its target) | `ToggleMenuFlyoutItem` / `RadioMenuFlyoutItem` (`GroupName`) | `checkable` actions; a radio group in one `QQC2.ActionGroup` |
| roles (About, Settings, Quit) | the app menu, AppKit's titles and shortcuts | the last section: Settings, About, Quit | where the app put them | the end of the drawer: Settings, About, Quit |

- **`parent: None`** means the focused window: AppKit uses the key window, then the main window. Only fall back to app-modal if there is no window. Qt's `active` is true for a focused window's transient parents too, and so for their other dialogs; Kirigami tracks `QGuiApplication::focusWindow()` instead, and WinUI `GetActiveWindow`.
- **A file dialog's `start_folder`** is where it opens: take `services::existing_folder(&request.start_folder)`, which is `None` for a folder that isn't there, and leave the choice to the platform then. **A `FileFilter` with no extensions** (`FileFilter::all`, `is_all`) lets every file through: a filter of its own where the platform offers a choice of filters, and no restriction at all where it only has one list of allowed types (AppKit). `OpenFile::directories` opens folders.

### 8.1 Menus

- **`set_menu(None, …)` is the app's menus; `set_menu(Some(window), …)` is that window's own,** shown with the app's (`MenuBarData::merged`), and an empty bar removes them. Menus in each window show `MenuBarData::for_window`: a modal window (a dialog, `Prop::Modal` in its `Create`) shows only its own, without your Quit. A window's menus usually arrive before its `Create` is applied, and may arrive after it's destroyed: keep them by `NodeId`.
- **Menus inside the window** (GTK without a global menu, WinUI): the menu bar takes space the core doesn't know about. Put it above your content host, and report the **remaining** content size in `WindowResized`.
- **Keep the platform's standard menus** (Quit, Edit with Cut, Copy, Paste and Undo) and leave their enabling to the platform. The app's own items follow its `enabled` state.
- **Update in place when you can:** if `MenuBarData::same_structure` holds, only enabled and checked states changed; rebuilding would close an open menu.
- **Items with a role** (`MenuRole::{About, Settings, Quit}`) go where the platform puts them: `MenuBarData::take_role` takes them out of the app's menus, tidying separators. A Quit item replaces your own Quit. Where the platform has no place for them, leave them.
- **Check and radio items** are drawn by the platform, radio groups named by `MenuData::radio_groups`. If it toggles an item itself on a click, put the app's state back: the core sends the new state when the app changes it, and only the user's choice may call `activate`.
- `Shortcut::primary` is Command on macOS and Ctrl on GTK, Qt and WinUI.
- **Context menus and menu buttons** (`Prop::ContextMenu`, `Prop::Menu`) are built by the same code: ids, check marks, radio groups, submenus, separators, shortcuts shown (only the menu bar's work from the keyboard); roles mean nothing there.

### 8.2 Windows closing and the app quitting

- **Escape asks a modal window to close** (`WindowCloseRequested`), through the platform's own path, so a focused control that uses Escape gets it first: AppKit's `cancelOperation:` up the responder chain to the window's delegate, GTK's bubble-phase `ShortcutController`, Qt's `Shortcut` on `StandardKey.Cancel`, WinUI's `KeyboardAccelerator`. Plain windows ignore it.
- **The platform's own quit** (macOS's `terminate:` from the Dock, the app menu's default Quit or logging out; the session ending elsewhere) goes to the app: call `ui.request_quit()` (the app's Quit item, or a close request to every window), tick, and if windows are left, refuse it the platform's way. Never let it end the process while the app has windows open: it may have work to lose. ARCHITECTURE.md §14.7 has each platform's route.

## 9. Escape hatches: custom widgets, native views and tweaks

The core does the shared work; a backend supplies four things. Each backend's are in its `src/custom.rs` and `src/tweak.rs`. If your controls may size themselves or skip change events for some sources, see ARCHITECTURE.md §15, WinUI.

1. **A `NativeRender` trait** for custom widgets, in your crate, shaped like AppKit's: `type View`, `create(props, cx)`, `update(view, old, new)`, and optional `measure`, `read` (read the props back from the widget, for the mirror check) and `perform`. Provide `native::<W>() -> Renderer<W>` for the platform's own controls and `ad_hoc::<W>()` for renders built from the platform's widgets the way its apps build them: wrap a type-erased render in an `Opaque` and pass it to `Renderer::native` (or `Renderer::ad_hoc`). On `Create` of `Custom(_)`, `find_prop!(props, Custom)`: if `custom.native()` is `Some`, downcast it to your render and create the view; otherwise the widget is drawn.
2. **A drawn view** that rasterizes `DisplayList`s (fill and stroke of rects, rounded rects, ellipses and paths) with the platform's 2D API. Resolve the semantic `Color`s at draw time, so they follow the appearance. Report primary-button `Pointer` down/up in the widget's coordinates, and support `SyntheticInput::Click`. Don't measure drawn widgets (the core does); `native_state` reports `Custom` and `Drawing` as last received.
3. **`NativeView::<platform>(factory)`** for app-supplied widgets, with a payload type of your own in `Prop::Native`. Run the factory on `Create`, and apply the updates on `Create` and on each `SetProp`.
4. **`tweak` and `tweak_with`** for raw settings of built-in widgets: `tweak(|b: &NSButton| …)` returns a `Tweak<W>` (the core's), typed through a `Tweakable` trait that names your control for each widget. Pass `Tweak::new` the value and a function that makes the `Opaque` (a closure over the node's view) from it; `tweak` is `tweak_with` of a static `()`. The backend runs it as `Prop::Tweak` says (§3.2). ARCHITECTURE.md §7.4 has the table of native types; a widget whose node stands for a scroll view (`List`, `TextArea`) runs its tweak on the view inside.

Also:

- **A context for factories** (`AppKitCx`, `GtkCx`, `WinUiCx`, `KirigamiCx`): the main-thread marker where there is one, an `Emitter` that queues `UiEvent::Custom(AnyValue::new(event))` for the node, and a way to hear a control's actions or signals whose targets live as long as the node.
- **`perform` on custom widgets:** call the render's `perform`. Return `Unsupported` when it doesn't handle an action; the core then emits the event the widget's shared definition maps it to. **On native views:** perform on the accessibility element the way the screen reader would. That can be a child of the view (AppKit: an `NSStepper`'s cell).
- **Put the platform's bindings on your crate's public API** (AppKit re-exports `objc2`, `objc2_app_kit`, `objc2_foundation`; GTK `gtk`; WinUI `bindings` and `windows_core`), so apps use the same versions.

## 10. Lists

A `List` is the platform's list control, and the platform virtualises it: it scrolls, decides which rows to realise, recycles them, and draws and handles the selection. The core builds what's in the rows.

- **The data is `Prop::Rows`:** the rows' keys, in order. When `Rows` changes, the selection must follow the rows, not their indexes: diff the keys into native inserts, removes and moves, or reload and select the rows that stayed selected (AppKit reloads). Report `Changed(Rows)` if selected rows went.
- **Report the rows you realise:** `RowShown(key)` when the platform prepares a row (a cell asked for, a delegate created, a container realised), `RowHidden(key)` when it recycles it. The core mounts each shown row as a `Container` with `Prop::Row(key)`, inserted as the `List`'s native child, and disposes it when hidden. The `List`'s native children are those hosts, in row order. A reload that shows the same rows again must not hide and show them: report only the difference, or their state goes.
- **Cells:** give a realised row an empty cell until its host arrives (`Insert`), then put the host in it. A host's `SetFrame` is its size, at origin 0; make the row that high, and place it where the platform places rows. Never let a cell be 0 high while it waits: some platforms then realise every row, or shift the scroll offset.
- **Heights:** rows the platform hasn't shown yet need a height from somewhere if it sizes rows up front (AppKit): `EstimatedRowHeight` if the app gave one, or else the first row measured. **Keep the heights of rows you've measured** when you let them go, and keep the estimate steady: guesses that change as rows come and go move rows in and out of view, and the rows shown flip back and forth.
- **Row width:** the core lays rows out at the list's width. If rows get another width (legacy scroll bars, a scroll bar's own column as on Breeze, a `Framed` list's border, insets), report it with `RowWidth`.
- **Platforms keep rows of their own.** Which rows the platform realises is its call, and the tests allow for it: AppKit prepares a few around the view, GTK about 200, and GTK keeps its cursor row and selected rows bound wherever it scrolls. Where rows go when rows are inserted above the view is its call too (GTK keeps the rows in view where they were).
- **Build rows before drawing.** Report rows as soon as the platform decides them, ideally inside the `apply` or scroll that changed them, so the core builds them in the same run-loop turn: platforms that realise rows in a layout pass (AppKit) run it at the end of `apply` and in `settle`.
- **Callbacks come at any time.** A table can ask for cells, heights and counts in the middle of your own `apply` (a reload, a scroll, a resize). Keep the list's data (keys, heights, hosts, cells) in a small `Rc<RefCell<…>>` of its own that the data source reads, never your backend's main state or the `Ui`, and only `emit` from there.
- **`native_state` of a row host** reports the rect the platform gave that row, in the list's content; the core uses its position (that's where frames inside rows, visibility and `scroll_into_view` come from), and the mirror check compares its size with the host's. The `List` reports its `Rows`, `SelectionMode` and `Selected` as the native control shows them, and its `ListStyle` as last set.
- **Focus:** the `List` itself takes focus (it's in the Tab order), as the native control does.

ARCHITECTURE.md §13.22 has each platform's list.

## 11. Window chrome: toolbars, sidebars and tab views

### 11.1 Toolbar items

A `ToolbarItem` is a host the core lays out on its own, at its natural size, and inserts as the window's native child after its content. It goes in the window's toolbar, at the trailing end; its index among the items is `index` less the content's children. Its `SetFrame` is only its size: the toolbar places it and spaces the items. Hide it while its size is empty (a `Show` that shows nothing), and report it at `Rect::ZERO` while it's hidden or in the toolbar's overflow menu. The content keeps its size: `SetWindowSize` and `WindowResized` are the content's, and the window grows by the bar.

### 11.2 Sidebars

A `Sidebar` is the list down a window's leading side that picks what the window shows (System Settings, GNOME Settings, Windows Settings). It's window chrome, like the toolbar: a native child of the window, after its content and toolbar items, at most one per window, that the core never lays out or measures. Its items are data (`Prop::Sections`); only the platform draws them.

- **The window's content goes beside it.** Put the content host in the split's other pane (AppKit's split view item, libadwaita's content page, the `NavigationView`'s `Content`, the content's page in Kirigami's page row); take it back when the sidebar is removed. Toolbar items still go before it (a toolbar declared after the sidebar is inserted before it).
- **The content keeps its size:** `SetWindowSize` and `WindowResized` are the content's, and the window is larger by the sidebar (and any title bar the content moves under), as with a toolbar. Report the content's size when the user moves the divider or the platform collapses the sidebar.
- **Collapse it the platform's way** in a narrow window: none of it is the core's.
- **Report the user's choice only** as `Changed(Index)`; the app's `SelectedIndex` never. A sidebar keeps its selection where the platform lets the user take it away (a click on empty space, Ctrl+click).
- **It's in the Tab order,** first: the core sends it in `SetFocusOrder`. Report its focus as the sidebar's.

### 11.3 Tab views

A `Tabs` is the platform's tab view: pages, one shown at a time, and a strip of tabs to pick one (a settings window's panes, not documents). Its native children are page hosts (`Container`s), each a page, titled by `Prop::TabTitles` at the same index; `Prop::SelectedIndex` is the page shown.

- **Every page stays alive.** Hide the others the platform's way (`NSTabView` takes their views out, a notebook unmaps them); never destroy a host until `Destroy`.
- **The core sizes the pages, the platform places them.** Each host gets its size from `SetFrame`; put it at the top-left of the page area (inside the strip and border), wrapped in a view of your own if the platform sizes its pages itself. `metrics().tab_insets` says where that area is, so the core's sizes fit it; the `Tabs`' own `measure` is its strip and border (§5).
- **Report the user's choice only** as `Changed(Index)`: a click on a tab, the arrow keys, a mnemonic. Changing the page from `SelectedIndex`, or when pages come and go, is the app's.
- **It's in the Tab order,** before its page's controls, which follow it; the hidden pages' controls aren't in the order the core sends. Report its focus as the tab view's.

## 12. GPU surfaces

A `GpuSurface` is a native surface the app presents to with its own GPU API, from its own thread. The backend makes the native surface, places it over its widget and reports its size; it never draws in it.

- **Hand it out as a `SurfaceHandle`:** implement `NativeSurface` (the `raw-window-handle` window and display handles) for your native surface and wrap it with `SurfaceHandle::new`, then report `SurfaceReady(handle)`. Make it when the platform can: AppKit on `Create`, Wayland once the window has a surface (when the widget is mapped), WinUI once the node is in a window. Where there's no surface to make (a test display that's neither Wayland nor X11), hand out one without window handles, as headless does, and end any lock or grab at once.
- **The handle owns the surface:** it lives until the app's last handle is dropped, after the node is destroyed too, since a GPU surface made on it must not outlive it. `Destroy` only stops it showing and reporting, and must keep the window from taking it down (WinUI moves its child window under `HWND_MESSAGE`, X11 under the root window). The last handle may be dropped on any thread: free what must be freed on the UI thread there (AppKit's main queue, WinUI's `WM_CLOSE`). Don't let the widget hold a handle to itself past its node, or it never goes.
- **Follow the widget:** where the surface is a window of its own (a subsurface, a child window), place it after each of the window's frames, so layout, scrolling and resizes all move it, and hide it while the widget has no size or isn't shown. It takes no input (an empty input region or shape, `HTTRANSPARENT`), so the pointer goes to the widget under it. Where the pointer can't pass through to the toolkit (WinUI's content island never gets it), the surface's window reports the pointer itself while it takes input.
- **Report its size in pixels and its scale** as `SurfaceResized`, and set it on the handle first (`SurfaceHandle::set_size`), so a render thread that reads it sees it as soon as the app does. Report it when the frame changes, not at the window's next frame: a settle doesn't wait for one, and the app would draw its next frame at the old size.
- **Measure it as nothing:** it's as large as the layout makes it. `native_state` reports its `Label`, `TakesInput`, `PointerLock`, `KeyboardGrab` and `Cursor`.
- **Input (`TakesInput`) comes through the widget under the surface,** the toolkit's own events: a click focuses it, and it's in the Tab order. Report keys by where they are on the keyboard (`KeyCode::from_mac`, `from_evdev`, `from_windows_scancode`, with the platform's code as `native`; on Linux, keys that type no character by their keysym, `from_keysym`, so the keymap's remaps apply), down and up, `repeat` for the platform's own repeats, and each key's release only after its press. It gets every key the window doesn't take first as a shortcut of its menus, in the platform's own order, and Tab too; Control+Tab is left to the toolkit where it moves focus out of a view that takes Tab. Report the pointer in points from its top left, when it leaves, every button, and scrolling towards the end (down, right) as positive, in lines for a wheel's notches and points for a trackpad. Keys it has down when it loses focus, or its window stops being the active one, are reported released (`pressed: false`) right then: their releases would go elsewhere, and the app would think them held.
- **The cursor (`Cursor`)** shows while the pointer is over the widget and not locked: the platform's arrow for `Default`, a blank one for `Hidden` (not the platform's "hide the cursor", which is global and counted), an image for `Image`. Set it on the widget under the surface, whose pointer it is.
- **The pointer lock** hides the cursor, holds it, and reports how far the mouse moved as `SurfaceInput::Motion`, in points, accelerated as the cursor would be (no `PointerMoved` meanwhile), and after it, where the platform has it, as `SurfaceInput::RawMotion`, in the device's counts before the host's acceleration (AppKit: `GCMouse`; Wayland: the relative pointer's unaccelerated values; X11: XInput 2 raw motion; Windows: Raw Input). Only in the active window: if it isn't, report `PointerLockEnded` at once. A lock asked for before the node is in a window waits for one. End it (`PointerLockEnded`) when the window stops being the active one or the platform lets go; the app turning it off, hiding or destroying the node release it silently.
- **The keyboard grab** focuses the surface, then gives it every key, the window's shortcuts and as many of the system's as the platform lets an app take (AppKit: Command-Tab with the app's presentation options; GTK: `inhibit_system_shortcuts`; Qt: the Wayland shortcuts inhibitor or an X11 keyboard grab; Windows: a low-level keyboard hook). It ends (`KeyboardGrabEnded`) when the surface or its window loses focus, or the platform lets go.
- **Synthesized input** (`synthesize` on a surface that takes input): `Click` focuses it and reports the primary button down and up there, `Key` the key down and up (the surface has focus), `Scroll` one scroll in points. Through the platform's own event methods where it has them.

ARCHITECTURE.md §13.26 has each platform's route.

## 13. The app's id, name and icon

`Backend::set_app_info(&AppInfo)` comes before the app's first window (`run` calls `Ui::set_app_info` right after `Ui::new`), and perhaps again later (tests). Take what the platform has a place for, for the windows there are and those to come; a packaged app's own (a bundle's, an MSIX package's) wins.

| | AppKit | GTK 4 | WinUI 3 | Kirigami |
|---|---|---|---|---|
| id | the bundle's; ignored | `glib::set_prgname` (Wayland app id, X11 class), also in `run` before `gtk::init` | `SetCurrentProcessExplicitAppUserModelID`, unless packaged | `QGuiApplication::setDesktopFileName` |
| name | the app menu's About, Hide and Quit | `glib::set_application_name` | ignored (the executable's or its shortcut's) | `setApplicationDisplayName` (Qt adds it to window titles) |
| icon | `applicationIconImage`, unless the bundle has an icon | the theme's icon named after the id (`set_default_icon_name`); the image is ignored | `AppWindow.SetIcon` on each window: an `.ico` file's path, or else an `HICON` from the PNG | `setWindowIcon(QIcon::fromTheme(id, image))` |

## 14. Tab order

`SetFocusOrder` gives the window-wide order. Platforms differ in how to impose it:

- **AppKit:** turn off `autorecalculatesKeyViewLoop` and link `nextKeyView` into a loop.
- **GTK 4:** there is no "next widget" pointer. The window's content host overrides the `focus` vfunc: for Tab and Shift+Tab it `grab_focus`es the next widget in the order that accepts focus, wrapping around; other directions keep GTK's behaviour.
- **WinUI:** `TabIndex` is scoped to each container, so it can't express a window-wide order across nested hosts. Handle Tab in `PreviewKeyDown` on the window's root and focus the next control in the order yourself, as on GTK.
- **Qt Quick:** its focus chain follows item order within each parent. An event filter on the window handles Tab and Shift+Tab along the core's order, skipping disabled and hidden items; popups (dialogs, menus) keep Qt's own chain. Qt Quick focuses nothing in a new window, where AppKit and GTK focus the first control, so the first order a window gets focuses its first control that can take focus, if nothing has it.

The conformance tests use three controls arranged so that reading order and position on screen disagree (right-to-left rows, absolute positioning, `tab_index`). An order based on position fails them, as it should.

## 15. The run loop

`run(info, setup)` owns the platform's main loop (see `mitsuami-appkit/src/app.rs`):

1. Create the platform app, your backend and `Ui::new(backend)`. Give it the app's id, name and icon with `ui.set_app_info(info)` (§13), then install the standard menus with `ui.set_menu(MenuBar::new())`.
2. `ui.set_commit_scheduler(wake)`: called when something changed; make the loop turn soon.
3. `ui.set_waker(Arc<dyn Fn() + Send + Sync>)`: a **thread-safe** wake-up, called when background work completes a task.
4. `setup(&ui)` (the app creates its windows), then `ui.tick()`, **then** show the windows (each backend's `show_pending_windows`), so nobody sees an unlaid-out frame.
5. Call `ui.tick()` whenever the loop is about to sleep, and again after each wake-up. After each tick, arm **one** timer for `ui.time_to_next_timer()`.
6. Stop when `ui.windows()` is empty.
7. Hand the platform's own quit to the app (§8.2).

| Step | AppKit | GTK 4 | WinUI 3 | Kirigami |
|---|---|---|---|---|
| tick before sleeping | `CFRunLoopObserver` (BeforeWaiting, common modes) | an idle source at `HIGH_IDLE` (`glib::idle_add_local_full`), ahead of GTK's layout and drawing, guarded by a "scheduled" flag | our own `PeekMessage` loop (no `Application::Start`) ticks before it sleeps; ticks are also scheduled with `DispatcherQueue.TryEnqueue`, which runs inside modal loops (live resizing) | the event dispatcher's `aboutToBlock` |
| thread-safe wake | `CFRunLoopWakeUp` | `glib::MainContext::default().invoke(...)` to schedule the tick | `PostThreadMessageW(WM_NULL)` to the UI thread | `QAbstractEventDispatcher::wakeUp` |
| timer | one `CFRunLoopTimer`, armed again | a `glib::timeout_add_local_once` replaced when armed again | the timeout of `MsgWaitForMultipleObjectsEx` | one single-shot `QTimer`, armed again |
| stop | `stop:` **plus an empty posted event** (otherwise it waits for the next real event) | `glib::MainLoop::quit` (there's no `gtk::Application`) | leave the loop, then release XAML objects while XAML still runs | `QCoreApplication::quit` (`quitOnLastWindowClosed` off: the core decides) |

Never call `tick()` from inside a widget callback; the loop calls it.

## 16. Sync and async in the contract

The contract is async wherever **any** platform might complete the work later. Reply-based methods take a `Reply<T>` (a `FnOnce(T)`). Call it exactly once, from a completion handler if needed, or right away when the platform is synchronous.

| Async (reply) | Why |
|---|---|
| `capture` | WinUI renders to bitmaps asynchronously |
| `clipboard_text`, `set_clipboard_text` | GTK and WinUI clipboards are async; writes can fail |
| `alert`, `open_file`, `save_file` | the user answers later |

| Sync | Why it can stay sync |
|---|---|
| `measure`, `group_insets`, `tab_insets` | layout needs the answer now; every platform measures synchronously |
| `native_state`, `metrics` | plain property reads |
| `init`, `apply`, `set_menu`, `set_app_info`, `services` | instructions with no answer |
| `perform`, `synthesize` | the `Result` only says whether the action was accepted; its effects arrive later as events |

If your platform can only do one of the sync ones asynchronously, raise it: we change the contract rather than block the UI thread.

## 17. Testing your backend

```sh
MITSUAMI_NATIVE=1 cargo test --workspace                     # every suite on your backend
MITSUAMI_NATIVE=1 cargo test -p mitsuami --test conformance  # the contract
MITSUAMI_SHOW_WINDOWS=1 MITSUAMI_NATIVE=1 cargo test         # watch it
cargo run -p mitsuami --example run_loop_smoke               # timers, background wake-up, exit (must exit by itself)
cargo run --manifest-path examples/showcase/Cargo.toml       # look at it: every example
```

- **`tests/conformance.rs` is the contract.** Get it green first; the other suites mostly follow.
- **Platforms that report asynchronously** do their catching up in `TestHooks::settle` (§7.5): tests call it while settling, and while a test awaits native work (a capture). Report what your backend causes itself right away, rather than waiting for the platform's event, so settles stay deterministic.
- **On Kirigami**, the backend and the test kit need their features: `MITSUAMI_NATIVE=1 cargo test -p mitsuami -p mitsuami-kirigami --features mitsuami/kde,mitsuami-test/kde,mitsuami-kirigami/qt`.
- **On Windows**, build with the MSVC toolchain: `cargo +1.96-x86_64-pc-windows-msvc test` if your default host is gnu.
- **Mirror checks** run after every settle, comparing native and core children, props, frames, focus and scroll offsets (§7.3). A failure names the node and the difference.
- **Visual baselines** are stored per backend and machine image in `tests/visual/<name>/<image>/` (ARCHITECTURE.md §12). The first run records them on your machine; look at them. CI records its own: a failing run uploads them, and `.github/scripts/accept-snapshots.sh <run id>` accepts them.
- **Headless-only tests** (`#[mitsuami_test::test(headless)]`: fake metrics, simulated system changes) are skipped in native runs.
- **Your real services** aren't exercised by app tests, which use a scripted fake. Copy `mitsuami-appkit/tests/services.rs`: a private clipboard if possible, the real menu structure plus an activation (submenus, check marks, roles, a window's own menus), an alert answered through its real button, and a cancelled file dialog.
- **Type-check what you can't run.** The GTK, Kirigami and WinUI backends type-check from macOS (CLAUDE.md has the commands); say in ARCHITECTURE.md what has only been type-checked.

## 18. Suggested order

1. Window, Container and Text, `SetFrame`, `measure` → the counter layout test passes natively.
2. Button, events, `perform`, `native_state` → the counter suite passes.
3. TextInput, Checkbox and Switch, including the "no events on programmatic set" guards and Enter-only submit → the forms suite passes.
4. Focus tracking, `SetFocusOrder`, `synthesize` → the Tab and focus conformance tests pass.
5. ScrollView → the scrolling conformance tests pass.
6. `run()` with tick, waker and timer → `run_loop_smoke` exits by itself; the showcase works.
7. Services, and your native services checks.
8. `capture`, and visual baselines.
9. The drawn view, then `NativeRender` and `NativeView` (§9) → the `escape_hatches` suite passes. The example has three widgets, each native on some platforms (a rating on AppKit and WinUI; a lock on GTK and Kirigami; a pips pager on WinUI and Kirigami), in `crates/mitsuami/examples/escape_hatches/<widget>/<platform>.rs` (`macos`, `gtk`, `windows`, `kde`). Add a native render for each your platform has as a real control, and name it in the widget's `Render` impl with `native::<W>()`. For the others, prefer an ad hoc render built the way your platform's apps build that widget (`ad_hoc::<W>()`) over the drawn one; only a real platform control counts as native.
10. `List` (§10) → the `lists` and `contacts` suites pass.
11. The rest of the widgets and the window chrome, a suite at a time.

When something in the contract doesn't fit your platform, **change the contract rather than working around it**, and update the other backends and this guide. Capture and the clipboard became async for exactly this reason.
