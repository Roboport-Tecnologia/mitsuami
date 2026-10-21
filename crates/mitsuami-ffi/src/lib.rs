//! mitsuami over a C ABI, for bindings to other languages. It drives the
//! core's [`Ui`] directly: create a node of a kind, set its props and
//! style, put it in the tree, hear its events.
//! mitsuami's builder API (generics, closures, `view!`) can't cross a C
//! ABI, and the widgets are thin wrappers over these calls anyway.
//!
//! - Every function runs on the UI thread, in `mitsuami_run`'s setup, an
//!   event or a posted callback, except `mitsuami_post`, which any thread
//!   may call.
//! - Strings are UTF-8, as a pointer and a length. Strings given to a
//!   callback live until it returns.
//! - A node is its id's number. The core never reuses one, so a stale node
//!   is never another node; calls on a destroyed one do nothing.
//! - Nothing panics across the boundary: a call that fails returns false
//!   (or a node of 0), and `mitsuami_last_error` says why.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Mutex;
use std::task::{Poll, Waker};

use mitsuami::App;
use mitsuami::core::services::{Alert, AlertStyle, OpenFile};
use mitsuami::core::style::Edges;
use mitsuami::core::{
    Align, ButtonRole, ButtonStyle, DocumentTabsStyle, EventValue, FlexDirection, Justify, Length, List, ListStyle,
    Modality, NodeId, Prop, Rect, SelectionMode, Size, TextStyle, Ui, UiEvent, VectorIcon, View, WidgetKind,
    WindowSize,
};
use mitsuami::reactive::{Owner, Signal, on_cleanup, signal, watch};

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
    static LISTS: RefCell<HashMap<u32, ListState>> = RefCell::new(HashMap::new());
    static LAST_ERROR: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Makes `ui` the one these functions drive, and starts taking posted
/// callbacks on it. `mitsuami_run` calls it; tests and hosts that make
/// their own `Ui` call it themselves.
pub fn attach(ui: &Ui) {
    UI.with(|u| *u.borrow_mut() = Some(ui.clone()));
    ui.spawn_local(take_posted());
}

fn ui() -> Result<Ui, String> {
    UI.with(|u| u.borrow().clone()).ok_or_else(|| "no Ui: call from mitsuami_run's setup".to_string())
}

/// Runs `f`, turning an error or a panic into `None` and the last error.
fn guard<T>(f: impl FnOnce() -> Result<T, String>) -> Option<T> {
    let error = match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(value)) => return Some(value),
        Ok(Err(error)) => error,
        Err(panic) => panic
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".to_string()),
    };
    LAST_ERROR.with(|e| *e.borrow_mut() = error.into_bytes());
    None
}

/// # Safety
/// `ptr` points to `len` bytes, or is null with `len` 0.
unsafe fn string(ptr: *const u8, len: usize) -> Result<String, String> {
    if ptr.is_null() {
        return if len == 0 { Ok(String::new()) } else { Err("null string".to_string()) };
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    String::from_utf8(bytes.to_vec()).map_err(|_| "string isn't UTF-8".to_string())
}

fn node(id: u32) -> NodeId {
    NodeId::from_raw(id)
}

/// The last error on this thread, as UTF-8: its length, with `*ptr` set to
/// its bytes, valid until the next call that fails.
///
/// # Safety
/// `ptr` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_last_error(ptr: *mut *const u8) -> usize {
    LAST_ERROR.with(|e| {
        let e = e.borrow();
        unsafe { *ptr = e.as_ptr() };
        e.len()
    })
}

/// Runs the app until its last window closes, calling `setup(user)` once
/// it has started, to open its windows. Call it on the main thread: AppKit
/// runs only there.
///
/// # Safety
/// The strings are as [`string`] says; `setup` is a valid function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_run(
    id: *const u8,
    id_len: usize,
    name: *const u8,
    name_len: usize,
    setup: extern "C" fn(*mut c_void),
    user: *mut c_void,
) -> bool {
    let user = user as usize;
    guard(|| {
        let (id, name) = unsafe { (string(id, id_len)?, string(name, name_len)?) };
        let mut app = App::new();
        if !id.is_empty() {
            app = app.id(id);
        }
        if !name.is_empty() {
            app = app.name(name);
        }
        app.setup(move |ui| {
            attach(ui);
            setup(user as *mut c_void);
        })
        .run();
        Ok(())
    })
    .is_some()
}

// Posting from other threads.

type Posted = (extern "C" fn(*mut c_void), usize);

