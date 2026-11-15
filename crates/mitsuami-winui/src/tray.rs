//! The tray icon on Windows: a notification area icon (`Shell_NotifyIconW`,
//! `NOTIFYICON_VERSION_4`), which WinUI doesn't wrap. Its callbacks come to
//! a message-only window of its own. A click, left or right, opens its
//! menu, a Win32 popup menu (`TrackPopupMenuEx` after
//! `SetForegroundWindow`), as Windows 11's own tray menus are: a XAML
//! flyout would need a window to show in. The icon has a GUID made from
//! the app's id, so Windows keeps the user's choice to show it; for an
//! unsigned app Windows ties that GUID to the executable's path, and when
//! it's taken by another path the icon goes without one. Explorer
//! restarting (`TaskbarCreated`) adds it again. Windows doesn't recolor
//! tray icons for the taskbar's theme: an icon's dark variant shows on a
//! dark one, swapped when the user changes it, as Windows' own apps do.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami_core::AppIcon;
use mitsuami_core::services::{MenuCheck, MenuEntry, TrayData};
use windows_core::{GUID, HSTRING, PCWSTR, PWSTR};

use crate::bindings as w;

/// Where Windows keeps the taskbar's theme ("Choose your default Windows
/// mode"), which isn't the apps' one.
const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

/// The message the icon's callbacks come as.
const CALLBACK: u32 = w::WM_APP as u32 + 1;
/// A `TaskbarCreated` that came while the state was borrowed, posted to
/// be handled from the queue.
const RESTARTED: u32 = w::WM_APP as u32 + 2;
/// The same for a theme change (its `WM_SETTINGCHANGE` string doesn't
/// outlive the send).
const THEME_CHANGED: u32 = w::WM_APP as u32 + 3;
/// The timer that tries again to add an icon Explorer didn't take.
const RETRY: usize = 1;
const RETRY_MS: u32 = 5_000;
const CLASS: PCWSTR = windows_core::w!("MitsuamiTray");

type Activate = Rc<dyn Fn(u32)>;
type Shown = Rc<dyn Fn(bool)>;

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

struct State {
    window: w::HWND,
    /// `TaskbarCreated`, which Explorer sends when it starts again.
    taskbar_created: u32,
    data: TrayData,
    /// `AppInfo::icon`, for a tray icon that gives none.
    app_icon: Option<AppIcon>,
    guid: GUID,
    /// Whether Windows took the GUID: without, the icon goes by its id.
    with_guid: bool,
    icon: w::HICON,
    /// Whether `icon` is this app's to destroy, not the system's shared one.
    owned: bool,
    /// What `icon` was loaded from, so a change to the tooltip or menu
    /// doesn't load it again.
    loaded: Option<(Option<AppIcon>, bool)>,
    /// Whether the taskbar is dark, as the icon is loaded for.
    dark: bool,
    added: bool,
    /// Whether an add was tried since Explorer last started: one that
    /// timed out may have gone through, so a retry modifies first.
    tried: bool,
    activate: Activate,
    shown: Shown,
}

/// `Services::set_tray`.
pub(crate) fn set(tray: Option<&TrayData>, info: &mitsuami_core::AppInfo, activate: Activate, shown: Shown) {
    let Some(tray) = tray else {
        if let Some(state) = STATE.with(|s| s.borrow_mut().take()) {
            state.remove();
            (state.shown)(false);
        }
        return;
    };
    let exists = STATE.with(|s| s.borrow().is_some());
    if !exists {
        let Some(window) = message_window() else { return shown(false) };
        let taskbar_created = unsafe { w::RegisterWindowMessageW(windows_core::w!("TaskbarCreated")) };
        // An elevated app gets Explorer's broadcast only when it lets it
        // through: its integrity level is higher than Explorer's.
        unsafe {
            _ = w::ChangeWindowMessageFilterEx(window, taskbar_created, w::MSGFLT_ALLOW as u32, std::ptr::null_mut())
        };
        let exe = || std::env::current_exe().map(|exe| exe.display().to_string()).unwrap_or_default();
        let state = State {
            window,
            taskbar_created,
            data: tray.clone(),
            app_icon: None,
            guid: guid(&info.id.clone().unwrap_or_else(exe)),
            with_guid: true,
            icon: std::ptr::null_mut(),
            owned: false,
            loaded: None,
            dark: taskbar_dark(),
            added: false,
            tried: false,
            activate: activate.clone(),
            shown: shown.clone(),
        };
        STATE.with(|s| *s.borrow_mut() = Some(state));
    }
    let added = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let state = s.as_mut().expect("the tray's state");
        state.data = tray.clone();
        state.app_icon = info.icon.clone();
        state.activate = activate;
        state.shown = shown.clone();
        state.update();
        state.added
    });
    shown(added);
}

