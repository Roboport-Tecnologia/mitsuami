//! The keys a dialog manager (`IsDialogMessage`) handles, handled our way:
//! it can't reach controls in nested hosts, and Tab follows the order the
//! core sends rather than the windows' z-order.

use mitsuami_core::{ButtonRole, NodeId, WidgetKind};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, IsWindowEnabled, SetFocus, VK_ESCAPE, VK_RETURN, VK_SHIFT, VK_TAB,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CB_GETDROPPEDSTATE, GA_ROOT, GetAncestor, IsWindowVisible, MSG, SendMessageW, UIS_CLEAR, UISF_HIDEFOCUS,
    WM_CHANGEUISTATE, WM_KEYDOWN,
};

use crate::registry::{Shared, lookup_up};

/// Handles Tab, Shift+Tab, Return and Escape in our windows. Returns
/// whether the message was used.
pub(crate) fn handle(msg: &MSG) -> bool {
    if msg.message != WM_KEYDOWN {
        return false;
    }
    let key = msg.wParam as u16;
    if ![VK_TAB, VK_RETURN, VK_ESCAPE].contains(&key) {
        return false;
    }
    let Some((shared, id, kind, control)) = lookup_up(msg.hwnd) else { return false };
    let root = unsafe { GetAncestor(control, GA_ROOT) };
    let Some((_, window)) = shared.window_of_root(root) else { return false };
    // An open drop-down list takes its own keys.
    if kind == WidgetKind::Select && unsafe { SendMessageW(control, CB_GETDROPPEDSTATE, 0, 0) } != 0 {
        return false;
    }
    match key {
        VK_TAB => {
            let back = unsafe { GetKeyState(VK_SHIFT as i32) } < 0;
            let order = window.order.borrow().clone();
            if let Some(next) = next_in_order(&shared, &order, id, back) {
                unsafe {
                    SetFocus(next);
                    // Focus rectangles show once the keyboard is used.
                    SendMessageW(root, WM_CHANGEUISTATE, (UIS_CLEAR | (UISF_HIDEFOCUS << 16)) as usize, 0);
                }
            }
            true
        }
        // Return presses the focused button, else the window's default one.
        VK_RETURN if kind == WidgetKind::Button => {
            crate::backend::click(control);
            true
        }
        VK_RETURN => press_role(&shared, root, ButtonRole::Default),
        VK_ESCAPE => press_role(&shared, root, ButtonRole::Cancel),
        _ => false,
    }
}

/// The next control after `from` in the order that can take focus,
/// wrapping around; the first if `from` isn't in it.
fn next_in_order(shared: &Shared, order: &[NodeId], from: NodeId, back: bool) -> Option<HWND> {
    let len = order.len();
    let start = order.iter().position(|n| *n == from);
    (1..=len)
        .map(|step| match (start, back) {
            (Some(i), false) => order[(i + step) % len],
            (Some(i), true) => order[(i + len * 2 - step) % len],
            (None, false) => order[step - 1],
            (None, true) => order[len - step],
        })
        .filter_map(|id| shared.hwnd(id))
        .find(|hwnd| unsafe { IsWindowEnabled(*hwnd) != 0 && IsWindowVisible(*hwnd) != 0 })
}

fn press_role(shared: &Shared, root: HWND, role: ButtonRole) -> bool {
    let buttons: Vec<NodeId> = shared.roles.borrow().iter().filter(|(_, r)| **r == role).map(|(id, _)| *id).collect();
    let button = buttons.into_iter().filter_map(|id| shared.hwnd(id)).find(|hwnd| unsafe {
        GetAncestor(*hwnd, GA_ROOT) == root && IsWindowEnabled(*hwnd) != 0 && IsWindowVisible(*hwnd) != 0
    });
    match button {
        Some(button) => {
            crate::backend::click(button);
            true
        }
        None => false,
    }
}
