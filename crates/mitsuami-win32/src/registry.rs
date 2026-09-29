//! What window procedures see: a thread-local map from a window to its
//! node, and the little state they need to report events. They never
//! touch the backend's state, which may be borrowed while they run
//! (`SendMessage` calls them synchronously, from inside `apply`).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use mitsuami_core::{ButtonRole, EventSink, EventValue, NodeId, Size, UiEvent, WidgetKind};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows_sys::Win32::UI::Controls::{BST_CHECKED, TBM_SETPOS};
use windows_sys::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BN_CLICKED, BS_AUTO3STATE, BS_AUTOCHECKBOX, BS_TYPEMASK, CB_GETCURSEL, CBN_SELCHANGE, CallWindowProcW,
    DefWindowProcW, GA_ROOT, GWL_EXSTYLE, GWL_STYLE, GWLP_WNDPROC, GetAncestor, GetClientRect, GetWindowLongPtrW,
    IsWindow, MINMAXINFO, SIZE_MINIMIZED, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW, SetWindowLongPtrW, SetWindowPos,
    WA_INACTIVE, WM_ACTIVATE, WM_CLOSE, WM_COMMAND, WM_DPICHANGED, WM_GETMINMAXINFO, WM_HSCROLL, WM_KILLFOCUS,
    WM_NCDESTROY, WM_SETFOCUS, WM_SIZE, WM_VSCROLL, WNDPROC,
};

/// A trackbar's position (`WM_USER`), which the bindings lack.
pub(crate) const TBM_GETPOS: u32 = 0x400;

/// How a slider's positions (whole numbers, as trackbars have) map to its
/// values: each position is `unit` apart from `min`, and a vertical one
/// counts down, so that up is larger.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Scale {
    pub min: f64,
    pub max: f64,
    pub unit: f64,
    pub count: i32,
    pub vertical: bool,
    /// The last position reported or set: moves report only a change.
    pub last: isize,
}

impl Scale {
    /// One position per step; without one, per whole number where the
    /// range is wide enough for that, else a hundredth of it.
    pub fn new(min: f64, max: f64, step: Option<f64>, vertical: bool) -> Scale {
        let width = (max - min).max(0.0);
        let unit = match step.filter(|s| *s > 0.0) {
            Some(step) => step,
            None if width >= 10.0 && width.fract() == 0.0 => 1.0,
            None if width > 0.0 => width / 100.0,
            None => 1.0,
        };
        let count = (width / unit + 1e-9).floor().min(i32::MAX as f64) as i32;
        Scale { min, max, unit, count, vertical, last: 0 }
    }

    pub fn value(&self, pos: isize) -> f64 {
        let steps = if self.vertical { self.count as isize - pos } else { pos };
        (self.min + steps as f64 * self.unit).clamp(self.min, self.max.max(self.min))
    }

    pub fn pos(&self, value: f64) -> isize {
        let steps = ((value - self.min) / self.unit).round().clamp(0.0, self.count as f64) as isize;
        if self.vertical { self.count as isize - steps } else { steps }
    }
}

/// A top-level window's state its procedure needs.
#[derive(Default)]
pub(crate) struct WindowInfo {
    pub hwnd: isize,
    /// The content size last reported, and the one last asked for.
    pub reported: Cell<Option<Size>>,
    pub requested: Cell<Option<Size>>,
    pub min: Cell<Option<Size>>,
    pub lock_height: Cell<bool>,
    /// The control that had focus when the window was last active.
    pub last_focus: Cell<isize>,
    pub order: RefCell<Vec<NodeId>>,
}

/// One backend's state that its windows' procedures share.
#[derive(Default)]
pub(crate) struct Shared {
    pub events: RefCell<EventSink>,
    pub focus: Cell<Option<NodeId>>,
    pub hwnds: RefCell<HashMap<NodeId, isize>>,
    pub windows: RefCell<HashMap<NodeId, Rc<WindowInfo>>>,
    pub sliders: RefCell<HashMap<NodeId, Scale>>,
    pub roles: RefCell<HashMap<NodeId, ButtonRole>>,
    /// Places a window's controls again at its new DPI; set by the backend.
    #[allow(clippy::type_complexity)]
    pub rescale: RefCell<Option<Rc<dyn Fn(NodeId)>>>,
}

