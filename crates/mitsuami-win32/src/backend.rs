//! The backend: every node a window, placed where the core says.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use mitsuami_core::a11y::{A11yAction, ActionError};
use mitsuami_core::backend::{CaptureError, FontSizes, Image};
use mitsuami_core::services::Reply;
use mitsuami_core::units::SpacingScale;
use mitsuami_core::{
    AppInfo, AvailableSpace, Backend, ButtonRole, Command, EventSink, EventValue, FontWeight, HorizontalAlign, Insets,
    Key, MeasureRequest, NativeAppInfo, NativeIcon, NativeState, NodeId, Orientation, PlatformMetrics, Point, Prop,
    Rect, Size, SyntheticInput, TextStyle, UiEvent, WidgetKind, find_prop,
};
use windows_sys::Win32::Foundation::{HWND, POINT, RECT, SIZE};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, CreateFontIndirectW,
    DIB_RGB_COLORS, DT_CALCRECT, DT_EXPANDTABS, DT_NOPREFIX, DT_WORDBREAK, DeleteDC, DeleteObject, DrawTextW,
    FW_NORMAL, FW_SEMIBOLD, GetDC, GetObjectW, GetTextExtentPoint32W, GetTextMetricsW, HDC, HFONT, InvalidateRect,
    MapWindowPoints, ReleaseDC, SelectObject, TEXTMETRICW,
};
use windows_sys::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::UI::Controls::{
    BCM_GETIDEALSIZE, BST_CHECKED, BST_INDETERMINATE, BST_UNCHECKED, PBM_GETPOS, PBM_SETMARQUEE, PBM_SETPOS,
    PBM_SETRANGE32, PBS_MARQUEE, TBM_SETPOSNOTIFY, TBM_SETRANGEMAX, TBM_SETRANGEMIN, TBM_SETTICFREQ, TBS_AUTOTICKS,
    TBS_NOTICKS, TBS_VERT,
};
use windows_sys::Win32::UI::HiDpi::{
    GetDpiForSystem, GetDpiForWindow, GetSystemMetricsForDpi, SystemParametersInfoForDpi,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, IsWindowEnabled, SetFocus, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT, VK_RETURN,
    VK_RIGHT, VK_SPACE, VK_TAB, VK_UP,
};
use windows_sys::Win32::UI::Shell::{GetCurrentProcessExplicitAppUserModelID, SetCurrentProcessExplicitAppUserModelID};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BN_CLICKED, BS_AUTO3STATE, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, BS_TYPEMASK,
    CB_ADDSTRING, CB_GETCOUNT, CB_GETCURSEL, CB_GETITEMHEIGHT, CB_GETLBTEXT, CB_GETLBTEXTLEN, CB_RESETCONTENT,
    CB_SETCURSEL, CB_SETITEMHEIGHT, CBS_DROPDOWNLIST, CW_USEDEFAULT, CreateIconFromResourceEx, CreateWindowExW,
    DestroyIcon, DestroyWindow, GA_ROOT, GW_CHILD, GW_HWNDNEXT, GWL_STYLE, GWLP_ID, GetAncestor, GetClientRect,
    GetIconInfo, GetParent, GetWindow, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, HICON,
    HWND_TOP, ICON_BIG, ICON_SMALL, ICONINFO, LR_DEFAULTCOLOR, LWA_ALPHA, MSG, NONCLIENTMETRICSW, SC_CLOSE, SM_CXEDGE,
    SM_CXVSCROLL, SPI_GETNONCLIENTMETRICS, SW_SHOW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, SendMessageW, SetForegroundWindow, SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW,
    SetWindowPos, SetWindowTextW, ShowWindow, WM_COMMAND, WM_GETFONT, WM_GETICON, WM_KEYDOWN, WM_KEYUP, WM_SETFONT,
    WM_SETICON, WM_SYSCOMMAND, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

use windows_sys::Win32::System::SystemServices::{SS_CENTER, SS_LEFT, SS_NOPREFIX, SS_RIGHT, SS_TYPEMASK};

use crate::registry::{self, Scale, Shared, TBM_GETPOS, WindowInfo, outer_size, pixels, scale};
use crate::runtime::{self, HOST_CLASS, WINDOW_CLASS, wide};
use crate::tweak::TweakFn;

/// How the backend is set up. Tests hide their windows, record commands
/// and keep the clipboard to themselves.
#[derive(Clone, Debug)]
pub struct BackendOptions {
    pub show_windows: bool,
    pub record_commands: bool,
    pub private_clipboard: bool,
}

impl Default for BackendOptions {
    fn default() -> Self {
        BackendOptions { show_windows: true, record_commands: false, private_clipboard: false }
    }
}

struct Node {
    kind: WidgetKind,
    hwnd: HWND,
    /// Every prop the core sent: what the control can't show or give back
    /// is reported from here.
    props: Vec<Prop>,
    frame: Rect,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    /// Checkboxes: whether it's checked underneath, while it shows mixed.
    checked: bool,
    /// Selects: the selection field's height and its item's, as the font
    /// makes them, in pixels.
    field: (i32, i32),
    /// The DPI its font was made for.
    dpi: u32,
}

impl Node {
    /// A system control, rather than a host of ours (a container, or a
    /// widget that isn't native yet).
    fn is_control(&self) -> bool {
        matches!(
            self.kind,
            WidgetKind::Text
                | WidgetKind::Button
                | WidgetKind::Checkbox
                | WidgetKind::Slider
                | WidgetKind::Select
                | WidgetKind::Progress
        )
    }