static POSTED: Mutex<VecDeque<Posted>> = Mutex::new(VecDeque::new());
static WAKER: Mutex<Option<Waker>> = Mutex::new(None);

/// Calls `callback(user)` on the UI thread soon. Any thread may call it.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_post(callback: extern "C" fn(*mut c_void), user: *mut c_void) {
    POSTED.lock().unwrap().push_back((callback, user as usize));
    if let Some(waker) = WAKER.lock().unwrap().as_ref() {
        waker.wake_by_ref();
    }
}

/// A task on the UI thread that runs what other threads post. Its waker is
/// `Send`, so posting wakes the platform's run loop as finished background
/// work does. It never ends: tests shouldn't wait for every task.
async fn take_posted() {
    std::future::poll_fn(|cx| {
        *WAKER.lock().unwrap() = Some(cx.waker().clone());
        loop {
            let Some((callback, user)) = POSTED.lock().unwrap().pop_front() else { break };
            if guard(|| {
                callback(user as *mut c_void);
                Ok(())
            })
            .is_none()
            {
                eprintln!("mitsuami: a posted callback panicked");
            }
        }
        Poll::<()>::Pending
    })
    .await
}

// Nodes.

/// The widgets the C ABI makes so far.
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum MitsuamiKind {
    Container = 0,
    Text = 1,
    Button = 2,
    ToggleButton = 3,
    Checkbox = 4,
    Switch = 5,
    TextInput = 6,
    PasswordInput = 7,
    SearchInput = 8,
    TextArea = 9,
    Separator = 10,
    Spinner = 11,
    Progress = 12,
    /// Tabs for the documents an app has open, as data: the app shows the
    /// chosen one itself.
    DocumentTabs = 13,
    /// The platform's drop-down: one of its options is always chosen.
    Select = 14,
    /// An icon: a name in the platform's own set, or the app's own.
    Icon = 15,
}

fn widget_kind(kind: u32) -> Result<WidgetKind, String> {
    Ok(match kind {
        0 => WidgetKind::Container,
        1 => WidgetKind::Text,
        2 => WidgetKind::Button,
        3 => WidgetKind::ToggleButton,
        4 => WidgetKind::Checkbox,
        5 => WidgetKind::Switch,
        6 => WidgetKind::TextInput,
        7 => WidgetKind::PasswordInput,
        8 => WidgetKind::SearchInput,
        9 => WidgetKind::TextArea,
        10 => WidgetKind::Separator,
        11 => WidgetKind::Spinner,
        12 => WidgetKind::Progress,
        13 => WidgetKind::DocumentTabs,
        14 => WidgetKind::Select,
        15 => WidgetKind::Icon,
        _ => return Err(format!("unknown kind {kind}")),
    })
}

/// Makes a node of `kind` (a `MitsuamiKind`), in no tree yet; 0 if it
/// fails.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_create(kind: u32) -> u32 {
    guard(|| Ok(ui()?.create(widget_kind(kind)?, Vec::new()).raw())).unwrap_or(0)
}

/// Opens a window with `title` and a content size in points; a negative
/// height follows the content's height, as a dialog's does. Closing it
/// destroys it; the app ends with its last window.
///
/// # Safety
/// The title is as [`string`] says.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_create_window(title: *const u8, title_len: usize, width: f32, height: f32) -> u32 {
    guard(|| {
        let ui = ui()?;
        let title = unsafe { string(title, title_len)? };
        let size = if height < 0.0 { WindowSize::FollowHeight(width) } else { Size::new(width, height).into() };
        let window = ui.create_window(title, size);
        let weak = ui.downgrade();
        ui.on_event(window, move |event| {
            if *event == UiEvent::WindowCloseRequested
                && let Some(ui) = weak.upgrade()
            {
                ui.destroy(window);
            }
        });
        Ok(window.raw())
    })
    .unwrap_or(0)
}

/// Makes a window just made modal, before it shows: `modality` 0 blocks
/// `owner` (a sheet on macOS), 1 the whole app. Without an owner (0), it
/// blocks the whole app.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_modal(window: u32, owner: u32, modality: u32) -> bool {
    guard(|| {
        let owner = (owner != 0).then(|| node(owner));
        let modality = match (modality, owner) {
            (0, Some(_)) => Modality::Window,
            (0 | 1, _) => Modality::Application,
            _ => return Err(format!("bad modality {modality}")),
        };
        ui()?.set_prop(node(window), Prop::Modal { owner, modality });
        Ok(())
    })
    .is_some()
}