impl Shared {
    pub fn emit(&self, id: NodeId, event: UiEvent) {
        self.events.borrow().emit(id, event);
    }

    pub fn hwnd(&self, id: NodeId) -> Option<HWND> {
        self.hwnds.borrow().get(&id).map(|h| *h as HWND)
    }

    pub fn window(&self, id: NodeId) -> Option<Rc<WindowInfo>> {
        self.windows.borrow().get(&id).cloned()
    }

    /// The window whose top-level `HWND` this is.
    pub fn window_of_root(&self, root: HWND) -> Option<(NodeId, Rc<WindowInfo>)> {
        self.windows.borrow().iter().find(|(_, w)| w.hwnd == root as isize).map(|(id, w)| (*id, w.clone()))
    }

    fn focus_in(&self, id: NodeId) {
        if self.focus.get() == Some(id) {
            return;
        }
        if let Some(old) = self.focus.replace(Some(id)) {
            self.emit(old, UiEvent::FocusOut);
        }
        self.emit(id, UiEvent::FocusIn);
    }

    fn focus_out(&self, id: NodeId) {
        if self.focus.get() == Some(id) {
            self.focus.set(None);
            self.emit(id, UiEvent::FocusOut);
        }
    }

    /// Reports a window's content size if it changed: the one asked for,
    /// when that's what it got in pixels.
    pub fn report_size(&self, id: NodeId, info: &WindowInfo) {
        let hwnd = info.hwnd as HWND;
        let mut client = RECT::default();
        unsafe { GetClientRect(hwnd, &mut client) };
        let scale = scale(hwnd);
        let (width, height) = (client.right - client.left, client.bottom - client.top);
        let size = match info.requested.get() {
            Some(asked) if pixels(asked.width, scale) == width && pixels(asked.height, scale) == height => asked,
            _ => Size::new(width as f32 / scale, height as f32 / scale),
        };
        if info.reported.replace(Some(size)) != Some(size) {
            self.emit(id, UiEvent::WindowResized(size));
        }
    }
}

struct Entry {
    shared: Rc<Shared>,
    id: NodeId,
    kind: WidgetKind,
}

thread_local! {
    static REGISTRY: RefCell<HashMap<isize, Entry>> = RefCell::new(HashMap::new());
    /// Controls' own procedures, which ours call on to.
    static ORIGINAL: RefCell<HashMap<isize, WNDPROC>> = RefCell::new(HashMap::new());
}

pub(crate) fn register(hwnd: HWND, shared: &Rc<Shared>, id: NodeId, kind: WidgetKind) {
    shared.hwnds.borrow_mut().insert(id, hwnd as isize);
    REGISTRY.with_borrow_mut(|r| r.insert(hwnd as isize, Entry { shared: shared.clone(), id, kind }));
}

pub(crate) fn unregister(hwnd: HWND) {
    let entry = REGISTRY.with_borrow_mut(|r| r.remove(&(hwnd as isize)));
    if let Some(entry) = entry {
        entry.shared.hwnds.borrow_mut().remove(&entry.id);
    }
}

pub(crate) fn lookup(hwnd: HWND) -> Option<(Rc<Shared>, NodeId, WidgetKind)> {
    REGISTRY.with_borrow(|r| r.get(&(hwnd as isize)).map(|e| (e.shared.clone(), e.id, e.kind)))
}

/// The node `hwnd` is, or is part of.
pub(crate) fn lookup_up(mut hwnd: HWND) -> Option<(Rc<Shared>, NodeId, WidgetKind, HWND)> {
    while !hwnd.is_null() {
        if let Some((shared, id, kind)) = lookup(hwnd) {
            return Some((shared, id, kind, hwnd));
        }
        hwnd = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetParent(hwnd) };
    }
    None
}