    fn takes_focus(&self) -> bool {
        matches!(self.kind, WidgetKind::Button | WidgetKind::Checkbox | WidgetKind::Slider | WidgetKind::Select)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct FontKey {
    dpi: u32,
    style: TextStyle,
    weight: Option<FontWeight>,
    italic: bool,
}

struct State {
    nodes: HashMap<NodeId, Node>,
    shared: Rc<Shared>,
    options: BackendOptions,
    log: Vec<Command>,
    pending_show: Vec<NodeId>,
    /// Fonts live as long as the thread: controls keep using them.
    fonts: HashMap<FontKey, HFONT>,
    icon: Option<HICON>,
}

/// The backend. Hand it to [`Ui::new`](mitsuami_core::Ui::new); keep a
/// [`Win32Handle`] for the run loop and tests.
pub struct Win32Backend {
    state: Rc<RefCell<State>>,
}

/// Shared access to a [`Win32Backend`] after it was moved into a `Ui`.
#[derive(Clone)]
pub struct Win32Handle {
    state: Rc<RefCell<State>>,
}

fn violation(command: &Command, problem: &str) -> ! {
    panic!("win32 backend: protocol violation in {command:?}: {problem}")
}

impl Win32Backend {
    pub fn new(options: BackendOptions) -> Win32Backend {
        runtime::init();
        let shared = Rc::new(Shared::default());
        let state = Rc::new(RefCell::new(State {
            nodes: HashMap::new(),
            shared: shared.clone(),
            options,
            log: Vec::new(),
            pending_show: Vec::new(),
            fonts: HashMap::new(),
            icon: None,
        }));
        // A window moved to a screen with another DPI: its controls get
        // fonts and places at the new one. Skipped if the backend is busy,
        // which a DPI change from the system never finds it.
        let weak = Rc::downgrade(&state);
        *shared.rescale.borrow_mut() = Some(Rc::new(move |window| {
            if let Some(state) = weak.upgrade()
                && let Ok(mut state) = state.try_borrow_mut()
            {
                state.rescale(window);
            }
        }));
        Win32Backend { state }
    }

    pub fn handle(&self) -> Win32Handle {
        Win32Handle { state: self.state.clone() }
    }
}

impl Win32Handle {
    /// Shows the windows made since the last call: after the first tick,
    /// so they're laid out when they appear.
    pub fn show_pending_windows(&self) {
        let (show, hwnds) = {
            let mut state = self.state.borrow_mut();
            let pending = std::mem::take(&mut state.pending_show);
            let hwnds: Vec<HWND> = pending.iter().filter_map(|id| state.nodes.get(id)).map(|n| n.hwnd).collect();
            (state.options.show_windows, hwnds)
        };
        for hwnd in hwnds {
            unsafe {
                if show {
                    ShowWindow(hwnd, SW_SHOW);
                    SetForegroundWindow(hwnd);
                } else {
                    // Alive, but invisible and click-through.
                    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                }
            }
        }
    }

    pub fn take_command_log(&self) -> Vec<Command> {
        std::mem::take(&mut self.state.borrow_mut().log)
    }

    pub fn node_count(&self) -> usize {
        self.state.borrow().nodes.len()
    }
}

impl mitsuami_core::TestHooks for Win32Handle {
    fn name(&self) -> &'static str {
        "win32"
    }

    /// As a drag of the window's border: no smaller than its minimum, and
    /// no taller or shorter while its content sets its height.
    fn resize_window(&self, window: NodeId, size: Size) {
        let state = self.state.borrow();
        let Some(info) = state.shared.window(window) else { return };
        let size = match (info.lock_height.get(), info.reported.get()) {
            (true, Some(now)) => Size::new(size.width, now.height),
            _ => size,
        };
        state.resize(window, size);
    }

    /// The close button's `SC_CLOSE`, which ends in `WM_CLOSE`.
    fn close_window(&self, window: NodeId) {
        let hwnd = self.state.borrow().shared.hwnd(window);
        if let Some(hwnd) = hwnd {
            unsafe { SendMessageW(hwnd, WM_SYSCOMMAND, SC_CLOSE as usize, 0) };
        }
    }

    fn take_command_log(&self) -> Vec<Command> {
        Win32Handle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        Win32Handle::node_count(self)
    }

    /// The process's AppUserModelID and the window's icon. Windows keeps
    /// no name for the app.
    fn app_info(&self, window: NodeId) -> NativeAppInfo {
        let mut id = std::ptr::null_mut();
        let id = unsafe { GetCurrentProcessExplicitAppUserModelID(&mut id) >= 0 && !id.is_null() }.then(|| unsafe {
            let len = (0..).take_while(|i| *id.add(*i) != 0).count();
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(id, len));
            CoTaskMemFree(id.cast());
            text
        });
        let hwnd = self.state.borrow().shared.hwnd(window);
        let icon = hwnd
            .map(|hwnd| unsafe { SendMessageW(hwnd, WM_GETICON, ICON_BIG as usize, 0) })
            .filter(|icon| *icon != 0)
            .and_then(|icon| icon_size(icon as HICON));
        NativeAppInfo { id, name: None, icon: icon.map(|(width, height)| NativeIcon::Image { width, height }) }
    }

    /// Posted messages (a window shown, a close) are dispatched; the rest
    /// was synchronous.
    fn settle(&self) {
        self.show_pending_windows();
        runtime::pump();
    }
}

fn icon_size(icon: HICON) -> Option<(u32, u32)> {
    unsafe {
        let mut info: ICONINFO = std::mem::zeroed();
        if GetIconInfo(icon, &mut info) == 0 {
            return None;
        }
        let mut bitmap: BITMAP = std::mem::zeroed();
        let bits = if info.hbmColor.is_null() { info.hbmMask } else { info.hbmColor };
        let read = GetObjectW(bits, size_of::<BITMAP>() as i32, (&mut bitmap as *mut BITMAP).cast());
        DeleteObject(info.hbmColor);
        DeleteObject(info.hbmMask);
        (read != 0).then_some((bitmap.bmWidth as u32, bitmap.bmHeight as u32))
    }
}

// ------------------------------------------------------------------ fonts

/// The message font (what dialogs and message boxes use) at `dpi`.
fn message_font(dpi: u32) -> windows_sys::Win32::Graphics::Gdi::LOGFONTW {
    unsafe {
        let mut metrics: NONCLIENTMETRICSW = std::mem::zeroed();
        metrics.cbSize = size_of::<NONCLIENTMETRICSW>() as u32;
        SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            (&mut metrics as *mut NONCLIENTMETRICSW).cast(),
            0,
            dpi,
        );
        metrics.lfMessageFont
    }
}

/// Win32 has no type ramp: sizes relative to the message font, in the
/// proportions Windows' own dialogs use for their headings.
fn style_ratio(style: TextStyle) -> f32 {
    match style {
        TextStyle::LargeTitle => 2.0,
        TextStyle::Title => 1.5,
        TextStyle::Headline => 1.25,
        TextStyle::Caption => 0.9,
        TextStyle::Body | TextStyle::Callout | TextStyle::Monospace => 1.0,
    }
}

fn font_sizes() -> FontSizes {
    let body = message_font(96).lfHeight.unsigned_abs() as f32;
    let size = |style| (body * style_ratio(style)).round();
    FontSizes {
        large_title: size(TextStyle::LargeTitle),
        title: size(TextStyle::Title),
        headline: size(TextStyle::Headline),
        body,
        callout: body,
        caption: size(TextStyle::Caption),
        monospace: body,
    }
}

fn weight_value(weight: FontWeight) -> i32 {
    match weight {
        FontWeight::Regular => 400,
        FontWeight::Medium => 500,
        FontWeight::Semibold => 600,
        FontWeight::Bold => 700,
    }
}

impl State {
    fn font(&mut self, key: FontKey) -> HFONT {
        *self.fonts.entry(key).or_insert_with(|| {
            let mut font = message_font(key.dpi);
            font.lfHeight = (font.lfHeight as f32 * style_ratio(key.style)).round() as i32;
            if key.style == TextStyle::Headline {
                font.lfWeight = FW_SEMIBOLD as i32;
            }
            if key.style == TextStyle::Monospace {
                font.lfFaceName = [0; 32];
                for (slot, c) in font.lfFaceName.iter_mut().zip("Consolas".encode_utf16()) {
                    *slot = c;
                }
            }
            if let Some(weight) = key.weight {
                font.lfWeight = weight_value(weight);
            } else if font.lfWeight == 0 {
                font.lfWeight = FW_NORMAL as i32;
            }
            font.lfItalic = key.italic as u8;
            unsafe { CreateFontIndirectW(&font) }
        })
    }