/// The bounds of a path's shapes, in its own units, into `out` (x, y,
/// width, height): for an app that pads an icon in a view box, to size it
/// against others. False if the data doesn't parse.
///
/// # Safety
/// The path data is as [`string`] says, and `out` holds four floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_vector_icon_bounds(data: MitsuamiStr, out: *mut f32) -> bool {
    guard(|| {
        let data = unsafe { string(data.ptr, data.len)? };
        let bounds = VectorIcon::new(&data)?.view_box();
        let out = unsafe { std::slice::from_raw_parts_mut(out, 4) };
        out.copy_from_slice(&[bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height]);
        Ok(())
    })
    .is_some()
}

/// Makes an icon showing the app's own icon, from path data as for
/// [`mitsuami_set_vector_icon`]; 0 if it fails. WinUI makes an icon a
/// `FontIcon` or a `PathIcon` by its first props, so an icon that will
/// show the app's own is made so.
///
/// # Safety
/// The path data is as [`string`] says.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_create_vector_icon(
    data: MitsuamiStr,
    view_x: f32,
    view_y: f32,
    view_width: f32,
    view_height: f32,
) -> u32 {
    guard(|| {
        let data = unsafe { string(data.ptr, data.len)? };
        let icon = match view_width > 0.0 {
            true => VectorIcon::with_view_box(&data, Rect::new(view_x, view_y, view_width, view_height))?,
            false => VectorIcon::new(&data)?,
        };
        Ok(ui()?.create(WidgetKind::Icon, vec![Prop::VectorIcon(Some(icon))]).raw())
    })
    .unwrap_or(0)
}

/// The app's own icon on an icon or a button, from path data in the SVG
/// syntax, fitted to its view box, or to its bounds if `view_width` isn't
/// positive. Empty data takes it away. False if the data doesn't parse.
///
/// # Safety
/// The path data is as [`string`] says.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_set_vector_icon(
    id: u32,
    data: MitsuamiStr,
    view_x: f32,
    view_y: f32,
    view_width: f32,
    view_height: f32,
) -> bool {
    guard(|| {
        let data = unsafe { string(data.ptr, data.len)? };
        let icon = match data.trim() {
            "" => None,
            _ if view_width > 0.0 => {
                Some(VectorIcon::with_view_box(&data, Rect::new(view_x, view_y, view_width, view_height))?)
            }
            _ => Some(VectorIcon::new(&data)?),
        };
        ui()?.set_prop(node(id), Prop::VectorIcon(icon));
        Ok(())
    })
    .is_some()
}

/// Puts `child` in `parent` at `index`, or last if `index` is past the end.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_insert_child(parent: u32, child: u32, index: usize) -> bool {
    guard(|| {
        ui()?.insert_child(node(parent), node(child), Some(index));
        Ok(())
    })
    .is_some()
}

#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_append_child(parent: u32, child: u32) -> bool {
    guard(|| {
        ui()?.append_child(node(parent), node(child));
        Ok(())
    })
    .is_some()
}

/// Destroys a node and everything in it.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_destroy(id: u32) -> bool {
    guard(|| {
        ui()?.destroy(node(id));
        // Known gap: a list inside a destroyed node keeps its scope.
        if let Some(list) = LISTS.with(|l| l.borrow_mut().remove(&id)) {
            list.owner.dispose();
        }
        Ok(())
    })
    .is_some()
}

// Props.

#[repr(u32)]
pub enum MitsuamiTextProp {
    /// A `Text`'s text.
    Text = 0,
    /// A button's caption, a checkbox's or a switch's label.
    Label = 1,
    /// A window's title.
    Title = 2,
    /// A text field's text.
    Value = 3,
    Placeholder = 4,
    /// The name assistive technology reads.
    A11yLabel = 5,
    /// What the platform shows by the pointer when it rests on a control.
    Tooltip = 6,
    /// An icon's name in the platform's own set: an SF Symbol, a themed
    /// icon, a Segoe Fluent Icons glyph.
    Icon = 7,
}

/// # Safety
/// The value is as [`string`] says.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_set_text(id: u32, prop: u32, value: *const u8, len: usize) -> bool {
    guard(|| {
        let ui = ui()?;
        let value = unsafe { string(value, len)? };
        let prop = match prop {
            0 => Prop::Text(value),
            1 => Prop::Label(value),
            2 => Prop::Title(value),
            3 => Prop::Value(value),
            4 => Prop::Placeholder(value),
            6 => Prop::Tooltip(value),
            7 => Prop::Icon(value),
            5 => {
                ui.update_a11y(node(id), |a11y| a11y.label = Some(value));
                return Ok(());
            }
            _ => return Err(format!("unknown text prop {prop}")),
        };
        ui.set_prop(node(id), prop);
        Ok(())
    })
    .is_some()
}

