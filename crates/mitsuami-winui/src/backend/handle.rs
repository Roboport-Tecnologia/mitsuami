//! The handle: shared access for services and tests, and the test hooks.

use std::rc::Rc;
use std::time::{Duration, Instant};

use mitsuami_core::services::MenuBarData;
use mitsuami_core::{Command, Modality, NativeAppInfo, NativeIcon, NodeId, Size};
use windows_core::Interface;

use super::focus::{report_focus, resolve, restore_focus};
use super::menus::refresh_menu;
use super::new_window::{icon_size, request_close};
use super::windows::{
    apply_full_screen, apply_maximized, apply_min_size, centre_on_work_area, resize_client, set_transparent,
};
use super::{State, Widget, WinUiHandle, WindowParts};
use crate::bindings as w;
use crate::runtime;

impl WinUiHandle {
    pub fn take_command_log(&self) -> Vec<Command> {
        std::mem::take(&mut self.state.borrow_mut().log)
    }

    /// Number of live native nodes.
    pub fn node_count(&self) -> usize {
        self.state.borrow().nodes.len()
    }

    /// Reports focus moves XAML made on its own (a focused control became
    /// disabled or went away) whose `GotFocus` hasn't arrived yet. XAML
    /// focuses parts of composite controls (a list's row container, a
    /// number box's text box): the node is the nearest one up from there.
    pub(crate) fn sync_focus(&self) {
        let state = self.state.borrow();
        for id in &state.windows {
            let Some(Widget::Window(parts)) = state.nodes.get(id).map(|n| &n.widget) else { continue };
            let element = parts
                .host
                .cast::<w::IUIElement>()
                .and_then(|h| h.XamlRoot())
                .and_then(|root| w::FocusManager::GetFocusedElementWithRoot(&root))
                .ok()
                .filter(|e| !e.as_raw().is_null());
            let now = resolve(&state.by_element, element)
                .filter(|id| !matches!(state.nodes.get(id).map(|n| &n.widget), Some(Widget::Window(_))));
            report_focus(&state.emitter, &parts.focus, now);
        }
    }

    /// Called after every event the platform reports, so the run loop turns.
    pub(crate) fn set_wake(&self, wake: impl Fn() + 'static) {
        *self.state.borrow().emitter.wake.borrow_mut() = Some(Rc::new(wake));
    }