    /// Gives the node the font for its DPI and props.
    fn set_font(&mut self, id: NodeId) {
        let node = &self.nodes[&id];
        if !node.is_control() {
            return;
        }
        let dpi = unsafe { GetDpiForWindow(node.hwnd) }.max(96);
        let key = if node.kind == WidgetKind::Text {
            FontKey {
                dpi,
                style: find_prop!(node.props, TextStyle).unwrap_or(TextStyle::Body),
                weight: find_prop!(node.props, FontWeight),
                italic: find_prop!(node.props, Italic).unwrap_or(false),
            }
        } else {
            FontKey { dpi, style: TextStyle::Body, weight: None, italic: false }
        };
        let font = self.font(key);
        let node = self.nodes.get_mut(&id).unwrap();
        node.dpi = dpi;
        unsafe { SendMessageW(node.hwnd, WM_SETFONT, font as usize, 1) };
        // A drop-down list's field is as high as its font makes it.
        if node.kind == WidgetKind::Select {
            let mut rect = RECT::default();
            unsafe { GetWindowRect(node.hwnd, &mut rect) };
            let item = unsafe { SendMessageW(node.hwnd, CB_GETITEMHEIGHT, usize::MAX, 0) } as i32;
            node.field = (rect.bottom - rect.top, item);
        }
    }

    /// A window moved to another DPI: fonts and places at the new one.
    fn rescale(&mut self, window: NodeId) {
        let Some(root) = self.nodes.get(&window).map(|n| n.hwnd) else { return };
        let ids: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.kind != WidgetKind::Window && unsafe { GetAncestor(n.hwnd, GA_ROOT) } == root)
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            self.set_font(id);
            self.place(id);
        }
    }
}

// --------------------------------------------------------------- measuring

/// Runs `f` with a DC that has the control's font.
fn with_dc<T>(hwnd: HWND, f: impl FnOnce(HDC) -> T) -> T {
    unsafe {
        let dc = GetDC(hwnd);
        let font = SendMessageW(hwnd, WM_GETFONT, 0, 0);
        let old = SelectObject(dc, font as _);
        let result = f(dc);
        SelectObject(dc, old);
        ReleaseDC(hwnd, dc);
        result
    }
}

/// Dialog base units: the average character width and the line height,
/// which Windows' layout guidelines measure controls in (dialog units:
/// a quarter of the one, an eighth of the other).
fn base_units(dc: HDC) -> (i32, i32) {
    const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
    unsafe {
        let mut metrics: TEXTMETRICW = std::mem::zeroed();
        GetTextMetricsW(dc, &mut metrics);
        let text = wide(ALPHABET);
        let mut size = SIZE::default();
        GetTextExtentPoint32W(dc, text.as_ptr(), 52, &mut size);
        ((size.cx / 26 + 1) / 2, metrics.tmHeight)
    }
}

struct Dlu(i32, i32);

impl Dlu {
    fn x(&self, n: i32) -> i32 {
        (n * self.0 + 2) / 4
    }
    fn y(&self, n: i32) -> i32 {
        (n * self.1 + 4) / 8
    }
}

fn text_width(dc: HDC, text: &str) -> i32 {
    let text: Vec<u16> = text.encode_utf16().collect();
    let mut size = SIZE::default();
    unsafe { GetTextExtentPoint32W(dc, text.as_ptr(), text.len() as i32, &mut size) };
    size.cx
}

fn ideal_size(hwnd: HWND) -> SIZE {
    let mut size = SIZE::default();
    unsafe { SendMessageW(hwnd, BCM_GETIDEALSIZE, 0, (&mut size as *mut SIZE) as isize) };
    size
}

thread_local! {
    /// A two-state checkbox out of sight, to measure three-state ones by.
    static PROBE: std::cell::Cell<HWND> = const { std::cell::Cell::new(std::ptr::null_mut()) };
}

/// A checkbox's ideal size. `BCM_GETIDEALSIZE` has none for a three-state
/// one (mixed), which is as large as a two-state one with its text.
fn checkbox_size(hwnd: HWND) -> SIZE {
    if style(hwnd) & BS_TYPEMASK as u32 != BS_AUTO3STATE as u32 {
        return ideal_size(hwnd);
    }
    let probe = PROBE.get();
    let probe = if probe.is_null() {
        let probe = create_control(WidgetKind::Checkbox);
        unsafe { ShowWindow(probe, 0) };
        PROBE.set(probe);
        probe
    } else {
        probe
    };
    unsafe { SendMessageW(probe, WM_SETFONT, SendMessageW(hwnd, WM_GETFONT, 0, 0) as usize, 0) };
    set_window_text(probe, &window_text(hwnd));
    ideal_size(probe)
}

fn window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        let mut buffer = vec![0u16; len as usize + 1];
        let read = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        String::from_utf16_lossy(&buffer[..read.max(0) as usize])
    }
}

fn set_window_text(hwnd: HWND, text: &str) {
    unsafe { SetWindowTextW(hwnd, wide(text).as_ptr()) };
}

/// Button captions take `&` as a mnemonic's mark; ours are literal.
fn caption(label: &str) -> String {
    label.replace('&', "&&")
}

fn uncaption(text: &str) -> String {
    text.replace("&&", "&")
}

fn style(hwnd: HWND) -> u32 {
    unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 }
}

fn set_style(hwnd: HWND, mask: u32, bits: u32) {
    let old = style(hwnd);
    let new = (old & !mask) | bits;
    if new != old {
        unsafe {
            SetWindowLongPtrW(hwnd, GWL_STYLE, new as isize);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | 0x0020, // SWP_FRAMECHANGED
            );
            InvalidateRect(hwnd, std::ptr::null(), 1);
        }
    }
}

fn combo_options(hwnd: HWND) -> Vec<String> {
    let count = unsafe { SendMessageW(hwnd, CB_GETCOUNT, 0, 0) }.max(0) as usize;
    (0..count)
        .map(|i| unsafe {
            let len = SendMessageW(hwnd, CB_GETLBTEXTLEN, i, 0).max(0) as usize;
            let mut buffer = vec![0u16; len + 1];
            SendMessageW(hwnd, CB_GETLBTEXT, i, buffer.as_mut_ptr() as isize);
            String::from_utf16_lossy(&buffer[..len])
        })
        .collect()
}

/// A frame's pixels, edges rounded, so frames that meet still meet.
fn pixel_rect(frame: Rect, scale: f32) -> RECT {
    RECT {
        left: pixels(frame.x(), scale),
        top: pixels(frame.origin.y, scale),
        right: pixels(frame.x() + frame.width(), scale),
        bottom: pixels(frame.origin.y + frame.size.height, scale),
    }
}

// ------------------------------------------------------------------ nodes