#[repr(u32)]
pub enum MitsuamiBoolProp {
    Enabled = 0,
    Checked = 1,
    /// Document tabs' close buttons (and the platform's close keys).
    TabsClosable = 2,
    /// Document tabs the user can drag to another place.
    TabsReorderable = 3,
    /// Document tabs' new-tab button.
    NewTabButton = 4,
    /// A button shows only its icon; its caption stays its name.
    IconOnly = 5,
}

#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_bool(id: u32, prop: u32, value: bool) -> bool {
    guard(|| {
        let prop = match prop {
            0 => Prop::Enabled(value),
            1 => Prop::Checked(value),
            2 => Prop::TabsClosable(value),
            3 => Prop::TabsReorderable(value),
            4 => Prop::NewTabButton(value),
            5 => Prop::IconOnly(value),
            _ => return Err(format!("unknown bool prop {prop}")),
        };
        ui()?.set_prop(node(id), prop);
        Ok(())
    })
    .is_some()
}

#[repr(u32)]
pub enum MitsuamiNumberProp {
    /// A progress bar's, a slider's or a number field's value.
    Number = 0,
    /// A list's guess at its rows' height, for rows not built yet.
    EstimatedRowHeight = 1,
    /// An icon's size, in points.
    IconSize = 2,
}

#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_number(id: u32, prop: u32, value: f64) -> bool {
    guard(|| {
        let prop = match prop {
            0 => Prop::Number(value),
            1 => Prop::EstimatedRowHeight(value as f32),
            2 => Prop::IconSize(value as f32),
            _ => return Err(format!("unknown number prop {prop}")),
        };
        ui()?.set_prop(node(id), prop);
        Ok(())
    })
    .is_some()
}

#[repr(u32)]
pub enum MitsuamiStringsProp {
    /// Document tabs' titles, in order.
    TabTitles = 0,
    /// A drop-down's options, in order.
    Options = 1,
}

/// # Safety
/// `values` points to `count` strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_set_strings(id: u32, prop: u32, values: *const MitsuamiStr, count: usize) -> bool {
    guard(|| {
        let values = unsafe { strings(values, count)? };
        let prop = match prop {
            0 => Prop::TabTitles(values),
            1 => Prop::Options(values),
            _ => return Err(format!("unknown strings prop {prop}")),
        };
        ui()?.set_prop(node(id), prop);
        Ok(())
    })
    .is_some()
}

#[repr(u32)]
pub enum MitsuamiIndexProp {
    /// The chosen document tab, option, item or tab.
    SelectedIndex = 0,
}

/// Sets an index; a negative one is none.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_index(id: u32, prop: u32, index: isize) -> bool {
    guard(|| {
        let index = usize::try_from(index).ok();
        let prop = match prop {
            0 => Prop::SelectedIndex(index),
            _ => return Err(format!("unknown index prop {prop}")),
        };
        ui()?.set_prop(node(id), prop);
        Ok(())
    })
    .is_some()
}

#[repr(u32)]
pub enum MitsuamiEnumProp {
    /// `LargeTitle`, `Title`, `Headline`, `Body`, `Callout`, `Caption`,
    /// `Monospace`: 0 to 6.
    TextStyle = 0,
    /// `Normal`, `Default`, `Cancel`, `Destructive`: 0 to 3.
    ButtonRole = 1,
    /// `Automatic`, `Bordered`, `Borderless`: 0 to 2.
    ButtonStyle = 2,
    /// A list's: `None`, `Single`, `Multiple`, 0 to 2.
    SelectionMode = 3,
    /// A list's: `Automatic`, `Plain`, `Framed`, 0 to 2.
    ListStyle = 4,
    /// Document tabs': `Automatic`, `Strip`, `Window`, `TitleRow`, 0 to 3.
    DocumentTabsStyle = 5,
}