impl State {
    /// Adds the icon, or sends what changed.
    fn update(&mut self) {
        let source = (self.data.icon.clone().or_else(|| self.app_icon.clone()), self.dark);
        let mut old = None;
        if self.loaded.as_ref() != Some(&source) {
            let (icon, owned) = load_icon(source.0.as_ref(), self.dark);
            old = Some((std::mem::replace(&mut self.icon, icon), std::mem::replace(&mut self.owned, owned)));
            self.loaded = Some(source);
        }
        let mut data = self.notify_data();
        if self.added {
            unsafe { _ = w::Shell_NotifyIconW(w::NIM_MODIFY as u32, &data) };
        } else {
            if self.tried {
                self.added = unsafe { w::Shell_NotifyIconW(w::NIM_MODIFY as u32, &data) }.as_bool();
            }
            if !self.added {
                self.added = unsafe { w::Shell_NotifyIconW(w::NIM_ADD as u32, &data) }.as_bool();
            }
            self.tried = true;
            if !self.added && self.with_guid {
                // The GUID may belong to this app at another path. If the
                // icon can't be added without it either, the notification
                // area isn't there yet or is busy (a launch at login): the
                // timer below tries again, with the GUID.
                self.with_guid = false;
                data = self.notify_data();
                self.added = unsafe { w::Shell_NotifyIconW(w::NIM_ADD as u32, &data) }.as_bool();
                if !self.added {
                    self.with_guid = true;
                }
            }
            if self.added {
                data.Anonymous.uVersion = w::NOTIFYICON_VERSION_4 as u32;
                unsafe { _ = w::Shell_NotifyIconW(w::NIM_SETVERSION as u32, &data) };
            }
            // At login Explorer may be up but too busy to answer, with its
            // `TaskbarCreated` sent before this window was made: without a
            // retry the icon would never show that session.
            unsafe {
                if self.added {
                    _ = w::KillTimer(self.window, RETRY);
                } else {
                    w::SetTimer(self.window, RETRY, RETRY_MS, None);
                }
            }
        }
        if let Some((old, true)) = old
            && !old.is_null()
        {
            unsafe { _ = w::DestroyIcon(old) };
        }
    }

    fn notify_data(&self) -> w::NOTIFYICONDATAW {
        let mut data = w::NOTIFYICONDATAW {
            cbSize: size_of::<w::NOTIFYICONDATAW>() as u32,
            hWnd: self.window,
            uID: 1,
            uFlags: (w::NIF_MESSAGE | w::NIF_ICON | w::NIF_TIP | w::NIF_SHOWTIP) as u32,
            uCallbackMessage: CALLBACK,
            hIcon: self.icon,
            ..Default::default()
        };
        if self.with_guid {
            data.uFlags |= w::NIF_GUID as u32;
            data.guidItem = self.guid;
        }
        // The tooltip, else the title: it's what Narrator reads too.
        let tip = if self.data.tooltip.is_empty() { &self.data.title } else { &self.data.tooltip };
        for (slot, unit) in data.szTip.iter_mut().zip(tip.encode_utf16().take(127)) {
            *slot = unit;
        }
        data
    }