impl State {
    fn create(&mut self, id: NodeId, kind: WidgetKind, props: &[Prop], command: &Command) {
        if self.nodes.contains_key(&id) {
            violation(command, "node already exists");
        }
        if kind == WidgetKind::Fragment {
            violation(command, "fragments are core-only");
        }
        let hwnd = if kind == WidgetKind::Window { self.create_window(id) } else { create_control(kind) };
        registry::register(hwnd, &self.shared, id, kind);
        let node = Node {
            kind,
            hwnd,
            props: Vec::new(),
            frame: Rect::ZERO,
            parent: None,
            children: Vec::new(),
            checked: false,
            field: (0, 0),
            dpi: 0,
        };
        let focusable = node.takes_focus();
        self.nodes.insert(id, node);
        if focusable {
            registry::subclass(hwnd);
        }
        if kind == WidgetKind::Slider {
            self.shared.sliders.borrow_mut().insert(id, Scale::new(0.0, 1.0, None, false));
        }
        self.set_font(id);
        for prop in props {
            self.set_prop(id, prop);
        }
        self.run_tweak(id);
    }

    fn create_window(&mut self, id: NodeId) -> HWND {
        let hidden = !self.options.show_windows;
        let ex_style = if hidden { WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW } else { 0 };
        let class = wide(WINDOW_CLASS);
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                class.as_ptr(),
                wide("").as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if hidden {
            // Alpha 1, not 0: a fully transparent window isn't drawn at
            // all, and captures would come out empty.
            unsafe { SetLayeredWindowAttributes(hwnd, 0, 1, LWA_ALPHA) };
        }
        if let Some(icon) = self.icon {
            set_icon(hwnd, icon);
        }
        let info = WindowInfo { hwnd: hwnd as isize, ..WindowInfo::default() };
        self.shared.windows.borrow_mut().insert(id, Rc::new(info));
        self.pending_show.push(id);
        hwnd
    }

    fn node(&self, id: NodeId, command: &Command) -> &Node {
        self.nodes.get(&id).unwrap_or_else(|| violation(command, &format!("node {id} does not exist")))
    }

    fn set_prop(&mut self, id: NodeId, prop: &Prop) {
        let Some(node) = self.nodes.get_mut(&id) else { return };
        node.props.retain(|p| p.key() != prop.key());
        node.props.push(prop.clone());
        let (hwnd, kind) = (node.hwnd, node.kind);
        match (prop, kind) {
            (Prop::Title(title), WidgetKind::Window) => set_window_text(hwnd, title),
            (Prop::MinSize(min), WidgetKind::Window) => {
                let Some(info) = self.shared.window(id) else { return };
                info.min.set(Some(*min));
                // A window smaller than its new minimum grows to it.
                if let Some(now) = info.reported.get().or(info.requested.get())
                    && (now.width < min.width || now.height < min.height)
                {
                    self.resize(id, now);
                }
            }
            (Prop::HeightFollowsContent(on), WidgetKind::Window) => {
                if let Some(info) = self.shared.window(id) {
                    info.lock_height.set(*on);
                }
            }
            (Prop::Text(text), WidgetKind::Text) => set_window_text(hwnd, text),
            (Prop::TextStyle(_) | Prop::FontWeight(_) | Prop::Italic(_), WidgetKind::Text) => self.set_font(id),
            (Prop::TextAlign(align), WidgetKind::Text) => {
                let bits = match align {
                    HorizontalAlign::Center => SS_CENTER,
                    HorizontalAlign::Right => SS_RIGHT,
                    _ => SS_LEFT,
                };
                set_style(hwnd, SS_TYPEMASK, bits);
            }
            (Prop::Label(label), WidgetKind::Button | WidgetKind::Checkbox) => set_window_text(hwnd, &caption(label)),
            (Prop::Enabled(on), _) if self.nodes[&id].is_control() => unsafe {
                EnableWindow(hwnd, *on as i32);
            },
            (Prop::ButtonRole(role), WidgetKind::Button) => {
                self.shared.roles.borrow_mut().insert(id, *role);
                // The default button shows as one: Return presses it.
                let bits = if *role == ButtonRole::Default { BS_DEFPUSHBUTTON } else { BS_PUSHBUTTON };
                set_style(hwnd, BS_TYPEMASK as u32, bits as u32);
            }
            (Prop::Checked(checked), WidgetKind::Checkbox) => {
                let node = self.nodes.get_mut(&id).unwrap();
                node.checked = *checked;
                if !shows_mixed(hwnd) {
                    let state = if *checked { BST_CHECKED } else { BST_UNCHECKED };
                    unsafe { SendMessageW(hwnd, BM_SETCHECK, state as usize, 0) };
                }
            }
            // Three states while mixed, two otherwise: Windows' own
            // three-state box would come back to mixed on later clicks.
            (Prop::Mixed(mixed), WidgetKind::Checkbox) => {
                let shown = shows_mixed(hwnd);
                if *mixed && !shown {
                    let node = self.nodes.get_mut(&id).unwrap();
                    node.checked = unsafe { SendMessageW(hwnd, BM_GETCHECK, 0, 0) } == BST_CHECKED as isize;
                    set_style(hwnd, BS_TYPEMASK as u32, BS_AUTO3STATE as u32);
                    unsafe { SendMessageW(hwnd, BM_SETCHECK, BST_INDETERMINATE as usize, 0) };
                } else if !*mixed && shown {
                    let checked = self.nodes[&id].checked;
                    set_style(hwnd, BS_TYPEMASK as u32, BS_AUTOCHECKBOX as u32);
                    let state = if checked { BST_CHECKED } else { BST_UNCHECKED };
                    unsafe { SendMessageW(hwnd, BM_SETCHECK, state as usize, 0) };
                }
            }
            (Prop::Range { .. } | Prop::Step(_) | Prop::Orientation(_), WidgetKind::Slider) => self.set_scale(id),
            (Prop::Number(number), WidgetKind::Slider) => {
                let pos = self.shared.sliders.borrow().get(&id).map(|s| s.pos(*number));
                if let Some(pos) = pos {
                    registry::set_slider(&self.shared, id, hwnd, pos);
                }
            }
            // The chosen option stays if it's still one, else the first.
            (Prop::Options(options), WidgetKind::Select) => unsafe {
                let chosen = SendMessageW(hwnd, CB_GETCURSEL, 0, 0);
                SendMessageW(hwnd, CB_RESETCONTENT, 0, 0);
                for option in options {
                    SendMessageW(hwnd, CB_ADDSTRING, 0, wide(option).as_ptr() as isize);
                }
                let chosen = if chosen >= 0 && (chosen as usize) < options.len() {
                    chosen
                } else if options.is_empty() {
                    -1
                } else {
                    0
                };
                SendMessageW(hwnd, CB_SETCURSEL, chosen as usize, 0);
            },
            (Prop::SelectedIndex(index), WidgetKind::Select) => unsafe {
                SendMessageW(hwnd, CB_SETCURSEL, index.map_or(usize::MAX, |i| i), 0);
            },
            // Marquee while there's nothing to show, a bar in thousandths
            // otherwise.
            (Prop::Progress(progress), WidgetKind::Progress) => unsafe {
                match progress {
                    Some(value) => {
                        SendMessageW(hwnd, PBM_SETMARQUEE, 0, 0);
                        set_style(hwnd, PBS_MARQUEE, 0);
                        SendMessageW(hwnd, PBM_SETRANGE32, 0, 1000);
                        SendMessageW(hwnd, PBM_SETPOS, (value.clamp(0.0, 1.0) * 1000.0).round() as usize, 0);
                    }
                    None => {
                        set_style(hwnd, PBS_MARQUEE, PBS_MARQUEE);
                        SendMessageW(hwnd, PBM_SETMARQUEE, 1, 0);
                    }
                }
            },
            _ => {}
        }
    }