#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_enum(id: u32, prop: u32, value: u32) -> bool {
    guard(|| {
        let bad = || format!("bad value {value} for enum prop {prop}");
        let prop = match prop {
            0 => Prop::TextStyle(match value {
                0 => TextStyle::LargeTitle,
                1 => TextStyle::Title,
                2 => TextStyle::Headline,
                3 => TextStyle::Body,
                4 => TextStyle::Callout,
                5 => TextStyle::Caption,
                6 => TextStyle::Monospace,
                _ => return Err(bad()),
            }),
            1 => Prop::ButtonRole(match value {
                0 => ButtonRole::Normal,
                1 => ButtonRole::Default,
                2 => ButtonRole::Cancel,
                3 => ButtonRole::Destructive,
                _ => return Err(bad()),
            }),
            2 => Prop::ButtonStyle(match value {
                0 => ButtonStyle::Automatic,
                1 => ButtonStyle::Bordered,
                2 => ButtonStyle::Borderless,
                _ => return Err(bad()),
            }),
            3 => Prop::SelectionMode(match value {
                0 => SelectionMode::None,
                1 => SelectionMode::Single,
                2 => SelectionMode::Multiple,
                _ => return Err(bad()),
            }),
            4 => Prop::ListStyle(match value {
                0 => ListStyle::Automatic,
                1 => ListStyle::Plain,
                2 => ListStyle::Framed,
                _ => return Err(bad()),
            }),
            5 => Prop::DocumentTabsStyle(match value {
                0 => DocumentTabsStyle::Automatic,
                1 => DocumentTabsStyle::Strip,
                2 => DocumentTabsStyle::Window,
                3 => DocumentTabsStyle::TitleRow,
                _ => return Err(bad()),
            }),
            _ => return Err(format!("unknown enum prop {prop}")),
        };
        ui()?.set_prop(node(id), prop);
        Ok(())
    })
    .is_some()
}

// Style. Lengths are in points; a negative one is `Auto`.

fn length(points: f32) -> Length {
    if points < 0.0 { Length::Auto } else { Length::Px(points) }
}

fn align(value: u32) -> Result<Option<Align>, String> {
    Ok(Some(match value {
        0 => return Ok(None),
        1 => Align::Start,
        2 => Align::End,
        3 => Align::Center,
        4 => Align::Stretch,
        5 => Align::Baseline,
        _ => return Err(format!("bad align {value}")),
    }))
}

/// Hides a node, or shows it again: it takes no room while hidden, and
/// keeps its layout for when it's back.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_hidden(id: u32, hidden: bool) -> bool {
    guard(|| {
        ui()?.update_style(node(id), |s| s.hidden = hidden);
        Ok(())
    })
    .is_some()
}

/// A container's direction: 0 a column (the default), 1 a row.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_direction(id: u32, row: bool) -> bool {
    let direction = if row { FlexDirection::Row } else { FlexDirection::Column };
    guard(|| {
        ui()?.update_style(node(id), |s| s.flex_direction = direction);
        Ok(())
    })
    .is_some()
}

/// The space between a container's children.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_gap(id: u32, gap: f32) -> bool {
    guard(|| {
        ui()?.update_style(node(id), |s| {
            s.row_gap = length(gap);
            s.column_gap = length(gap);
        });
        Ok(())
    })
    .is_some()
}

/// Padding, with start and end following the text's direction.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_padding(id: u32, top: f32, end: f32, bottom: f32, start: f32) -> bool {
    let edges = Edges { top: length(top), end: length(end), bottom: length(bottom), start: length(start) };
    guard(|| {
        ui()?.update_style(node(id), |s| s.padding = edges);
        Ok(())
    })
    .is_some()
}

#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_margin(id: u32, top: f32, end: f32, bottom: f32, start: f32) -> bool {
    let edges = Edges { top: length(top), end: length(end), bottom: length(bottom), start: length(start) };
    guard(|| {
        ui()?.update_style(node(id), |s| s.margin = edges);
        Ok(())
    })
    .is_some()
}

#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_size(id: u32, width: f32, height: f32) -> bool {
    guard(|| {
        ui()?.update_style(node(id), |s| {
            s.width = length(width);
            s.height = length(height);
        });
        Ok(())
    })
    .is_some()
}

#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_grow(id: u32, grow: f32) -> bool {
    guard(|| {
        ui()?.update_style(node(id), |s| s.flex_grow = grow);
        Ok(())
    })
    .is_some()
}

/// How a container aligns its children across its direction: 0 unset,
/// then `Start`, `End`, `Center`, `Stretch`, `Baseline`.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_align_items(id: u32, align_items: u32) -> bool {
    guard(|| {
        let value = align(align_items)?;
        ui()?.update_style(node(id), |s| s.align_items = value);
        Ok(())
    })
    .is_some()
}

/// How a node aligns itself in its container, over the container's
/// `align_items`: as for that.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_align_self(id: u32, align_self: u32) -> bool {
    guard(|| {
        let value = align(align_self)?;
        ui()?.update_style(node(id), |s| s.align_self = value);
        Ok(())
    })
    .is_some()
}