    fn remove(&self) {
        let data = self.notify_data();
        unsafe {
            if self.added {
                _ = w::Shell_NotifyIconW(w::NIM_DELETE as u32, &data);
            }
            if self.owned && !self.icon.is_null() {
                _ = w::DestroyIcon(self.icon);
            }
            _ = w::DestroyWindow(self.window);
        }
    }
}

/// A GUID made from the app's id, the same in every run: FNV-1a twice,
/// with the version and variant bits of a name-based one.
fn guid(id: &str) -> GUID {
    let fnv = |seed: u64| id.bytes().fold(seed, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3));
    let high = fnv(0xcbf2_9ce4_8422_2325);
    let low = fnv(0x8422_2325_cbf2_9ce4);
    let value = (u128::from(high) << 64 | u128::from(low)) & !(0xf000 << 64) | (0x5000 << 64);
    let value = value & !(0xc000 << 48) | (0x8000 << 48);
    GUID::from_u128(value)
}

/// The image in an `.ico` file's bytes (`include_bytes!` of one) that's
/// the size asked for, else the smallest larger, else the largest: what
/// `LoadImageW` picks from a file. `None` for bytes that aren't an `.ico`
/// (a PNG), which go to Windows as they are.
fn ico_image(bytes: &[u8], size: i32) -> Option<&[u8]> {
    let u16_at = |at: usize| Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    let u32_at = |at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    if u16_at(0)? != 0 || u16_at(2)? != 1 {
        return None;
    }
    let images = (0..usize::from(u16_at(4)?)).filter_map(|i| {
        let entry = 6 + i * 16;
        // A width of 0 is 256.
        let width = match *bytes.get(entry)? {
            0 => 256,
            width => i32::from(width),
        };
        let (length, offset) = (u32_at(entry + 8)? as usize, u32_at(entry + 12)? as usize);
        Some((width, bytes.get(offset..offset.checked_add(length)?)?))
    });
    let (larger, smaller): (Vec<_>, Vec<_>) = images.partition(|(width, _)| *width >= size);
    let best = larger
        .into_iter()
        .min_by_key(|(width, _)| *width)
        .or_else(|| smaller.into_iter().max_by_key(|(width, _)| *width));
    best.map(|(_, image)| image)
}

/// Whether the taskbar is dark. Windows 10 before 1903 had only a dark
/// one, and no value for it.
fn taskbar_dark() -> bool {
    let (key, value) = (HSTRING::from(PERSONALIZE), HSTRING::from("SystemUsesLightTheme"));
    let (mut data, mut size) = (0u32, size_of::<u32>() as u32);
    let read = unsafe {
        w::RegGetValueW(
            w::HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            w::RRF_RT_REG_DWORD as u32,
            std::ptr::null_mut(),
            (&raw mut data).cast(),
            &mut size,
        )
    };
    read != 0 || data == 0
}

/// Takes the tray icon away as the app ends, so it doesn't stay in the
/// notification area until the pointer passes over it: the core's removal
/// runs only while the `Ui` lives.
pub(crate) fn shutdown() {
    if let Some(state) = STATE.with(|s| s.borrow_mut().take()) {
        state.remove();
    }
}

/// The small icon's size for the system's DPI, for the taskbar's theme: an
/// `.ico` file at the size it has, else the image as an icon of its own.
/// Says whether the icon is the app's own to destroy: the fallback is the
/// system's shared one.
fn load_icon(icon: Option<&AppIcon>, dark: bool) -> (w::HICON, bool) {
    let dpi = unsafe { w::GetDpiForSystem() };
    let size = unsafe { w::GetSystemMetricsForDpi(w::SM_CXSMICON, dpi) };
    let loaded = match icon.and_then(|icon| icon.image_for(dark)) {
        Some(AppIcon::File(path)) if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ico")) => {
            let path = HSTRING::from(path.as_os_str());
            unsafe {
                w::LoadImageW(
                    std::ptr::null_mut(),
                    PCWSTR(path.as_ptr()),
                    w::IMAGE_ICON as u32,
                    size,
                    size,
                    w::LR_LOADFROMFILE as u32,
                )
            }
        }
        Some(icon) => match icon.read() {
            Some(bytes) => unsafe {
                let bytes = ico_image(&bytes, size).unwrap_or(&bytes);
                w::CreateIconFromResourceEx(
                    bytes.as_ptr(),
                    bytes.len() as u32,
                    true.into(),
                    0x0003_0000,
                    size,
                    size,
                    w::LR_DEFAULTCOLOR as u32,
                )
            },
            None => std::ptr::null_mut(),
        },
        None => std::ptr::null_mut(),
    };
    if loaded.is_null() {
        (unsafe { w::LoadIconW(std::ptr::null_mut(), w::IDI_APPLICATION) }, false)
    } else {
        (loaded, true)
    }
}