    /// A slider's positions for its range, step and orientation, with a
    /// tick mark at each step where it has one, as Windows' stepped
    /// sliders show them.
    fn set_scale(&mut self, id: NodeId) {
        let node = &self.nodes[&id];
        let hwnd = node.hwnd;
        let (min, max) = node
            .props
            .iter()
            .find_map(|p| match p {
                Prop::Range { min, max } => Some((*min, *max)),
                _ => None,
            })
            .unwrap_or((0.0, 1.0));
        let step = find_prop!(node.props, Step).flatten();
        let vertical = find_prop!(node.props, Orientation) == Some(Orientation::Vertical);
        let old = self.shared.sliders.borrow().get(&id).copied();
        let value = old.map(|s| s.value(unsafe { SendMessageW(hwnd, TBM_GETPOS, 0, 0) }));
        let scale = Scale::new(min, max, step, vertical);
        self.shared.sliders.borrow_mut().insert(id, scale);
        let ticks = if step.is_some() { TBS_AUTOTICKS } else { TBS_NOTICKS };
        let vert = if vertical { TBS_VERT } else { 0 };
        set_style(hwnd, TBS_VERT | TBS_AUTOTICKS | TBS_NOTICKS, vert | ticks);
        unsafe {
            SendMessageW(hwnd, TBM_SETRANGEMIN, 0, 0);
            SendMessageW(hwnd, TBM_SETRANGEMAX, 1, scale.count as isize);
            SendMessageW(hwnd, TBM_SETTICFREQ, 1, 0);
        }
        // Where it was, in the new positions; the core sends `Number` next.
        if let Some(value) = value {
            registry::set_slider(&self.shared, id, hwnd, scale.pos(value));
        }
    }

    fn run_tweak(&self, id: NodeId) {
        let node = &self.nodes[&id];
        if let Some(Prop::Tweak(tweak)) = node.props.iter().find(|p| matches!(p, Prop::Tweak(_)))
            && let Some(run) = tweak.downcast_ref::<TweakFn>()
        {
            run(node.hwnd);
        }
    }

    /// Places a node's window at its frame, in pixels at its DPI.
    fn place(&mut self, id: NodeId) {
        let node = &self.nodes[&id];
        if node.kind == WidgetKind::Window {
            return;
        }
        if unsafe { GetDpiForWindow(node.hwnd) } != node.dpi {
            self.set_font(id);
        }
        let node = &self.nodes[&id];
        let r = pixel_rect(node.frame, scale(node.hwnd));
        unsafe {
            SetWindowPos(
                node.hwnd,
                std::ptr::null_mut(),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        // A drop-down list's height is its field's, which its item height
        // sets; the layout may make it taller than the font does.
        if node.kind == WidgetKind::Select {
            let (field, item) = node.field;
            let want = r.bottom - r.top;
            if want > 0 && item > 0 {
                let item = (item + want - field).max(1);
                unsafe { SendMessageW(node.hwnd, CB_SETITEMHEIGHT, usize::MAX, item as isize) };
            }
        }
    }

    /// Gives a window this content size, no smaller than its minimum.
    fn resize(&self, window: NodeId, size: Size) {
        let (Some(info), Some(hwnd)) = (self.shared.window(window), self.shared.hwnd(window)) else { return };
        let min = registry::screen_bound(hwnd, info.min.get().unwrap_or(Size::ZERO));
        let size = Size::new(size.width.max(min.width), size.height.max(min.height));
        info.requested.set(Some(size));
        let s = scale(hwnd);
        let (w, h) = outer_size(hwnd, pixels(size.width, s), pixels(size.height, s));
        // The height lock would keep the old height.
        let locked = info.lock_height.replace(false);
        unsafe { SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE) };
        info.lock_height.set(locked);
        self.shared.report_size(window, &info);
    }

    fn apply(&mut self, command: &Command) {
        match command {
            Command::Create { id, kind, props } => self.create(*id, *kind, props, command),
            Command::SetProp { id, prop } => {
                self.node(*id, command);
                self.set_prop(*id, prop);
                self.run_tweak(*id);
            }
            Command::Insert { parent, child, index } => {
                if let Some(p) = self.node(*child, command).parent {
                    violation(command, &format!("child is still attached to {p}"));
                }
                let parent_node = self.node(*parent, command);
                if *index > parent_node.children.len() {
                    violation(command, &format!("index out of bounds (len {})", parent_node.children.len()));
                }
                let parent_hwnd = parent_node.hwnd;
                let after = if *index == 0 { HWND_TOP } else { self.nodes[&parent_node.children[index - 1]].hwnd };
                let hwnd = self.nodes[child].hwnd;
                unsafe {
                    SetParent(hwnd, parent_hwnd);
                    // Child windows are in z-order: the core's order.
                    SetWindowPos(hwnd, after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
                self.nodes.get_mut(parent).unwrap().children.insert(*index, *child);
                self.nodes.get_mut(child).unwrap().parent = Some(*parent);
                self.place(*child);
            }
            Command::Remove { parent, child } => {
                let siblings =
                    &mut self.nodes.get_mut(parent).unwrap_or_else(|| violation(command, "no parent")).children;
                let Some(pos) = siblings.iter().position(|c| c == child) else {
                    violation(command, "not a child of this parent");
                };
                siblings.remove(pos);
                let node = self.nodes.get_mut(child).unwrap_or_else(|| violation(command, "no child"));
                node.parent = None;
                unsafe { SetParent(node.hwnd, runtime::parking()) };
            }
            Command::Destroy { id } => {
                let node = self.nodes.remove(id).unwrap_or_else(|| violation(command, "node does not exist"));
                if let Some(parent) = node.parent.and_then(|p| self.nodes.get_mut(&p)) {
                    parent.children.retain(|c| c != id);
                }
                // Children still in it go on living.
                for child in &node.children {
                    if let Some(child) = self.nodes.get_mut(child) {
                        child.parent = None;
                        unsafe { SetParent(child.hwnd, runtime::parking()) };
                    }
                }
                registry::unregister(node.hwnd);
                if self.shared.focus.get() == Some(*id) {
                    self.shared.focus.set(None);
                }
                self.shared.sliders.borrow_mut().remove(id);
                self.shared.roles.borrow_mut().remove(id);
                self.shared.windows.borrow_mut().remove(id);
                self.pending_show.retain(|w| w != id);
                unsafe { DestroyWindow(node.hwnd) };
            }
            Command::SetFrame { id, frame } => {
                let node = self.nodes.get_mut(id).unwrap_or_else(|| violation(command, "node does not exist"));
                if node.kind == WidgetKind::Window {
                    violation(command, "window frames belong to the platform");
                }
                node.frame = *frame;
                self.place(*id);
            }
            Command::SetA11y { id, .. } => {
                self.node(*id, command);
            }
            Command::SetWindowSize { id, size } => {
                self.node(*id, command);
                self.resize(*id, *size);
            }
            Command::SetFocusOrder { window, order } => {
                if self.node(*window, command).kind != WidgetKind::Window {
                    violation(command, "not a window");
                }
                if let Some(info) = self.shared.window(*window) {
                    *info.order.borrow_mut() = order.clone();
                }
            }
            // Nothing scrolls yet: it's where it was asked to be.
            Command::ScrollTo { id, offset } => {
                self.node(*id, command);
                self.shared.emit(*id, UiEvent::Scrolled(*offset));
            }
            Command::ScrollToRow { id, .. } => {
                self.node(*id, command);
            }
            Command::Focus { id } => {
                let hwnd = self.node(*id, command).hwnd;
                unsafe { SetFocus(hwnd) };
            }
        }
    }
}

/// A checkbox that shows the mixed state.
fn shows_mixed(hwnd: HWND) -> bool {
    unsafe { SendMessageW(hwnd, BM_GETCHECK, 0, 0) == BST_INDETERMINATE as isize }
}

/// A system control (or a host of ours), out of any tree for now.
fn create_control(kind: WidgetKind) -> HWND {
    let (class, style): (&str, u32) = match kind {
        WidgetKind::Text => ("STATIC", SS_LEFT | SS_NOPREFIX),
        WidgetKind::Button => ("BUTTON", WS_TABSTOP | BS_PUSHBUTTON as u32),
        WidgetKind::Checkbox => ("BUTTON", WS_TABSTOP | BS_AUTOCHECKBOX as u32),
        WidgetKind::Slider => ("msctls_trackbar32", WS_TABSTOP | TBS_NOTICKS),
        WidgetKind::Select => ("COMBOBOX", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32),
        WidgetKind::Progress => ("msctls_progress32", 0),
        _ => (HOST_CLASS, WS_CLIPCHILDREN),
    };
    let class = wide(class);
    unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            wide("").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | style,
            0,
            0,
            0,
            0,
            runtime::parking(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    }
}

fn set_icon(hwnd: HWND, icon: HICON) {
    unsafe {
        SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, icon as isize);
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, icon as isize);
    }
}

impl Backend for Win32Backend {
    fn init(&mut self, events: EventSink) {
        *self.state.borrow().shared.events.borrow_mut() = events;
    }

