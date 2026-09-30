//! Making a window: its XAML root, its icon, and Escape for dialogs.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use mitsuami_core::backend::Appearance;
use mitsuami_core::services::MenuBarData;
use mitsuami_core::{AppIcon, Modality, NodeId, Size, UiEvent};
use windows_core::{EventRevoker, Interface};

use super::fields::text_area_probe;
use super::focus::{report_focus, resolve, restore_focus, tab};
use super::menus::refresh_menu;
use super::windows::{
    WINDOW_ROOT, clip_to_size, correct_client, in_full_screen, is_maximized, report_size, set_transparent,
};
use super::{MenuItems, R, State, Widget, WindowIcon, WindowParts};
use crate::bindings as w;
use crate::runtime;

/// `WM_CLOSE`, which the close button and Alt+F4 end in: the app window
/// raises `Closing`, which asks the app.
/// Whether the app runs from a package (MSIX), which has its own id and
/// icon.
pub(super) fn packaged() -> bool {
    let mut length = 0u32;
    unsafe { w::GetCurrentPackageFullName(&mut length, windows_core::PWSTR::null()) != w::APPMODEL_ERROR_NO_PACKAGE }
}

/// An `.ico` file as it is, or else the image (PNG, which icons may hold)
/// as one icon of its own size.
pub(super) fn window_icon(icon: &AppIcon) -> Option<WindowIcon> {
    if let AppIcon::File(path) = icon
        && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("ico"))
    {
        return Some(WindowIcon::File(path.to_string_lossy().into_owned()));
    }
    let bytes = icon.read()?;
    let icon = unsafe {
        w::CreateIconFromResourceEx(
            bytes.as_ptr(),
            bytes.len() as u32,
            true.into(),
            0x0003_0000,
            0,
            0,
            w::LR_DEFAULTCOLOR as u32,
        )
    };
    (!icon.is_null()).then_some(WindowIcon::Handle(icon))
}

pub(super) fn set_icon(app_window: &w::AppWindow, icon: &WindowIcon) -> R<()> {
    let app_window = app_window.cast::<w::IAppWindow>()?;
    match icon {
        WindowIcon::File(path) => app_window.SetIcon(path),
        // An `IconId` is the icon's handle, as a `WindowId` is a window's.
        WindowIcon::Handle(icon) => app_window.SetIconWithIconId(w::IconId { value: *icon as u64 }),
    }
}

/// An icon's size in pixels. A monochrome icon's mask holds its image
/// and its mask, one above the other.
pub(super) fn icon_size(icon: w::HICON) -> Option<(u32, u32)> {
    let mut info = w::ICONINFO::default();
    if !unsafe { w::GetIconInfo(icon, &mut info) }.as_bool() {
        return None;
    }
    let colour = !info.hbmColor.is_null();
    let mut bitmap = w::BITMAP::default();
    let read = unsafe {
        w::GetObjectW(
            if colour { info.hbmColor } else { info.hbmMask },
            size_of::<w::BITMAP>() as i32,
            (&raw mut bitmap).cast(),
        )
    };
    unsafe {
        _ = w::DeleteObject(info.hbmColor);
        _ = w::DeleteObject(info.hbmMask);
    }
    let height = if colour { bitmap.bmHeight } else { bitmap.bmHeight / 2 };
    (read != 0).then_some((bitmap.bmWidth.max(0) as u32, height.max(0) as u32))
}

pub(super) fn request_close(hwnd: w::HWND) {
    unsafe { _ = w::PostMessageW(hwnd, w::WM_CLOSE as u32, 0, 0) };
}

/// What a Win32 dialog does with Escape (`IDCANCEL`): it asks to close, as
/// the close button does. An accelerator only fires when the focused
/// control didn't use the key, so an open drop-down still closes itself.
pub(super) fn escape_closes(root: &w::Grid, hwnd: w::HWND) -> R<EventRevoker> {
    let accelerator = w::KeyboardAccelerator::new()?;
    let accel: w::IKeyboardAccelerator = accelerator.cast()?;
    accel.SetKey(w::VirtualKey::Escape)?;
    accel.SetModifiers(w::VirtualKeyModifiers::None)?;
    let revoker = accel.Invoked(move |_, args| {
        if let Some(args) = args.as_ref() {
            _ = args.cast::<w::IKeyboardAcceleratorInvokedEventArgs>().and_then(|a| a.SetHandled(true));
        }
        request_close(hwnd);
    })?;
    root.cast::<w::IUIElement>()?.KeyboardAccelerators()?.Append(&accelerator)?;
    Ok(revoker)
}