    /// Resizes a window's content like the user would, keeping a locked
    /// height; the platform reports it back as `WindowResized`.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let state = self.state.borrow();
        if let Some(Widget::Window(parts)) = state.nodes.get(&window).map(|n| &n.widget) {
            // The user can't resize it.
            if !parts.resizable {
                return;
            }
            let height = parts.size.get().filter(|_| parts.height_locked).map_or(size.height, |s| s.height);
            resize_client(parts, Size::new(size.width, height));
        }
    }

    /// Makes visible the windows whose first layout has been applied,
    /// modal ones first made modal.
    pub fn show_pending_windows(&self) {
        self.state.borrow().focus_wanted();
        let pending = std::mem::take(&mut self.state.borrow_mut().pending_show);
        for id in pending {
            self.make_modal(id);
            let mut state = self.state.borrow_mut();
            let (by_element, emitter) = (state.by_element.clone(), state.emitter.clone());
            if let Some(Widget::Window(parts)) = state.nodes.get_mut(&id).map(|n| &mut n.widget) {
                // A dialog is centred on its owner (`make_modal`).
                if parts.modal.is_none() {
                    centre_on_work_area(parts);
                }
                set_transparent(parts.hwnd, false);
                unsafe { _ = w::SetForegroundWindow(parts.hwnd) };
                parts.shown = true;
                // Full screen, or maximized, asked for before it was shown.
                apply_full_screen(parts);
                apply_maximized(parts);
                // It was activated when made, before its content: its
                // controls can take focus now, unless one asked for still
                // can't.
                if parts.wanted_focus.borrow().is_none() {
                    restore_focus(&parts.root, &by_element, &parts.focus, &parts.tab_order.borrow(), &emitter);
                }
            }
        }
        // Windows moved to another display since.
        let state = self.state.borrow();
        for id in &state.windows {
            if let Some(Widget::Window(parts)) = state.nodes.get(id).map(|n| &n.widget)
                && parts.moved.replace(false)
            {
                apply_min_size(parts);
            }
        }
    }

    /// The Windows App SDK's way: owned by its window (`GWLP_HWNDPARENT`,
    /// how a WinUI 3 window gets an owner), centred on it, and `IsModal`,
    /// which disables the owner while it's shown; a dialog's presenter
    /// can't be minimized or maximized. Application-modal (or without an
    /// owner, where `IsModal` can't apply), it also disables the app's
    /// other windows, as Win32 apps do, until it closes.
    fn make_modal(&self, id: NodeId) {
        let mut state = self.state.borrow_mut();
        let Some(Widget::Window(parts)) = state.nodes.get(&id).map(|n| &n.widget) else { return };
        let Some((owner, modality)) = parts.modal else { return };
        let (hwnd, app_window) = (parts.hwnd, parts.app_window.clone());
        let owner = owner.and_then(|o| match state.nodes.get(&o).map(|n| &n.widget) {
            Some(Widget::Window(owner)) => Some((owner.hwnd, owner.app_window.clone())),
            _ => None,
        });
        let others: Vec<w::HWND> = state
            .nodes
            .values()
            .filter_map(|n| match &n.widget {
                Widget::Window(other) if other.hwnd != hwnd => Some(other.hwnd),
                _ => None,
            })
            .collect();
        let app = app_window.cast::<w::IAppWindow>().ok();
        if let Some((owner_hwnd, owner_window)) = &owner {
            unsafe { w::SetWindowLongPtrW(hwnd, w::GWLP_HWNDPARENT, *owner_hwnd as isize) };
            // Centred on its owner, as dialogs open.
            let owner_window = owner_window.cast::<w::IAppWindow>().ok();
            if let (Some(app), Some(owner_window)) = (&app, owner_window)
                && let (Ok(at), Ok(size), Ok(own)) = (owner_window.Position(), owner_window.Size(), app.Size())
            {
                _ = app.Move(w::PointInt32 {
                    x: at.x + (size.width - own.width) / 2,
                    y: at.y + (size.height - own.height) / 2,
                });
            }
        }
        if let Some(presenter) =
            app.and_then(|a| a.Presenter().ok()).and_then(|p| p.cast::<w::IOverlappedPresenter>().ok())
        {
            _ = presenter.SetIsMinimizable(false);
            _ = presenter.SetIsMaximizable(false);
            if owner.is_some() {
                _ = presenter.SetIsModal(true);
            }
        }
        let mut disabled = Vec::new();
        if modality == Modality::Application || owner.is_none() {
            for other in others {
                // Already disabled (by another modal window): not ours to enable.
                if unsafe { w::IsWindowEnabled(other) }.as_bool() {
                    unsafe { _ = w::EnableWindow(other, false.into()) };
                    disabled.push(other);
                }
            }
        }
        if let Some(Widget::Window(parts)) = state.nodes.get_mut(&id).map(|n| &mut n.widget) {
            parts.disabled = disabled;
        }
    }

    /// Escape hatch: the XAML window of a window node.
    pub fn xaml_window(&self, id: NodeId) -> Option<w::Window> {
        match &self.state.borrow().nodes.get(&id)?.widget {
            Widget::Window(parts) => Some(parts.window.clone()),
            _ => None,
        }
    }

    /// The window ids and XAML roots of live windows, for services.
    pub(crate) fn window_parts<T>(&self, id: Option<NodeId>, f: impl FnOnce(&WindowParts) -> T) -> Option<T> {
        let state = self.state.borrow();
        let parts = match id {
            Some(id) => match &state.nodes.get(&id)?.widget {
                Widget::Window(parts) => Some(&**parts),
                _ => None,
            },
            None => None,
        };
        // No (or an unknown) parent: the active window, else one a modal
        // window doesn't block, focused if one is, else any window. Every
        // window keeps its focused control while inactive, so focus alone
        // picked a modal window's owner, which it blocks.
        let parts = parts.or_else(|| {
            let windows: Vec<&WindowParts> = state
                .nodes
                .values()
                .filter_map(|n| match &n.widget {
                    Widget::Window(parts) => Some(&**parts),
                    _ => None,
                })
                .collect();
            let active = unsafe { w::GetActiveWindow() };
            let enabled = |p: &&WindowParts| unsafe { w::IsWindowEnabled(p.hwnd) }.as_bool();
            windows
                .iter()
                .find(|p| p.hwnd == active)
                .or_else(|| windows.iter().find(|p| enabled(p) && p.focus.get().is_some()))
                .or_else(|| windows.iter().find(|p| enabled(p)))
                .or(windows.first())
                .copied()
        })?;
        Some(f(parts))
    }

    pub(crate) fn xaml_root(&self, id: Option<NodeId>) -> Option<w::XamlRoot> {
        self.window_parts(id, |parts| parts.host.cast::<w::IUIElement>().ok()?.XamlRoot().ok()).flatten()
    }

    /// Installs the app's menus (`None`), shown in every window, or a
    /// window's own, shown in it with the app's. A window's menus may come
    /// before the window does: it gets them when it's created.
    pub(crate) fn set_menu(&self, window: Option<NodeId>, menu: &MenuBarData, activate: Rc<dyn Fn(u32)>) {
        let mut state = self.state.borrow_mut();
        let State { nodes, menus, .. } = &mut *state;
        menus.activate = Some(activate);
        match window {
            None => menus.app = menu.clone(),
            Some(window) if menu.menus.is_empty() => _ = menus.windows.remove(&window),
            Some(window) => _ = menus.windows.insert(window, menu.clone()),
        }
        for (id, node) in nodes.iter_mut() {
            if let Widget::Window(parts) = &mut node.widget
                && window.is_none_or(|w| w == *id)
            {
                refresh_menu(parts, menus);
            }
        }
    }
}