/// How a container spreads its children along its direction: 0 unset, then
/// `Start`, `End`, `Center`, `Stretch`, `SpaceBetween`, `SpaceAround`,
/// `SpaceEvenly`.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_justify(id: u32, justify: u32) -> bool {
    guard(|| {
        let justify = match justify {
            0 => None,
            1 => Some(Justify::Start),
            2 => Some(Justify::End),
            3 => Some(Justify::Center),
            4 => Some(Justify::Stretch),
            5 => Some(Justify::SpaceBetween),
            6 => Some(Justify::SpaceAround),
            7 => Some(Justify::SpaceEvenly),
            _ => return Err(format!("bad justify {justify}")),
        };
        ui()?.update_style(node(id), |s| s.justify_content = justify);
        Ok(())
    })
    .is_some()
}

// Events.

#[repr(u32)]
pub enum MitsuamiEventKind {
    Click = 0,
    /// The user changed the value; `value` has it.
    Changed = 1,
    /// Return in a text field.
    Submit = 2,
    FocusIn = 3,
    FocusOut = 4,
    /// The user asked to close a window, which then closes.
    CloseRequested = 5,
    /// A search field searches, when its platform does: `value` has the
    /// text.
    Search = 6,
    /// The user asked to close a document tab: `index` is which. It stays
    /// until the app takes it away.
    TabCloseRequested = 7,
    /// The user moved a document tab from `index` to `to`. It's shown there
    /// already, and the titles and chosen index follow.
    TabMoved = 8,
    /// The user asked for a new document tab.
    NewTabRequested = 9,
}

#[repr(u32)]
pub enum MitsuamiValueKind {
    None = 0,
    Text = 1,
    Bool = 2,
    Number = 3,
    Index = 4,
}

/// An event, with the value that came with it in the field its
/// `value_kind` names.
#[repr(C)]
pub struct MitsuamiEvent {
    pub kind: u32,
    pub value_kind: u32,
    pub text: *const u8,
    pub text_len: usize,
    pub flag: bool,
    pub number: f64,
    pub index: u64,
    /// Where a moved tab went.
    pub to: u64,
}

/// Calls `callback(user, node, event)` for each event on the node that the
/// C ABI knows. `user` is the caller's, kept as long as the node.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_on_event(
    id: u32,
    callback: extern "C" fn(*mut c_void, u32, *const MitsuamiEvent),
    user: *mut c_void,
) -> bool {
    let user = user as usize;
    guard(|| {
        ui()?.on_event(node(id), move |event| {
            let kind = match event {
                UiEvent::Click => MitsuamiEventKind::Click,
                UiEvent::Changed(_) => MitsuamiEventKind::Changed,
                UiEvent::Submit => MitsuamiEventKind::Submit,
                UiEvent::FocusIn => MitsuamiEventKind::FocusIn,
                UiEvent::FocusOut => MitsuamiEventKind::FocusOut,
                UiEvent::WindowCloseRequested => MitsuamiEventKind::CloseRequested,
                UiEvent::Search(_) => MitsuamiEventKind::Search,
                UiEvent::TabCloseRequested(_) => MitsuamiEventKind::TabCloseRequested,
                UiEvent::TabMoved { .. } => MitsuamiEventKind::TabMoved,
                UiEvent::NewTabRequested => MitsuamiEventKind::NewTabRequested,
                _ => return,
            };
            let mut out = MitsuamiEvent {
                kind: kind as u32,
                value_kind: MitsuamiValueKind::None as u32,
                text: std::ptr::null(),
                text_len: 0,
                flag: false,
                number: 0.0,
                index: 0,
                to: 0,
            };
            match event {
                UiEvent::TabCloseRequested(index) => {
                    out.value_kind = MitsuamiValueKind::Index as u32;
                    out.index = *index as u64;
                }
                UiEvent::TabMoved { from, to } => {
                    out.value_kind = MitsuamiValueKind::Index as u32;
                    out.index = *from as u64;
                    out.to = *to as u64;
                }
                _ => {}
            }
            if let UiEvent::Search(text) = event {
                out.value_kind = MitsuamiValueKind::Text as u32;
                out.text = text.as_ptr();
                out.text_len = text.len();
            }
            if let UiEvent::Changed(value) = event {
                match value {
                    EventValue::Text(text) => {
                        out.value_kind = MitsuamiValueKind::Text as u32;
                        out.text = text.as_ptr();
                        out.text_len = text.len();
                    }
                    EventValue::Bool(b) => {
                        out.value_kind = MitsuamiValueKind::Bool as u32;
                        out.flag = *b;
                    }
                    EventValue::Number(n) => {
                        out.value_kind = MitsuamiValueKind::Number as u32;
                        out.number = *n;
                    }
                    EventValue::Index(i) => {
                        out.value_kind = MitsuamiValueKind::Index as u32;
                        out.index = *i as u64;
                    }
                    _ => {}
                }
            }
            if guard(|| {
                callback(user as *mut c_void, id, &out);
                Ok(())
            })
            .is_none()
            {
                eprintln!("mitsuami: an event callback panicked");
            }
        });
        Ok(())
    })
    .is_some()
}