/// Hears a control's focus changes, which go to the control itself.
pub(crate) fn subclass(hwnd: HWND) {
    let original = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, control_proc as *const () as isize) };
    let original: WNDPROC = unsafe { std::mem::transmute(original) };
    ORIGINAL.with_borrow_mut(|o| o.insert(hwnd as isize, original));
}

unsafe extern "system" fn control_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_SETFOCUS => {
            if let Some((shared, id, _)) = lookup(hwnd) {
                shared.focus_in(id);
                let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
                if let Some((_, window)) = shared.window_of_root(root) {
                    window.last_focus.set(hwnd as isize);
                }
            }
        }
        WM_KILLFOCUS => {
            if let Some((shared, id, _)) = lookup(hwnd) {
                shared.focus_out(id);
            }
        }
        _ => {}
    }
    let original = ORIGINAL.with_borrow(|o| o.get(&(hwnd as isize)).copied().flatten());
    if message == WM_NCDESTROY {
        ORIGINAL.with_borrow_mut(|o| o.remove(&(hwnd as isize)));
    }
    unsafe { CallWindowProcW(original, hwnd, message, wparam, lparam) }
}

/// Windows and hosts: their controls' notifications come here.
pub(crate) unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_COMMAND if lparam != 0 => {
            let control = lparam as HWND;
            if let Some((shared, id, kind)) = lookup(control) {
                let code = (wparam >> 16) as u32 & 0xffff;
                match (kind, code) {
                    (WidgetKind::Button, BN_CLICKED) => shared.emit(id, UiEvent::Click),
                    (WidgetKind::Checkbox, BN_CLICKED) => {
                        let checked = checkbox_clicked(control);
                        shared.emit(id, UiEvent::Changed(EventValue::Bool(checked)));
                    }
                    (WidgetKind::Select, CBN_SELCHANGE) => {
                        let index = unsafe { SendMessageW(control, CB_GETCURSEL, 0, 0) };
                        if index >= 0 {
                            shared.emit(id, UiEvent::Changed(EventValue::Index(index as usize)));
                        }
                    }
                    _ => {}
                }
                return 0;
            }
        }
        WM_HSCROLL | WM_VSCROLL if lparam != 0 => {
            let control = lparam as HWND;
            if let Some((shared, id, WidgetKind::Slider)) = lookup(control) {
                slider_moved(&shared, id, control);
                return 0;
            }
        }
        WM_CLOSE => {
            if let Some((shared, id, WidgetKind::Window)) = lookup(hwnd) {
                // The app decides.
                shared.emit(id, UiEvent::WindowCloseRequested);
                return 0;
            }
        }
        WM_SIZE if wparam != SIZE_MINIMIZED as usize => {
            if let Some((shared, id, WidgetKind::Window)) = lookup(hwnd)
                && let Some(info) = shared.window(id)
            {
                shared.report_size(id, &info);
            }
        }
        WM_GETMINMAXINFO => {
            if let Some((shared, id, WidgetKind::Window)) = lookup(hwnd)
                && let Some(info) = shared.window(id)
            {
                let limits = unsafe { &mut *(lparam as *mut MINMAXINFO) };
                let scale = scale(hwnd);
                if let Some(min) = info.min.get().map(|min| screen_bound(hwnd, min)) {
                    let (w, h) = outer_size(hwnd, pixels(min.width, scale), pixels(min.height, scale));
                    limits.ptMinTrackSize.x = w;
                    limits.ptMinTrackSize.y = h;
                }
                if info.lock_height.get() {
                    let mut client = RECT::default();
                    unsafe { GetClientRect(hwnd, &mut client) };
                    let (_, h) = outer_size(hwnd, 0, client.bottom - client.top);
                    limits.ptMinTrackSize.y = h;
                    limits.ptMaxTrackSize.y = h;
                }
                return 0;
            }
        }
        // The window moved to a screen with another DPI: take the size
        // Windows suggests, and place and size the controls again.
        WM_DPICHANGED => {
            if let Some((shared, id, WidgetKind::Window)) = lookup(hwnd) {
                let r = unsafe { &*(lparam as *const RECT) };
                unsafe {
                    SetWindowPos(
                        hwnd,
                        std::ptr::null_mut(),
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
                let rescale = shared.rescale.borrow().clone();
                if let Some(rescale) = rescale {
                    rescale(id);
                }
                shared.emit(id, UiEvent::MetricsChanged);
                return 0;
            }
        }
        // As a dialog does: focus goes back to the control that had it.
        WM_ACTIVATE if (wparam & 0xffff) as u32 != WA_INACTIVE && restore_focus(hwnd) => return 0,
        WM_SETFOCUS if restore_focus(hwnd) => return 0,
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn restore_focus(hwnd: HWND) -> bool {
    let Some((shared, id, WidgetKind::Window)) = lookup(hwnd) else { return false };
    let Some(info) = shared.window(id) else { return false };
    let last = info.last_focus.get() as HWND;
    if last.is_null() || unsafe { IsWindow(last) } == 0 || lookup(last).is_none() {
        return false;
    }
    if unsafe { GetFocus() } != last {
        unsafe { SetFocus(last) };
    }
    true
}

/// A click on a checkbox: it has toggled itself. One that was mixed took
/// three states for that, and landed where Windows lands (from mixed,
/// unchecked); it takes two from now on, so it doesn't come back to mixed.
fn checkbox_clicked(control: HWND) -> bool {
    unsafe {
        let style = GetWindowLongPtrW(control, GWL_STYLE);
        if style & BS_TYPEMASK as isize == BS_AUTO3STATE as isize {
            SetWindowLongPtrW(control, GWL_STYLE, (style & !(BS_TYPEMASK as isize)) | BS_AUTOCHECKBOX as isize);
            InvalidateRect(control, std::ptr::null(), 1);
        }
        SendMessageW(control, BM_GETCHECK, 0, 0) == BST_CHECKED as isize
    }
}

fn slider_moved(shared: &Shared, id: NodeId, control: HWND) {
    let pos = unsafe { SendMessageW(control, TBM_GETPOS, 0, 0) };
    let value = {
        let mut sliders = shared.sliders.borrow_mut();
        let Some(scale) = sliders.get_mut(&id) else { return };
        if scale.last == pos {
            return;
        }
        scale.last = pos;
        scale.value(pos)
    };
    shared.emit(id, UiEvent::Changed(EventValue::Number(value)));
}

/// Moves a slider without reporting it.
pub(crate) fn set_slider(shared: &Shared, id: NodeId, control: HWND, pos: isize) {
    if let Some(scale) = shared.sliders.borrow_mut().get_mut(&id) {
        scale.last = pos;
    }
    unsafe { SendMessageW(control, TBM_SETPOS, 1, pos) };
}

/// Pixels to logical units, at the window's DPI.
pub(crate) fn scale(hwnd: HWND) -> f32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

pub(crate) fn pixels(logical: f32, scale: f32) -> i32 {
    (logical * scale).round() as i32
}

/// A minimum content size no larger than the content of a window filling
/// the screen's work area (the screen less the taskbar).
pub(crate) fn screen_bound(hwnd: HWND, size: Size) -> Size {
    unsafe {
        let mut monitor = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..std::mem::zeroed() };
        if GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut monitor) == 0 {
            return size;
        }
        let work = monitor.rcWork;
        let (frame_w, frame_h) = outer_size(hwnd, 0, 0);
        let s = scale(hwnd);
        let width = (work.right - work.left - frame_w) as f32 / s;
        let height = (work.bottom - work.top - frame_h) as f32 / s;
        Size::new(size.width.min(width.floor()), size.height.min(height.floor()))
    }
}

/// A window's size for a client area this large.
pub(crate) fn outer_size(hwnd: HWND, width: i32, height: i32) -> (i32, i32) {
    let mut rect = RECT { left: 0, top: 0, right: width, bottom: height };
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        AdjustWindowRectExForDpi(&mut rect, style, 0, ex_style, GetDpiForWindow(hwnd));
    }
    (rect.right - rect.left, rect.bottom - rect.top)
}