fn message_window() -> Option<w::HWND> {
    static REGISTER: std::sync::Once = std::sync::Once::new();
    REGISTER.call_once(|| unsafe {
        let class = w::WNDCLASSEXW {
            cbSize: size_of::<w::WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: w::GetModuleHandleW(PCWSTR::null()),
            lpszClassName: CLASS,
            ..Default::default()
        };
        w::RegisterClassExW(&class);
    });
    let hwnd = unsafe {
        w::CreateWindowExW(
            0,
            CLASS,
            PCWSTR::null(),
            0,
            0,
            0,
            0,
            0,
            // A top-level window, never shown: a message-only one gets no
            // broadcasts, and `TaskbarCreated` and `WM_SETTINGCHANGE` are.
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            w::GetModuleHandleW(PCWSTR::null()),
            std::ptr::null(),
        )
    };
    (!hwnd.is_null()).then_some(hwnd)
}

unsafe extern "system" fn window_proc(hwnd: w::HWND, message: u32, wparam: w::WPARAM, lparam: w::LPARAM) -> w::LRESULT {
    if message == CALLBACK {
        // Version 4: the event in the low word, the anchor in `wparam`.
        let event = (lparam & 0xffff) as u32;
        if event == w::NIN_SELECT as u32 || event == w::NIN_KEYSELECT as u32 || event == w::WM_CONTEXTMENU as u32 {
            let x = (wparam & 0xffff) as u16 as i16 as i32;
            let y = ((wparam >> 16) & 0xffff) as u16 as i16 as i32;
            show_menu(hwnd, x, y);
        }
        return 0;
    }
    // Nobody but the app closes it: a `WM_CLOSE` from outside (`taskkill`
    // without `/f`, an installer) would leave the app with no icon.
    if message == w::WM_CLOSE as u32 {
        return 0;
    }
    // The state is borrowed while `Shell_NotifyIconW` waits on Explorer,
    // and messages sent in the meantime (a theme change's broadcasts) come
    // here: a panic here would end the process, so those are posted again,
    // to be handled once the borrow is over.
    let restarted = message == RESTARTED
        || STATE.with(|s| s.try_borrow().is_ok_and(|s| s.as_ref().is_some_and(|st| message == st.taskbar_created)));
    if restarted || (message == w::WM_TIMER as u32 && wparam == RETRY) {
        let reported = STATE.with(|s| {
            let Ok(mut s) = s.try_borrow_mut() else {
                if restarted {
                    unsafe { _ = w::PostMessageW(hwnd, RESTARTED, 0, 0) };
                }
                return None;
            };
            let state = s.as_mut()?;
            if restarted {
                state.added = false;
                state.tried = false;
            } else if state.added {
                return None;
            }
            state.update();
            Some((state.added, state.shown.clone()))
        });
        if let Some((added, shown)) = reported {
            shown(added);
        }
        return 0;
    }
    let theme = message == THEME_CHANGED;
    if theme || message == w::WM_SETTINGCHANGE as u32 && setting_is(lparam, "ImmersiveColorSet") {
        // The theme changed; the taskbar's may have.
        let dark = taskbar_dark();
        STATE.with(|s| match s.try_borrow_mut() {
            Ok(mut s) => {
                if let Some(state) = s.as_mut()
                    && state.dark != dark
                {
                    state.dark = dark;
                    state.update();
                }
            }
            Err(_) => unsafe { _ = w::PostMessageW(hwnd, THEME_CHANGED, 0, 0) },
        });
        if theme {
            return 0;
        }
    }
    unsafe { w::DefWindowProcW(hwnd, message, wparam, lparam) }
}