    /// Spacing from Windows' layout guidelines (in dialog units: 4 between
    /// related controls, 7 from a dialog's edge), at 96 DPI; classic
    /// controls have no dark mode.
    fn metrics(&self) -> PlatformMetrics {
        let dpi = unsafe { GetDpiForSystem() }.max(96);
        PlatformMetrics {
            scale_factor: dpi as f32 / 96.0,
            spacing: SpacingScale { xs: 4.0, sm: 7.0, md: 11.0, lg: 14.0, xl: 21.0 },
            font_sizes: font_sizes(),
            dark_mode: false,
            high_contrast: false,
            reduced_motion: false,
            tab_insets: Insets::new(24.0, 4.0, 4.0, 4.0),
            group_insets: Insets::new(8.0, 8.0, 8.0, 8.0),
            titled_group_insets: Insets::new(20.0, 8.0, 8.0, 8.0),
        }
    }

    fn apply(&mut self, batch: &[Command]) {
        let mut state = self.state.borrow_mut();
        for command in batch {
            if state.options.record_commands {
                state.log.push(command.clone());
            }
            state.apply(command);
        }
    }

    fn measure(&mut self, id: NodeId, request: MeasureRequest) -> Size {
        let state = self.state.borrow();
        let known = |natural: Size| {
            Size::new(request.known_width.unwrap_or(natural.width), request.known_height.unwrap_or(natural.height))
        };
        let Some(node) = state.nodes.get(&id) else { return known(Size::ZERO) };
        if !node.is_control() {
            return known(Size::ZERO);
        }
        let (hwnd, kind) = (node.hwnd, node.kind);
        let s = scale(hwnd);
        let (width, height) = with_dc(hwnd, |dc| {
            let dlu = {
                let (x, y) = base_units(dc);
                Dlu(x, y)
            };
            match kind {
                WidgetKind::Text => {
                    let text = find_prop!(node.props, Text).unwrap_or_default();
                    let wrap = request.known_width.or(match request.available_width {
                        AvailableSpace::Definite(w) => Some(w),
                        AvailableSpace::MinContent => Some(0.0),
                        AvailableSpace::MaxContent => None,
                    });
                    let line = {
                        let mut metrics: TEXTMETRICW = unsafe { std::mem::zeroed() };
                        unsafe { GetTextMetricsW(dc, &mut metrics) };
                        metrics.tmHeight
                    };
                    if text.is_empty() {
                        return (0, line);
                    }
                    let mut buffer: Vec<u16> = text.encode_utf16().collect();
                    let mut rect = RECT {
                        right: wrap.map_or(i32::MAX / 2, |w| ((w * s).floor() as i32).max(1)),
                        ..RECT::default()
                    };
                    let mut flags = DT_CALCRECT | DT_NOPREFIX | DT_EXPANDTABS;
                    if wrap.is_some() {
                        flags |= DT_WORDBREAK;
                    }
                    unsafe { DrawTextW(dc, buffer.as_mut_ptr(), buffer.len() as i32, &mut rect, flags) };
                    let lines = find_prop!(node.props, MaxLines).flatten();
                    let height = lines.map_or(rect.bottom, |n| rect.bottom.min(n as i32 * line));
                    (rect.right, height)
                }
                // Windows' guidelines: buttons at least 50 by 14 dialog
                // units, check boxes 10 high, sliders 15, drop-down lists
                // as their font makes them, progress bars 8.
                WidgetKind::Button => {
                    let ideal = ideal_size(hwnd);
                    (ideal.cx.max(dlu.x(50)), ideal.cy.max(dlu.y(14)))
                }
                WidgetKind::Checkbox => {
                    let ideal = checkbox_size(hwnd);
                    (ideal.cx, ideal.cy.max(dlu.y(10)))
                }
                WidgetKind::Slider => match find_prop!(node.props, Orientation).unwrap_or_default() {
                    Orientation::Horizontal => (dlu.x(107), dlu.y(15)),
                    Orientation::Vertical => (dlu.y(15), dlu.x(107)),
                },
                // As wide as its widest option, and its button.
                WidgetKind::Select => {
                    let widest = combo_options(hwnd).iter().map(|o| text_width(dc, o)).max().unwrap_or(0);
                    let dpi = unsafe { GetDpiForWindow(hwnd) };
                    let chrome = unsafe {
                        GetSystemMetricsForDpi(SM_CXVSCROLL, dpi) + 4 * GetSystemMetricsForDpi(SM_CXEDGE, dpi)
                    } + dlu.x(4);
                    (widest + chrome, node.field.0)
                }
                WidgetKind::Progress => (dlu.x(107), dlu.y(8)),
                _ => (0, 0),
            }
        });
        known(Size::new((width as f32 / s).ceil(), (height as f32 / s).ceil()))
    }