// Lists.

/// A string a caller gives: UTF-8, a pointer and a length.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MitsuamiStr {
    pub ptr: *const u8,
    pub len: usize,
}

/// What a list asks of its caller. The list's id comes with each call.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MitsuamiListCallbacks {
    /// Builds the row for a key when the platform shows it: a node in no
    /// tree yet (0 for an empty row).
    pub build_row: extern "C" fn(*mut c_void, u32, *const u8, usize) -> u32,
    /// The platform let a row go: its node is destroyed.
    pub release_row: extern "C" fn(*mut c_void, u32, u32),
    /// The user changed the selection; `mitsuami_list_selected` has it.
    pub selection_changed: extern "C" fn(*mut c_void, u32),
    /// The user activated a row (double-click, Return): its key.
    pub activated: extern "C" fn(*mut c_void, u32, *const u8, usize),
    pub user: *mut c_void,
}

struct ListState {
    keys: Signal<Vec<String>>,
    selected: Signal<Vec<String>>,
    /// The selection as the caller last set it, so setting it isn't
    /// reported back as the user's.
    set: Rc<RefCell<Vec<String>>>,
    owner: Owner,
}

fn list<T>(id: u32, f: impl FnOnce(&ListState) -> T) -> Result<T, String> {
    LISTS.with(|l| l.borrow().get(&id).map(f)).ok_or_else(|| format!("node {id} isn't a list"))
}

/// # Safety
/// `strings` points to `count` strings, each as [`string`] says.
unsafe fn strings(strings: *const MitsuamiStr, count: usize) -> Result<Vec<String>, String> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let strings = unsafe { std::slice::from_raw_parts(strings, count) };
    strings.iter().map(|s| unsafe { string(s.ptr, s.len) }).collect()
}

/// A node the caller made, as a row's content.
struct Adopt(u32);

impl View for Adopt {
    fn build(self, ui: &Ui) -> NodeId {
        if self.0 == 0 { ui.create(WidgetKind::Container, Vec::new()) } else { node(self.0) }
    }
}

/// Makes a list: the platform's own list control, which builds rows (with
/// `build_row`) only as it shows them. Rows are keyed by strings, unique
/// in the list. It selects one row at a time until its `SelectionMode`
/// says otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_create_list(callbacks: MitsuamiListCallbacks) -> u32 {
    guard(|| {
        let ui = ui()?;
        let cb = Rc::new(callbacks);
        let id = Rc::new(Cell::new(0));
        let owner = Owner::new_root();
        let set = Rc::new(RefCell::new(Vec::new()));
        let (keys, selected, list) = owner.with(|| {
            let (keys, selected) = (signal(Vec::<String>::new()), signal(Vec::<String>::new()));
            let (row_cb, row_id) = (cb.clone(), id.clone());
            let (activate_cb, activate_id) = (cb.clone(), id.clone());
            let list = List::new(
                keys,
                |key: &String| key.clone(),
                move |key: String| {
                    let (cb, list) = (row_cb.clone(), row_id.get());
                    let row = (cb.build_row)(cb.user, list, key.as_ptr(), key.len());
                    if row != 0 {
                        on_cleanup(move || (cb.release_row)(cb.user, list, row));
                    }
                    Adopt(row)
                },
            )
            .selected(selected)
            .on_activate(move |key: String| {
                (activate_cb.activated)(activate_cb.user, activate_id.get(), key.as_ptr(), key.len())
            });
            let (changed_cb, changed_id, set) = (cb.clone(), id.clone(), set.clone());
            watch(
                move || selected.get(),
                move |now, _| {
                    if *now != *set.borrow() {
                        (changed_cb.selection_changed)(changed_cb.user, changed_id.get());
                    }
                },
            );
            (keys, selected, list.build(&ui))
        });
        id.set(list.raw());
        LISTS.with(|l| l.borrow_mut().insert(list.raw(), ListState { keys, selected, set, owner }));
        Ok(list.raw())
    })
    .unwrap_or(0)
}