impl mitsuami_core::TestHooks for WinUiHandle {
    fn name(&self) -> &'static str {
        "winui"
    }

    fn dragged_files(&self, node: NodeId) -> Option<Vec<std::path::PathBuf>> {
        let state = self.state.borrow();
        let host = state.nodes.get(&node)?;
        match &state.nodes.get(&host.parent?)?.widget {
            Widget::List(list) => list.dragged_files(host.row?),
            _ => None,
        }
    }

    fn resize_window(&self, window: NodeId, size: Size) {
        WinUiHandle::resize_window(self, window, size);
    }

    /// `WM_CLOSE`, which the close button and Alt+F4 end in: the app
    /// window raises `Closing`. `Window.Close()` wouldn't: it closes
    /// without asking.
    fn close_window(&self, window: NodeId) {
        let hwnd = match self.state.borrow().nodes.get(&window).map(|n| &n.widget) {
            Some(Widget::Window(parts)) => parts.hwnd,
            _ => return,
        };
        request_close(hwnd);
        runtime::pump();
    }

    fn take_command_log(&self) -> Vec<Command> {
        WinUiHandle::take_command_log(self)
    }

    fn node_count(&self) -> usize {
        WinUiHandle::node_count(self)
    }

    /// The process's AppUserModelID and the icon the window's title bar
    /// and taskbar button show. Windows keeps no name for the app.
    fn app_info(&self, window: NodeId) -> NativeAppInfo {
        let mut id = windows_core::PWSTR::null();
        let id = unsafe { w::GetCurrentProcessExplicitAppUserModelID(&mut id) }.is_ok().then(|| unsafe {
            let text = id.to_string().ok();
            w::CoTaskMemFree(id.0.cast());
            text
        });
        let hwnd = match self.state.borrow().nodes.get(&window).map(|n| &n.widget) {
            Some(Widget::Window(parts)) => Some(parts.hwnd),
            _ => None,
        };
        let icon = hwnd
            .map(|hwnd| unsafe { w::SendMessageW(hwnd, w::WM_GETICON as u32, w::ICON_BIG as usize, 0) })
            .filter(|icon| *icon != 0)
            .and_then(|icon| icon_size(icon as w::HICON));
        NativeAppInfo {
            id: id.flatten(),
            name: None,
            icon: icon.map(|(width, height)| NativeIcon::Image { width, height }),
        }
    }

    /// XAML reports some changes (text edits, scrolling, focus) after the
    /// call that caused them: dispatch them.
    fn settle(&self) {
        runtime::pump();
        // Spinners have no size until XAML loads them, at its next frame,
        // and images from files until XAML has decoded them, in the
        // background: wait for both, so tests see the size the app gets.
        // Files' icons wait for the shell's image, for captures.
        // A window's content too is laid out at its new size only at
        // XAML's next frame (a new window's at the size it opened at, wider
        // than the one set), and the toolbar places items from its edge.
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.state.borrow().nodes.values().any(|n| match &n.widget {
            Widget::Spinner(ring) => !ring.cast::<w::IFrameworkElement>().and_then(|f| f.IsLoaded()).unwrap_or(true),
            Widget::Image { bitmap: Some(_), failed, opened, .. } => !failed.get() && !opened.get(),
            // Files' icons until the shell's image is shown.
            Widget::FileIcon { asked, shown, .. } => shown.get() != asked.get(),
            Widget::Window(parts) => parts.size.get().is_some_and(|size| {
                let Ok(host) = parts.host.cast::<w::IFrameworkElement>() else { return false };
                let (width, height) = (host.ActualWidth().unwrap_or(0.0), host.ActualHeight().unwrap_or(0.0));
                (width - size.width as f64).abs() > 1.0 || (height - size.height as f64).abs() > 1.0
            }),
            _ => false,
        }) && Instant::now() < deadline
        {
            runtime::wait(Some(Duration::from_millis(5)));
            runtime::pump();
        }
        // Every list: XAML may have laid out any of them meanwhile.
        let state = self.state.borrow();
        state.layout_lists(&state.lists.iter().copied().collect::<Vec<_>>());
        drop(state);
        // GPU surfaces follow their frames at XAML's next frame, which a
        // settle doesn't wait for: place them now, so the app has the size.
        for node in self.state.borrow().nodes.values() {
            if let Widget::GpuSurface(surface) = &node.widget {
                surface.place_now();
            }
        }
        self.sync_focus();
        // MITSUAMI_SHOW_WINDOWS=1: tests have no run loop to show them.
        self.show_pending_windows();
    }
}