    fn perform(&mut self, id: NodeId, action: &A11yAction) -> Result<(), ActionError> {
        let (hwnd, kind, shared) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            let disabled = if node.is_control() {
                unsafe { IsWindowEnabled(node.hwnd) == 0 }
            } else {
                find_prop!(node.props, Enabled) == Some(false)
            };
            if disabled && *action != A11yAction::ScrollIntoView {
                return Err(ActionError::Disabled);
            }
            (node.hwnd, node.kind, state.shared.clone())
        };
        match (action, kind) {
            (A11yAction::Activate, WidgetKind::Button | WidgetKind::Checkbox) => click(hwnd),
            (
                A11yAction::Focus,
                WidgetKind::Button | WidgetKind::Checkbox | WidgetKind::Slider | WidgetKind::Select,
            ) => unsafe {
                SetFocus(hwnd);
            },
            // Its arrow keys, which step it by a position; up is larger,
            // on a vertical one too.
            (A11yAction::Increment | A11yAction::Decrement, WidgetKind::Slider) => {
                let vertical = style(hwnd) & TBS_VERT != 0;
                let key = match (action, vertical) {
                    (A11yAction::Increment, false) => VK_RIGHT,
                    (A11yAction::Increment, true) => VK_UP,
                    (_, false) => VK_LEFT,
                    (_, true) => VK_DOWN,
                };
                press(hwnd, key);
            }
            // As a drag would leave it, stopping on a position.
            (A11yAction::SetValue(text), WidgetKind::Slider) => {
                let value: f64 = text.trim().parse().map_err(|_| ActionError::Unsupported)?;
                let pos = shared.sliders.borrow().get(&id).map(|s| s.pos(value)).ok_or(ActionError::Unsupported)?;
                unsafe { SendMessageW(hwnd, TBM_SETPOSNOTIFY, 0, pos) };
            }
            // The first option with that text, as choosing it from the
            // list does, without opening it.
            (A11yAction::SetValue(text), WidgetKind::Select) => {
                let index = combo_options(hwnd).iter().position(|o| o == text).ok_or(ActionError::Unsupported)?;
                unsafe { SendMessageW(hwnd, CB_SETCURSEL, index, 0) };
                shared.emit(id, UiEvent::Changed(EventValue::Index(index)));
            }
            (A11yAction::ScrollIntoView, _) => {}
            _ => return Err(ActionError::Unsupported),
        }
        Ok(())
    }

    fn synthesize(&mut self, id: NodeId, input: &SyntheticInput) -> Result<(), ActionError> {
        let (hwnd, kind) = {
            let state = self.state.borrow();
            let node = state.nodes.get(&id).ok_or(ActionError::UnknownNode)?;
            if node.is_control() && unsafe { IsWindowEnabled(node.hwnd) == 0 } {
                return Err(ActionError::Disabled);
            }
            (node.hwnd, node.kind)
        };
        let SyntheticInput::Key(key) = input else { return Err(ActionError::Unsupported) };
        let vk = match (kind, key) {
            (WidgetKind::Button, Key::Enter) => VK_RETURN,
            (WidgetKind::Button | WidgetKind::Checkbox, Key::Char(' ')) => VK_SPACE,
            (WidgetKind::Button | WidgetKind::Checkbox | WidgetKind::Slider | WidgetKind::Select, Key::Tab) => VK_TAB,
            (WidgetKind::Slider, Key::Up) => VK_UP,
            (WidgetKind::Slider, Key::Down) => VK_DOWN,
            (WidgetKind::Slider, Key::Home) => VK_HOME,
            (WidgetKind::Slider, Key::End) => VK_END,
            (_, Key::Escape) => VK_ESCAPE,
            _ => return Err(ActionError::Unsupported),
        };
        // Keys go to the control with focus.
        unsafe {
            if GetFocus() != hwnd {
                SetFocus(hwnd);
            }
        }
        let msg = MSG { hwnd, message: WM_KEYDOWN, wParam: vk as usize, lParam: 1, ..unsafe { std::mem::zeroed() } };
        if !crate::keys::handle(&msg) {
            press(hwnd, vk);
        }
        Ok(())
    }

    fn native_state(&self, id: NodeId) -> Option<NativeState> {
        let state = self.state.borrow();
        let node = state.nodes.get(&id)?;
        let hwnd = node.hwnd;
        let mut props = node.props.clone();
        let mut show = |prop: Prop| {
            props.retain(|p| p.key() != prop.key());
            props.push(prop);
        };
        match node.kind {
            WidgetKind::Window => show(Prop::Title(window_text(hwnd))),
            WidgetKind::Text => show(Prop::Text(window_text(hwnd))),
            WidgetKind::Button => show(Prop::Label(uncaption(&window_text(hwnd)))),
            WidgetKind::Checkbox => {
                show(Prop::Label(uncaption(&window_text(hwnd))));
                let check = unsafe { SendMessageW(hwnd, BM_GETCHECK, 0, 0) };
                let mixed = check == BST_INDETERMINATE as isize;
                show(Prop::Checked(if mixed { node.checked } else { check == BST_CHECKED as isize }));
                if find_prop!(node.props, Mixed).is_some() {
                    show(Prop::Mixed(mixed));
                }
            }
            // Its range and step are the core's; its position is its own,
            // which the app's value is kept for while it maps there.
            WidgetKind::Slider => {
                let pos = unsafe { SendMessageW(hwnd, TBM_GETPOS, 0, 0) };
                if let Some(scale) = state.shared.sliders.borrow().get(&id) {
                    let number = find_prop!(node.props, Number).filter(|n| scale.pos(*n) == pos);
                    show(Prop::Number(number.unwrap_or_else(|| scale.value(pos))));
                }
                if find_prop!(node.props, Orientation).is_some() {
                    let vertical = style(hwnd) & TBS_VERT != 0;
                    show(Prop::Orientation(if vertical { Orientation::Vertical } else { Orientation::Horizontal }));
                }
            }
            WidgetKind::Select => {
                show(Prop::Options(combo_options(hwnd)));
                let chosen = unsafe { SendMessageW(hwnd, CB_GETCURSEL, 0, 0) };
                show(Prop::SelectedIndex(usize::try_from(chosen).ok()));
            }
            WidgetKind::Progress => {
                if style(hwnd) & PBS_MARQUEE != 0 {
                    show(Prop::Progress(None));
                } else {
                    let pos = unsafe { SendMessageW(hwnd, PBM_GETPOS, 0, 0) };
                    let kept = find_prop!(node.props, Progress)
                        .flatten()
                        .filter(|v| (v.clamp(0.0, 1.0) * 1000.0).round() as isize == pos);
                    show(Prop::Progress(Some(kept.unwrap_or(pos as f64 / 1000.0))));
                }
            }
            _ => {}
        }
        if node.is_control() {
            show(Prop::Enabled(unsafe { IsWindowEnabled(hwnd) != 0 }));
        }

        // Where the window is in its parent: the core's frame, while
        // that's what it is in pixels.
        let frame = if node.kind == WidgetKind::Window {
            Rect::ZERO
        } else {
            let mut r = RECT::default();
            unsafe {
                GetWindowRect(hwnd, &mut r);
                MapWindowPoints(std::ptr::null_mut(), GetParent(hwnd), (&mut r as *mut RECT).cast::<POINT>(), 2);
            }
            let s = scale(hwnd);
            let want = pixel_rect(node.frame, s);
            if (r.left, r.top, r.right, r.bottom) == (want.left, want.top, want.right, want.bottom) {
                node.frame
            } else {
                Rect::new(
                    r.left as f32 / s,
                    r.top as f32 / s,
                    (r.right - r.left) as f32 / s,
                    (r.bottom - r.top) as f32 / s,
                )
            }
        };
        // Its child windows, in z-order, that are nodes.
        let mut children = Vec::new();
        if !node.is_control() {
            let mut child = unsafe { GetWindow(hwnd, GW_CHILD) };
            while !child.is_null() {
                if let Some((_, child_id, _)) = registry::lookup(child) {
                    children.push(child_id);
                }
                child = unsafe { GetWindow(child, GW_HWNDNEXT) };
            }
        }
        let focused = node.kind != WidgetKind::Window && unsafe { GetFocus() } == hwnd;
        Some(NativeState {
            kind: node.kind,
            props,
            frame,
            parent: node.parent,
            children,
            focused,
            scroll_offset: node.kind.scrolls().then_some(Point::ZERO),
        })
    }

