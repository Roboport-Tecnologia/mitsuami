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

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;
use std::task::{Poll, Waker};

use mitsuami::App;
use mitsuami::core::style::Edges;
use mitsuami::core::{
    Align, ButtonRole, ButtonStyle, EventValue, FlexDirection, Justify, Length, NodeId, Prop, Size, TextStyle, Ui,
    UiEvent, WidgetKind,
};

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
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
        _ => return Err(format!("unknown kind {kind}")),
    })
}

/// Makes a node of `kind` (a `MitsuamiKind`), in no tree yet; 0 if it
/// fails.
#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_create(kind: u32) -> u32 {
    guard(|| Ok(ui()?.create(widget_kind(kind)?, Vec::new()).raw())).unwrap_or(0)
}

/// Opens a window with `title` and a content size in points. Closing it
/// destroys it; the app ends with its last window.
///
/// # Safety
/// The title is as [`string`] says.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mitsuami_create_window(title: *const u8, title_len: usize, width: f32, height: f32) -> u32 {
    guard(|| {
        let ui = ui()?;
        let title = unsafe { string(title, title_len)? };
        let window = ui.create_window(title, Size::new(width, height));
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
}

#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_bool(id: u32, prop: u32, value: bool) -> bool {
    guard(|| {
        let prop = match prop {
            0 => Prop::Enabled(value),
            1 => Prop::Checked(value),
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
}

#[unsafe(no_mangle)]
pub extern "C" fn mitsuami_set_number(id: u32, prop: u32, value: f64) -> bool {
    guard(|| {
        let prop = match prop {
            0 => Prop::Number(value),
            _ => return Err(format!("unknown number prop {prop}")),
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
            };
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