/// Whether a `WM_SETTINGCHANGE` names `setting`: its `lParam` is the area
/// that changed, as a string, or null.
fn setting_is(lparam: w::LPARAM, setting: &str) -> bool {
    let name = lparam as *const u16;
    !name.is_null() && unsafe { PCWSTR(name).to_string() }.is_ok_and(|name| name == setting)
}

/// Opens the menu at the icon and reports the item chosen. Windows wants
/// the window in front first, or the menu doesn't close on a click
/// elsewhere, and a message after, so it closes at once next time.
fn show_menu(hwnd: w::HWND, x: i32, y: i32) {
    let Some((entries, activate)) =
        STATE.with(|s| s.try_borrow().ok()?.as_ref().map(|s| (s.data.menu.clone(), s.activate.clone())))
    else {
        return;
    };
    let mut titles = Vec::new();
    let menu = popup(&entries, &mut titles);
    let chosen = unsafe {
        _ = w::SetForegroundWindow(hwnd);
        let flags = w::TPM_RETURNCMD | w::TPM_RIGHTBUTTON | w::TPM_BOTTOMALIGN | w::TPM_RIGHTALIGN;
        let chosen = w::TrackPopupMenuEx(menu, flags as u32, x, y, hwnd, std::ptr::null());
        _ = w::PostMessageW(hwnd, w::WM_NULL as u32, 0, 0);
        _ = w::DestroyMenu(menu);
        chosen.0
    };
    drop(titles);
    if chosen > 0 {
        activate(chosen as u32);
    }
}

/// A popup menu of the entries. Titles stay in `titles` while it lives.
fn popup(entries: &[MenuEntry], titles: &mut Vec<Vec<u16>>) -> w::HMENU {
    let menu = unsafe { w::CreatePopupMenu() };
    for (position, entry) in entries.iter().enumerate() {
        let mut info = w::MENUITEMINFOW { cbSize: size_of::<w::MENUITEMINFOW>() as u32, ..Default::default() };
        let mut title = |text: &str| {
            // `&` marks an access key: a title's own is doubled.
            let mut wide: Vec<u16> = text.replace('&', "&&").encode_utf16().chain([0]).collect();
            let pointer = PWSTR(wide.as_mut_ptr());
            titles.push(wide);
            pointer
        };
        match entry {
            MenuEntry::Separator => {
                info.fMask = w::MIIM_FTYPE as u32;
                info.fType = w::MFT_SEPARATOR as u32;
            }
            MenuEntry::Item(item) => {
                info.fMask = (w::MIIM_ID | w::MIIM_STRING | w::MIIM_STATE | w::MIIM_FTYPE) as u32;
                info.wID = item.id;
                info.dwTypeData = title(&item.title);
                info.fType = w::MFT_STRING as u32;
                if !item.enabled {
                    info.fState |= w::MFS_DISABLED as u32;
                }
                match item.check {
                    MenuCheck::None => {}
                    MenuCheck::Check(on) => {
                        if on {
                            info.fState |= w::MFS_CHECKED as u32;
                        }
                    }
                    MenuCheck::Radio(on) => {
                        info.fType |= w::MFT_RADIOCHECK as u32;
                        if on {
                            info.fState |= w::MFS_CHECKED as u32;
                        }
                    }
                }
            }
            MenuEntry::Submenu(submenu) => {
                info.fMask = (w::MIIM_STRING | w::MIIM_SUBMENU | w::MIIM_FTYPE) as u32;
                info.fType = w::MFT_STRING as u32;
                info.dwTypeData = title(&submenu.title);
                info.hSubMenu = popup(&submenu.entries, titles);
            }
        }
        unsafe { _ = w::InsertMenuItemW(menu, position as u32, true.into(), &info) };
    }
    menu
}