    /// The window's client area as it prints itself, cropped to the node.
    fn capture(&mut self, id: NodeId, reply: Reply<Result<Image, CaptureError>>) {
        let state = self.state.borrow();
        let Some(node) = state.nodes.get(&id) else { return reply(Err(CaptureError::UnknownNode)) };
        let window = unsafe { GetAncestor(node.hwnd, GA_ROOT) };
        let mut client = RECT::default();
        unsafe { GetClientRect(window, &mut client) };
        let mut area = client;
        if node.kind != WidgetKind::Window {
            unsafe {
                GetWindowRect(node.hwnd, &mut area);
                MapWindowPoints(std::ptr::null_mut(), window, (&mut area as *mut RECT).cast::<POINT>(), 2);
            }
        }
        let s = scale(window);
        drop(state);
        reply(print_window(window, client, area, s));
    }

    fn services(&self) -> Box<dyn mitsuami_core::services::Services> {
        let state = self.state.borrow();
        Box::new(crate::services::Win32Services::new(state.shared.clone(), state.options.private_clipboard))
    }

    /// The AppUserModelID groups the app's windows on the taskbar; each
    /// window gets the icon, as a Win32 app's get its class's.
    fn set_app_info(&mut self, info: &AppInfo) {
        if let Some(id) = &info.id {
            unsafe { SetCurrentProcessExplicitAppUserModelID(wide(id).as_ptr()) };
        }
        let Some(bytes) = info.icon.as_ref().and_then(|icon| icon.read()) else { return };
        // PNG bits make an icon as they are, as in an .ico file.
        let icon = unsafe {
            CreateIconFromResourceEx(bytes.as_ptr(), bytes.len() as u32, 1, 0x0003_0000, 0, 0, LR_DEFAULTCOLOR)
        };
        if icon.is_null() {
            return;
        }
        let mut state = self.state.borrow_mut();
        for node in state.nodes.values().filter(|n| n.kind == WidgetKind::Window) {
            set_icon(node.hwnd, icon);
        }
        if let Some(old) = state.icon.replace(icon) {
            unsafe { DestroyIcon(old) };
        }
    }
}

/// What a click on a button does, as the button does it: focus, an auto
/// checkbox's next state, and `BN_CLICKED` to its parent. `BM_CLICK` would
/// send it a real mouse press, which only clicks while it can capture the
/// mouse: in the foreground window, which a test window never is.
pub(crate) fn click(hwnd: HWND) {
    unsafe {
        SetFocus(hwnd);
        let kind = style(hwnd) & BS_TYPEMASK as u32;
        let check = SendMessageW(hwnd, BM_GETCHECK, 0, 0) as u32;
        // Unchecked, checked, then mixed for a three-state box.
        let next = match (kind as i32, check) {
            (BS_AUTOCHECKBOX, BST_CHECKED) | (BS_AUTO3STATE, BST_INDETERMINATE) => Some(BST_UNCHECKED),
            (BS_AUTOCHECKBOX, _) => Some(BST_CHECKED),
            (BS_AUTO3STATE, BST_CHECKED) => Some(BST_INDETERMINATE),
            (BS_AUTO3STATE, _) => Some(BST_CHECKED),
            _ => None,
        };
        if let Some(next) = next {
            SendMessageW(hwnd, BM_SETCHECK, next as usize, 0);
        }
        let id = GetWindowLongPtrW(hwnd, GWLP_ID) as usize & 0xffff;
        SendMessageW(GetParent(hwnd), WM_COMMAND, id | ((BN_CLICKED as usize) << 16), hwnd as isize);
    }
}

/// A key pressed and let go on the control, as the keyboard sends it.
fn press(hwnd: HWND, key: u16) {
    unsafe {
        SendMessageW(hwnd, WM_KEYDOWN, key as usize, 1);
        SendMessageW(hwnd, WM_KEYUP, key as usize, 0xC000_0001u32 as isize);
    }
}

fn print_window(window: HWND, client: RECT, area: RECT, scale: f32) -> Result<Image, CaptureError> {
    let (width, height) = (client.right - client.left, client.bottom - client.top);
    if width <= 0 || height <= 0 {
        return Err(CaptureError::Failed("the window has no size".into()));
    }
    unsafe {
        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);
        ReleaseDC(std::ptr::null_mut(), screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
        if bitmap.is_null() {
            DeleteDC(dc);
            return Err(CaptureError::Failed("no bitmap".into()));
        }
        let old = SelectObject(dc, bitmap);
        // PW_CLIENTONLY | PW_RENDERFULLCONTENT
        let printed = PrintWindow(window, dc, 1 | 2 as PRINT_WINDOW_FLAGS);
        let pixels = std::slice::from_raw_parts(bits as *const u8, (width * height * 4) as usize);
        let left = area.left.clamp(0, width);
        let top = area.top.clamp(0, height);
        let right = area.right.clamp(left, width);
        let bottom = area.bottom.clamp(top, height);
        let mut rgba = Vec::with_capacity(((right - left) * (bottom - top) * 4) as usize);
        for y in top..bottom {
            for x in left..right {
                let i = ((y * width + x) * 4) as usize;
                rgba.extend_from_slice(&[pixels[i + 2], pixels[i + 1], pixels[i], 255]);
            }
        }
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        if printed == 0 {
            return Err(CaptureError::Failed("PrintWindow failed".into()));
        }
        Ok(Image { width: (right - left) as u32, height: (bottom - top) as u32, scale_factor: scale, rgba })
    }
}
