//! Raw Win32 settings for built-in widgets, past their semantic props.

use std::rc::Rc;

use mitsuami_core::reactive::{IntoValue, Value};
use mitsuami_core::{Opaque, Tweak};
use mitsuami_widgets::{Button, Checkbox, Progress, Select, Slider, Text};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_STYLE, GetWindowLongPtrW, SendMessageW};

/// A native control: its window, and what's commonly asked of one.
pub trait Control {
    fn hwnd(&self) -> HWND;

    /// `SendMessageW` to the control.
    fn send(&self, message: u32, wparam: usize, lparam: isize) -> isize {
        unsafe { SendMessageW(self.hwnd(), message, wparam, lparam) }
    }

    fn style(&self) -> u32 {
        unsafe { GetWindowLongPtrW(self.hwnd(), GWL_STYLE) as u32 }
    }

    fn is_enabled(&self) -> bool {
        unsafe { IsWindowEnabled(self.hwnd()) != 0 }
    }
}

macro_rules! controls {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(Clone, Copy, Debug)]
        pub struct $name(HWND);

        impl Control for $name {
            fn hwnd(&self) -> HWND {
                self.0
            }
        }

        impl From<HWND> for $name {
            fn from(hwnd: HWND) -> $name {
                $name(hwnd)
            }
        }
    )*};
}

controls! {
    /// A `BUTTON` with `BS_PUSHBUTTON` (or `BS_DEFPUSHBUTTON`).
    PushButton,
    /// A `BUTTON` with `BS_AUTOCHECKBOX` (`BS_AUTO3STATE` while mixed).
    CheckBox,
    /// A `msctls_trackbar32`.
    Trackbar,
    /// A `COMBOBOX` with `CBS_DROPDOWNLIST`.
    ComboBox,
    /// A `msctls_progress32`.
    ProgressBar,
    /// A `STATIC` label.
    Static,
}

/// A built-in widget, and the control that shows it.
pub trait Tweakable {
    type Native: Control + From<HWND>;
}

impl Tweakable for Button {
    type Native = PushButton;
}

impl Tweakable for Checkbox {
    type Native = CheckBox;
}

impl Tweakable for Slider {
    type Native = Trackbar;
}

impl Tweakable for Select {
    type Native = ComboBox;
}

impl Tweakable for Progress {
    type Native = ProgressBar;
}

impl Tweakable for Text {
    type Native = Static;
}

/// Settings as the backend runs them, on the node's window.
pub(crate) type TweakFn = Rc<dyn Fn(HWND)>;

/// Raw settings for a built-in widget's control:
/// `win32::tweak(|b: &PushButton| b.send(BCM_SETSHIELD, 0, 1))`. See
/// [`Tweak`].
pub fn tweak<W: Tweakable>(apply: impl Fn(&W::Native) + 'static) -> Tweak<W> {
    tweak_with(Value::Static(()), move |native, ()| apply(native))
}

/// Raw settings made from a value, applied again whenever it changes.
pub fn tweak_with<W: Tweakable, T: Clone + 'static>(
    value: impl IntoValue<T>,
    apply: impl Fn(&W::Native, &T) + 'static,
) -> Tweak<W> {
    let apply = Rc::new(apply);
    Tweak::new(value.into_value(), move |value| {
        let apply = apply.clone();
        let run: TweakFn = Rc::new(move |hwnd| apply(&W::Native::from(hwnd), &value));
        Opaque::new("win32 tweak", run)
    })
}