impl State {
    pub(super) fn create_window(
        &mut self,
        id: NodeId,
        modal: Option<(Option<NodeId>, Modality)>,
    ) -> R<(Widget, w::UIElement, Vec<EventRevoker>)> {
        let window = w::Window::new()?;
        // Markup, for the theme resources: they follow the element's theme
        // (tests force light) and switch live with the system's.
        let root: w::Grid = w::XamlReader::Load(WINDOW_ROOT)?.cast()?;
        let children = root.cast::<w::IPanel>()?.Children()?;
        let title_bar: w::TitleBar = children.GetAt(0)?.cast()?;
        let host: w::Canvas = children.GetAt(1)?.cast()?;
        let host_element: w::UIElement = host.cast()?;
        if self.right_to_left {
            super::windows::set_window_direction(&root, &host, true)?;
        }
        let text_probe = text_area_probe()?;
        children.Append(&text_probe.cast::<w::UIElement>()?)?;
        if let Some(appearance) = self.options.appearance {
            let theme = match appearance {
                Appearance::Light => w::ElementTheme::Light,
                Appearance::Dark => w::ElementTheme::Dark,
            };
            root.cast::<w::IFrameworkElement>()?.SetRequestedTheme(theme)?;
        }
        let iwindow: w::IWindow = window.cast()?;
        iwindow.SetContent(&root.cast::<w::UIElement>()?)?;
        // Fluent's title bar instead of the Win32 caption, which ignores the
        // app's theme. The system still draws the caption buttons: make them
        // tall enough for the TitleBar control and follow the theme.
        iwindow.SetExtendsContentIntoTitleBar(true)?;
        iwindow.SetTitleBar(&title_bar.cast::<w::UIElement>()?)?;

        let app_window = window.cast::<w::IWindow2>()?.AppWindow()?;
        let caption = app_window.cast::<w::IAppWindow>()?.TitleBar()?;
        caption.cast::<w::IAppWindowTitleBar2>()?.SetPreferredHeightOption(w::TitleBarHeightOption::Standard)?;
        caption.cast::<w::IAppWindowTitleBar3>()?.SetPreferredTheme(match self.options.appearance {
            Some(Appearance::Light) => w::TitleBarTheme::Light,
            Some(Appearance::Dark) => w::TitleBarTheme::Dark,
            None => w::TitleBarTheme::UseDefaultAppMode,
        })?;
        if let Some(icon) = &self.icon {
            _ = set_icon(&app_window, icon);
        }
        let window_id = app_window.cast::<w::IAppWindow>()?.Id()?;
        let hwnd = window_id.value as usize as w::HWND;
        crate::session::watch(hwnd);
        // Invisible until the first layout is applied (or for good, in
        // tests). XAML only measures elements in a live tree, so the window
        // must be activated before anything is measured.
        set_transparent(hwnd, true);
        if !self.options.show_windows {
            // Not moved offscreen: XAML stops rendering windows it considers
            // hidden, and capture needs rendering.
            app_window.cast::<w::IAppWindow>()?.SetIsShownInSwitchers(false)?;
        }

        let loaded = Rc::new(Cell::new(false));
        let mut revokers = Vec::new();
        revokers.push(host.cast::<w::IFrameworkElement>()?.Loaded({
            let loaded = loaded.clone();
            move |_, _| loaded.set(true)
        })?);
        window.cast::<w::IWindow>()?.Activate()?;
        let deadline = Instant::now() + Duration::from_secs(10);
        while !loaded.get() {
            assert!(Instant::now() < deadline, "winui backend: a window's content never loaded");
            runtime::pump_nested();
            if !loaded.get() {
                runtime::wait(Some(Duration::from_millis(5)));
            }
        }

        let emitter = self.emitter();
        revokers.push(app_window.cast::<w::IAppWindow>()?.Closing({
            let emitter = emitter.clone();
            move |_, args| {
                // The app decides; the core sends Destroy if it agrees.
                if let Some(args) = args.as_ref() {
                    _ = args.cast::<w::IAppWindowClosingEventArgs>().and_then(|a| a.SetCancel(true));
                }
                emitter.emit(id, UiEvent::WindowCloseRequested);
            }
        })?);
        let size = Rc::new(Cell::new(None::<Size>));
        let aimed = Rc::new(Cell::new(None::<(Size, Size)>));
        revokers.push(host.cast::<w::IFrameworkElement>()?.SizeChanged({
            let (emitter, last, aimed, app_window) = (emitter.clone(), size.clone(), aimed.clone(), app_window.clone());
            move |sender, args| {
                let Some(new) = args.as_ref().and_then(|a| a.cast::<w::ISizeChangedEventArgs>().ok()?.NewSize().ok())
                else {
                    return;
                };
                let host = sender.as_ref().and_then(|s| s.cast::<w::IUIElement>().ok());
                if let Some(host) = &host {
                    _ = clip_to_size(host, new);
                }
                let new = Size::new(new.width, new.height);
                if let Some((want, before)) = aimed.get()
                    && new != before
                {
                    aimed.set(None);
                    correct_client(&app_window, host.as_ref(), want, new);
                }
                report_size(&emitter, id, &last, new);
            }
        })?);
        let focus = Rc::new(Cell::new(None));
        let root_element: w::IUIElement = root.cast()?;
        revokers.push(root_element.GotFocus({
            let (emitter, by_element, focus) = (emitter.clone(), self.by_element.clone(), focus.clone());
            move |_, args| {
                let source = args.as_ref().and_then(|a| a.cast::<w::IRoutedEventArgs>().ok()?.OriginalSource().ok());
                report_focus(&emitter, &focus, resolve(&by_element, source));
            }
        })?);
        // A press on the title bar would reach XAML's root ScrollViewer,
        // which takes focus from the focused control; Windows' own title
        // bars leave focus where it is.
        revokers.push(title_bar.cast::<w::IUIElement>()?.PointerPressed(|_, args| {
            if let Some(args) = args.as_ref() {
                _ = args.cast::<w::IPointerRoutedEventArgs>().and_then(|a| a.SetHandled(true));
            }
        })?);
        // Full screen changed elsewhere (another part of the process): the
        // app hears of it. Our own changes match what it asked for.
        let full_screen = Rc::new(Cell::new(false));
        let maximized = Rc::new(Cell::new(false));
        let monitor = unsafe { w::MonitorFromWindow(hwnd, w::MONITOR_DEFAULTTONEAREST as u32) } as isize;
        let (monitor, moved) = (Rc::new(Cell::new(monitor)), Rc::new(Cell::new(false)));
        revokers.push(app_window.cast::<w::IAppWindow>()?.Changed({
            let (emitter, full_screen, title_bar) = (emitter.clone(), full_screen.clone(), title_bar.clone());
            let (monitor, moved, maximized) = (monitor.clone(), moved.clone(), maximized.clone());
            move |sender, args| {
                let args = args.as_ref().and_then(|a| a.cast::<w::IAppWindowChangedEventArgs>().ok());
                // Maximized or restored by the user (the caption's button, a
                // double-click on it, Win+Up), which resizes it; the app's own
                // match what it asked for. Full screen isn't maximized.
                if let (Some(true), Some(app_window)) =
                    (args.as_ref().and_then(|a| a.DidSizeChange().ok()), sender.as_ref())
                    && !in_full_screen(app_window)
                {
                    let now = is_maximized(app_window);
                    if maximized.replace(now) != now {
                        emitter.emit(id, UiEvent::MaximizedChanged(now));
                    }
                }
                // Onto another display: its minimum is applied again after
                // the next tick, where the backend's state is at hand.
                if args.as_ref().and_then(|a| a.DidPositionChange().ok()) == Some(true) {
                    let now = unsafe { w::MonitorFromWindow(hwnd, w::MONITOR_DEFAULTTONEAREST as u32) } as isize;
                    if monitor.replace(now) != now {
                        moved.set(true);
                        emitter.wake();
                    }
                }
                let changed = args.and_then(|a| a.DidPresenterChange().ok());
                let Some(app_window) = sender.as_ref().filter(|_| changed == Some(true)) else { return };
                let on = in_full_screen(app_window);
                let visibility = if on { w::Visibility::Collapsed } else { w::Visibility::Visible };
                _ = title_bar.cast::<w::IUIElement>().and_then(|e| e.SetVisibility(visibility));
                if full_screen.replace(on) != on {
                    emitter.emit(id, UiEvent::FullScreenChanged(on));
                }
            }
        })?);
        let tab_order = Rc::new(RefCell::new(Vec::new()));
        revokers.push(iwindow.Activated({
            let (emitter, by_element, focus, tab_order, root) =
                (emitter.clone(), self.by_element.clone(), focus.clone(), tab_order.clone(), root.clone());
            move |_, args| {
                let state = args
                    .as_ref()
                    .and_then(|a| a.cast::<w::IWindowActivatedEventArgs>().ok()?.WindowActivationState().ok());
                if state == Some(w::WindowActivationState::Deactivated) {
                    // GPU surfaces' pointer locks and keyboard grabs end
                    // when the window stops being the active one.
                    crate::surface::window_deactivated(hwnd);
                } else {
                    let (emitter, by_element, focus, tab_order, root) =
                        (emitter.clone(), by_element.clone(), focus.clone(), tab_order.clone(), root.clone());
                    let restore: Box<dyn FnOnce()> =
                        Box::new(move || restore_focus(&root, &by_element, &focus, &tab_order.borrow(), &emitter));
                    // After XAML's own restore, which follows `Activated`.
                    let ticket = crate::later::park(restore);
                    if let Ok(queue) = w::DispatcherQueue::GetForCurrentThread() {
                        crate::later::on_ui(&queue, move || {
                            if let Some(restore) = crate::later::take::<Box<dyn FnOnce()>>(ticket) {
                                restore();
                            }
                        });
                    }
                }
            }
        })?);
        revokers.push(root_element.PreviewKeyDown({
            let (emitter, by_element, focus, tab_order, root) =
                (emitter.clone(), self.by_element.clone(), focus.clone(), tab_order.clone(), root.clone());
            move |_, args| {
                let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()) else { return };
                if args.Key().ok() != Some(w::VirtualKey::Tab) {
                    return;
                }
                // A GPU surface that takes input takes Tab; Control+Tab
                // still moves on.
                if crate::surface::takes_tab(focus.get()) && unsafe { w::GetKeyState(w::VK_CONTROL) } >= 0 {
                    return;
                }
                let backwards = unsafe { w::GetKeyState(w::VK_SHIFT) } < 0;
                if let Some(next) =
                    tab(&root, &by_element, focus.get(), &tab_order.borrow(), backwards, w::FocusState::Keyboard)
                {
                    report_focus(&emitter, &focus, Some(next));
                    _ = args.SetHandled(true);
                }
            }
        })?);

        if self.options.show_windows {
            self.pending_show.push(id);
        }
        let mut parts = WindowParts {
            window,
            root,
            host,
            text_probe,
            title_bar,
            app_window,
            hwnd,
            id: window_id,
            menu_bar: None,
            menu_revokers: Vec::new(),
            menu_shown: MenuBarData::default(),
            menu_items: MenuItems::default(),
            requested: None,
            node: id,
            emitter,
            size,
            aimed,
            focus,
            tab_order,
            wanted_focus: RefCell::default(),
            // Known from its Create, so a dialog never gets the app's bar.
            modal,
            disabled: Vec::new(),
            escape: None,
            toolbar: None,
            toolbar_items: Vec::new(),
            sidebar: None,
            sidebar_revokers: Vec::new(),
            sidebar_place: None,
            full_screen,
            maximized,
            resizable: true,
            shown: false,
            overlapped: None,
            min_size: None,
            height_locked: false,
            moved,
        };
        refresh_menu(&mut parts, &self.menus);
        Ok((Widget::Window(Box::new(parts)), host_element, revokers))
    }
}