/// The list's rows, by key, in order.
///
/// # Safety
/// `keys` points to `count` strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_list_set_keys(id: u32, keys: *const MitsuamiStr, count: usize) -> bool {
    guard(|| {
        let new = unsafe { strings(keys, count)? };
        list(id, |l| l.keys)?.set(new);
        Ok(())
    })
    .is_some()
}

/// Selects the rows with these keys.
///
/// # Safety
/// `keys` points to `count` strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_list_set_selected(id: u32, keys: *const MitsuamiStr, count: usize) -> bool {
    guard(|| {
        let new = unsafe { strings(keys, count)? };
        let (selected, set) = list(id, |l| (l.selected, l.set.clone()))?;
        *set.borrow_mut() = new.clone();
        selected.set(new);
        Ok(())
    })
    .is_some()
}

/// How many rows are selected; `mitsuami_list_selected_key` gives each.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_list_selected_count(id: u32) -> usize {
    guard(|| list(id, |l| l.selected.with(|s| s.len()))).unwrap_or(0)
}

/// The key of the `index`th selected row: its length, with `*ptr` set to
/// its bytes, valid until the selection changes.
///
/// # Safety
/// `ptr` is valid for a write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_list_selected_key(id: u32, index: usize, ptr: *mut *const u8) -> usize {
    guard(|| {
        list(id, |l| {
            l.selected.with(|s| {
                let key = s.get(index).map(|k| (k.as_ptr(), k.len())).unwrap_or((std::ptr::null(), 0));
                unsafe { *ptr = key.0 };
                key.1
            })
        })
    })
    .unwrap_or(0)
}

// Alerts and file dialogs. They never block: the answer comes to a
// callback, on the UI thread.

/// Shows an alert on `window` (0: the focused one), its buttons in order
/// of importance, the first the default; `style` is `Info`, `Warning` or
/// `Critical`, 0 to 2. `answer(user, index)` gets the button chosen.
///
/// # Safety
/// The strings are as [`string`] and [`strings`] say.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_alert(
    window: u32,
    title: MitsuamiStr,
    message: MitsuamiStr,
    buttons: *const MitsuamiStr,
    count: usize,
    style: u32,
    answer: extern "C" fn(*mut c_void, usize),
    user: *mut c_void,
) -> bool {
    let user = user as usize;
    guard(|| {
        let ui = ui()?;
        let (title, message) = unsafe { (string(title.ptr, title.len)?, string(message.ptr, message.len)?) };
        let mut alert = Alert::new(title);
        if !message.is_empty() {
            alert = alert.message(message);
        }
        alert.buttons = unsafe { strings(buttons, count)? };
        alert.style = match style {
            0 => AlertStyle::Info,
            1 => AlertStyle::Warning,
            2 => AlertStyle::Critical,
            _ => return Err(format!("bad alert style {style}")),
        };
        let reply = ui.alert((window != 0).then(|| node(window)), alert);
        ui.spawn_local(async move {
            let index = reply.await;
            answer(user as *mut c_void, index);
        });
        Ok(())
    })
    .is_some()
}

/// Asks for files to open, or with `folders` for folders, on `window` (0:
/// the focused one), starting in `start_folder` if it's there.
/// `answer(user, paths, count)` gets what was chosen: none if cancelled.
///
/// # Safety
/// The strings are as [`string`] says.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_open_file(
    window: u32,
    title: MitsuamiStr,
    folders: bool,
    multiple: bool,
    start_folder: MitsuamiStr,
    answer: extern "C" fn(*mut c_void, *const MitsuamiStr, usize),
    user: *mut c_void,
) -> bool {
    let user = user as usize;
    guard(|| {
        let ui = ui()?;
        let (title, start) = unsafe { (string(title.ptr, title.len)?, string(start_folder.ptr, start_folder.len)?) };
        let mut request = OpenFile::new();
        if !title.is_empty() {
            request = request.title(title);
        }
        request.directories = folders;
        request.multiple = multiple;
        request.start_folder = (!start.is_empty()).then(|| start.into());
        let reply = ui.open_file((window != 0).then(|| node(window)), request);
        ui.spawn_local(async move {
            let paths: Vec<String> =
                reply.await.unwrap_or_default().iter().map(|p| p.to_string_lossy().into_owned()).collect();
            let strs: Vec<MitsuamiStr> = paths.iter().map(|p| MitsuamiStr { ptr: p.as_ptr(), len: p.len() }).collect();
            answer(user as *mut c_void, strs.as_ptr(), strs.len());
        });
        Ok(())
    })
    .is_some()
}
