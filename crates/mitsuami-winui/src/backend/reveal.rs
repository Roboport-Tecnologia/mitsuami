//! A menu bar in the title bar, shown in full screen while the pointer is
//! at the top edge of the screen (`FullScreenMenuBar::AtTopEdge`).
//!
//! Full screen hides the title bar, and the menu bar with it. A
//! `GpuSurface` is a child window XAML can't draw over, so the menu bar is
//! shown in a popup that isn't kept to the window's bounds, which XAML
//! puts in a window of its own above it, as it does a menu's flyout. The
//! surface's child window takes the pointer's moves, so it tells where the
//! pointer is (`pointer_moved`); the window acts on it at its next tick.

use std::cell::RefCell;
use std::collections::HashMap;

use windows_core::Interface;

use super::windows::{TALL_TITLE_BAR, in_full_screen, scale_of, title_content};
use super::{FullScreenMenuBar, MenuBarPlace, R, WindowParts};
use crate::bindings as w;

thread_local! {
    /// The pointer's height on screen, in pixels, over a surface of each
    /// top-level window, as last moved and not yet acted on.
    static POINTER: RefCell<HashMap<isize, i32>> = RefCell::new(HashMap::new());
}

/// The popup: a window-wide strip on the title bar's background, as tall
/// as the tall title bar the menu bar was in.
const REVEAL: &str = r#"
<Popup xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
       ShouldConstrainToRootBounds="False">
  <Grid Background="{ThemeResource SolidBackgroundFillColorBaseBrush}"/>
</Popup>"#;

/// The pointer moved over a surface's child window, not captured: where it
/// is, for its window's next tick.
pub(crate) fn pointer_moved(surface: w::HWND) {
    let root = unsafe { w::GetAncestor(surface, w::GA_ROOT as u32) };
    let mut at = w::POINT::default();
    if root.is_null() || !unsafe { w::GetCursorPos(&mut at) }.as_bool() {
        return;
    }
    POINTER.with(|p| p.borrow_mut().insert(root as isize, at.y));
    crate::app::schedule_tick();
}

/// Shows or hides the menu bar for where the pointer went: shown at the
/// screen's top edge, hidden once the pointer is back below it with no
/// menu open. Hidden too when the window leaves full screen.
pub(super) fn update_reveal(parts: &mut WindowParts) {
    let moved = POINTER.with(|p| p.borrow_mut().remove(&(parts.hwnd as isize)));
    let wanted = parts.full_screen_menu_bar == FullScreenMenuBar::AtTopEdge
        && parts.menu_bar_place == MenuBarPlace::InTitleBar
        && parts.menu_bar.is_some()
        && in_full_screen(&parts.app_window);
    if !wanted {
        _ = hide_reveal(parts);
        return;
    }
    let Some(y) = moved else { return };
    let monitor = unsafe { w::MonitorFromWindow(parts.hwnd, w::MONITOR_DEFAULTTONEAREST as u32) };
    let mut info = w::MONITORINFO { cbSize: std::mem::size_of::<w::MONITORINFO>() as u32, ..Default::default() };
    if monitor.is_null() || !unsafe { w::GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return;
    }
    let top = info.rcMonitor.top;
    let below = top + (TALL_TITLE_BAR * scale_of(parts)).round() as i32;
    if y <= top {
        _ = show_reveal(parts);
    } else if y >= below && !menu_open(parts) {
        _ = hide_reveal(parts);
    }
}

/// Moves the menu bar into the popup, the window's width, and opens it.
fn show_reveal(parts: &mut WindowParts) -> R<()> {
    if parts.revealed {
        return Ok(());
    }
    let Some(menu_bar) = parts.menu_bar.clone() else { return Ok(()) };
    let popup = match &parts.reveal {
        Some(popup) => popup.clone(),
        None => {
            let popup: w::Popup = w::XamlReader::Load(REVEAL)?.cast()?;
            parts.reveal = Some(popup.clone());
            popup
        }
    };
    let root = parts.root.cast::<w::IUIElement>()?;
    popup.cast::<w::IUIElement>()?.SetXamlRoot(&root.XamlRoot()?)?;
    let strip = popup.Child()?;
    let strip_fe = strip.cast::<w::IFrameworkElement>()?;
    strip_fe.SetWidth(parts.root.cast::<w::IFrameworkElement>()?.ActualWidth()?)?;
    strip_fe.SetHeight(TALL_TITLE_BAR)?;
    let element: w::UIElement = menu_bar.cast()?;
    move_element(&element, &title_content(parts)?.cast()?, &strip.cast()?, None)?;
    popup.SetIsOpen(true)?;
    parts.revealed = true;
    Ok(())
}

/// Closes the popup and puts the menu bar back in the title bar, first.
pub(super) fn hide_reveal(parts: &mut WindowParts) -> R<()> {
    if !parts.revealed {
        return Ok(());
    }
    parts.revealed = false;
    let Some(popup) = parts.reveal.clone() else { return Ok(()) };
    popup.SetIsOpen(false)?;
    if let Some(menu_bar) = parts.menu_bar.clone() {
        let element: w::UIElement = menu_bar.cast()?;
        move_element(&element, &popup.Child()?.cast()?, &title_content(parts)?.cast()?, Some(0))?;
    }
    Ok(())
}

/// Moves an element from one panel to another, at `index` or the end.
fn move_element(element: &w::UIElement, from: &w::Panel, to: &w::Panel, index: Option<u32>) -> R<()> {
    let from = from.cast::<w::IPanel>()?.Children()?;
    let mut at = 0;
    if from.IndexOf(element, &mut at)? {
        from.RemoveAt(at)?;
    }
    let to = to.cast::<w::IPanel>()?.Children()?;
    match index {
        Some(index) => to.InsertAt(index, element),
        None => to.Append(element),
    }
}

/// A menu of the bar is open: a popup besides the strip's own.
fn menu_open(parts: &WindowParts) -> bool {
    let root = parts.root.cast::<w::IUIElement>().and_then(|r| r.XamlRoot());
    let open = root.and_then(|r| w::VisualTreeHelper::GetOpenPopupsForXamlRoot(&r)).and_then(|p| p.Size());
    open.is_ok_and(|n| n > u32::from(parts.revealed))
}
