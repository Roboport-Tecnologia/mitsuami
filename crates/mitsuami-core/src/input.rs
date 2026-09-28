//! What a `GpuSurface` that takes input reports: keys by where they are on
//! the keyboard, and the pointer over it, as the platform delivers them.

use crate::geometry::Point;

/// Input on a `GpuSurface` that takes it, reported as
/// [`UiEvent::SurfaceInput`](crate::UiEvent::SurfaceInput).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurfaceInput {
    /// A key went down, or up, while the surface had focus. `repeat`: the
    /// platform's own repeat of a key held down.
    Key {
        code: KeyCode,
        /// The platform's own code for the key: a macOS virtual key code,
        /// a Linux evdev code, a Windows scan code (`0xE0` in the high
        /// byte for extended keys).
        native: u32,
        pressed: bool,
        repeat: bool,
        modifiers: Modifiers,
    },
    /// The pointer moved over the surface (anywhere, while a button is
    /// held), in points from its top left. Not reported while the pointer
    /// is locked.
    PointerMoved { position: Point, modifiers: Modifiers },
    /// The pointer left the surface.
    PointerLeft,
    /// A button went down or up at `position`, in points from the
    /// surface's top left.
    Button { button: MouseButton, pressed: bool, position: Point, modifiers: Modifiers },
    /// The wheel turned, or a trackpad scrolled. Positive: towards the end
    /// (down, right), with the user's scrolling direction applied.
    Scroll { delta: ScrollDelta, modifiers: Modifiers },
    /// While the pointer is locked: how far it moved, in points,
    /// accelerated as the cursor would be.
    Motion { dx: f32, dy: f32 },
    /// While the pointer is locked: how far the mouse moved, in the
    /// device's own counts, before the host's acceleration. Reported with
    /// `Motion`, for an app that applies its own (a virtual machine's
    /// guest, a game's camera).
    RawMotion { dx: f32, dy: f32 },
}

/// How far a scroll went.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollDelta {
    /// A wheel's notches (fractions, for high-resolution wheels).
    Lines { x: f32, y: f32 },
    /// A trackpad's (or a precise wheel's) distance, in points.
    Points { x: f32, y: f32 },
}

/// A pointer button, as the platform names it: the primary one is the
/// left one for right-handed users.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
    /// Another, by the platform's number for it.
    Other(u16),
}

/// The modifier keys held with an event. `meta` is Command on macOS, the
/// Super (Windows) key elsewhere.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

macro_rules! key_codes {
    ($($name:ident),* $(,)?) => {
        /// A key by where it is on the keyboard, whatever the layout says
        /// it types: the W3C UI Events `code` names (`KeyA` is the key
        /// right of Caps Lock, also on an AZERTY keyboard).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum KeyCode {
            $($name,)*
            /// A key without a name here: see the event's `native` code.
            Unidentified,
        }

        impl KeyCode {
            /// Its W3C `code` name.
            pub fn name(self) -> &'static str {
                match self {
                    $(KeyCode::$name => stringify!($name),)*
                    KeyCode::Unidentified => "Unidentified",
                }
            }
        }
    };
}

key_codes! {
    KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR,
    KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ, Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7,
    Digit8, Digit9, Backquote, Minus, Equal, BracketLeft, BracketRight, Backslash, Semicolon, Quote, Comma, Period,
    Slash, IntlBackslash, IntlRo, IntlYen, Escape, Tab, CapsLock, Backspace, Enter, Space, ShiftLeft, ShiftRight,
    ControlLeft, ControlRight, AltLeft, AltRight, MetaLeft, MetaRight, ContextMenu, Insert, Delete, Home, End,
    PageUp, PageDown, ArrowUp, ArrowDown, ArrowLeft, ArrowRight, PrintScreen, ScrollLock, Pause, NumLock, Numpad0,
    Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9, NumpadDivide, NumpadMultiply,
    NumpadSubtract, NumpadAdd, NumpadEnter, NumpadDecimal, NumpadEqual, NumpadComma, KanaMode, Lang1, Lang2,
    Convert, NonConvert, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20,
    F21, F22, F23, F24,
}

use KeyCode::*;

/// Linux evdev codes (`linux/input-event-codes.h`) from 1, in order;
/// `Unidentified` for the ones without a name here.
#[rustfmt::skip]
const EVDEV: [KeyCode; 127] = [
    Escape, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9, Digit0, Minus, Equal,
    Backspace, Tab, KeyQ, KeyW, KeyE, KeyR, KeyT, KeyY, KeyU, KeyI, KeyO, KeyP, BracketLeft, BracketRight, Enter,
    ControlLeft, KeyA, KeyS, KeyD, KeyF, KeyG, KeyH, KeyJ, KeyK, KeyL, Semicolon, Quote, Backquote, ShiftLeft,
    Backslash, KeyZ, KeyX, KeyC, KeyV, KeyB, KeyN, KeyM, Comma, Period, Slash, ShiftRight, NumpadMultiply, AltLeft,
    Space, CapsLock, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, NumLock, ScrollLock, Numpad7, Numpad8, Numpad9,
    NumpadSubtract, Numpad4, Numpad5, Numpad6, NumpadAdd, Numpad1, Numpad2, Numpad3, Numpad0, NumpadDecimal,
    // 84, 85 (zenkaku/hankaku), 86 (the ISO key), F11, F12, 89 (ro).
    Unidentified, Unidentified, IntlBackslash, F11, F12, IntlRo,
    // 90..=95: katakana, hiragana, henkan, katakana/hiragana, muhenkan,
    // keypad JP comma.
    Unidentified, Unidentified, Convert, KanaMode, NonConvert, NumpadComma, NumpadEnter, ControlRight,
    NumpadDivide, PrintScreen, AltRight,
    // 101: line feed.
    Unidentified, Home, ArrowUp, PageUp, ArrowLeft, ArrowRight, End, ArrowDown, PageDown, Insert, Delete,
    // 112..=116: macro, mute, volume down, volume up, power.
    Unidentified, Unidentified, Unidentified, Unidentified, Unidentified, NumpadEqual,
    // 118: keypad plus/minus.
    Unidentified, Pause,
    // 120: scale; 121: keypad comma.
    Unidentified, NumpadComma, Lang1, Lang2, IntlYen, MetaLeft, MetaRight, ContextMenu,
];

/// macOS virtual key codes (`kVK_…`), from 0.
#[rustfmt::skip]
const MAC: [KeyCode; 127] = [
    KeyA, KeyS, KeyD, KeyF, KeyH, KeyG, KeyZ, KeyX, KeyC, KeyV, IntlBackslash, KeyB, KeyQ, KeyW, KeyE, KeyR, KeyY,
    KeyT, Digit1, Digit2, Digit3, Digit4, Digit6, Digit5, Equal, Digit9, Digit7, Minus, Digit8, Digit0,
    BracketRight, KeyO, KeyU, BracketLeft, KeyI, KeyP, Enter, KeyL, KeyJ, Quote, KeyK, Semicolon, Backslash, Comma,
    Slash, KeyN, KeyM, Period, Tab, Space, Backquote, Backspace,
    // 0x34: unused.
    Unidentified, Escape, MetaRight, MetaLeft, ShiftLeft, CapsLock, AltLeft, ControlLeft, ShiftRight, AltRight,
    ControlRight,
    // 0x3F: Fn.
    Unidentified, F17, NumpadDecimal, Unidentified, NumpadMultiply, Unidentified, NumpadAdd, Unidentified,
    // 0x47: Clear, where PC keyboards have Num Lock.
    NumLock,
    // 0x48..=0x4A: volume up, down, mute.
    Unidentified, Unidentified, Unidentified, NumpadDivide, NumpadEnter, Unidentified, NumpadSubtract, F18, F19,
    NumpadEqual, Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, F20, Numpad8, Numpad9,
    IntlYen, IntlRo, NumpadComma, F5, F6, F7, F3, F8, F9, Lang2, F11, Lang1, F13, F16, F14, Unidentified, F10,
    ContextMenu, F12, Unidentified, F15,
    // 0x72: Help, where PC keyboards have Insert.
    Insert, Home, PageUp, Delete, F4, End, F2, PageDown, F1, ArrowLeft, ArrowRight, ArrowDown, ArrowUp,
];

impl KeyCode {
    /// The key a Linux evdev code stands for (GTK's and Qt's hardware key
    /// codes, on X11 and Wayland, are 8 more).
    pub fn from_evdev(code: u32) -> KeyCode {
        match code {
            1..=127 => EVDEV[code as usize - 1],
            183..=194 => [F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24][code as usize - 183],
            _ => Unidentified,
        }
    }

    /// The key an XKB keysym names, for the keys that type no character
    /// (modifiers, Caps Lock, Escape, arrows, function keys); `None` for
    /// the others. A keymap's options move these (`ctrl:swapcaps`,
    /// `caps:escape`) above the evdev code, as macOS and Windows remap
    /// below theirs, so they're read from the keysym the key types. Meta
    /// is left out: it's Shift+Alt's keysym on most layouts.
    pub fn from_keysym(keysym: u32) -> Option<KeyCode> {
        Some(match keysym {
            0xff1b => Escape,
            0xff09 | 0xfe20 => Tab,
            0xff0d => Enter,
            0xff08 => Backspace,
            0xffff => Delete,
            0xff63 => Insert,
            0xff50 => Home,
            0xff57 => End,
            0xff55 => PageUp,
            0xff56 => PageDown,
            0xff51 => ArrowLeft,
            0xff52 => ArrowUp,
            0xff53 => ArrowRight,
            0xff54 => ArrowDown,
            0xff61 => PrintScreen,
            0xff67 => ContextMenu,
            0xff13 => Pause,
            0xff14 => ScrollLock,
            0xff7f => NumLock,
            0xffe5 => CapsLock,
            0xffe1 => ShiftLeft,
            0xffe2 => ShiftRight,
            0xffe3 => ControlLeft,
            0xffe4 => ControlRight,
            0xffe9 => AltLeft,
            // AltGr.
            0xffea | 0xfe03 => AltRight,
            0xffeb => MetaLeft,
            0xffec => MetaRight,
            0xffbe..=0xffd5 => [
                F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21, F22,
                F23, F24,
            ][keysym as usize - 0xffbe],
            _ => return None,
        })
    }

    /// The key a macOS virtual key code stands for.
    pub fn from_mac(code: u16) -> KeyCode {
        MAC.get(code as usize).copied().unwrap_or(Unidentified)
    }

    /// The key a Windows scan code stands for: set 1, `0xE0` in the high
    /// byte for extended keys (as `WM_KEYDOWN` flags them).
    pub fn from_windows_scancode(code: u32) -> KeyCode {
        match code {
            // Where set 1 and evdev agree.
            0x01..=0x53 if code != 0x45 => EVDEV[code as usize - 1],
            // Pause comes without the extended flag, Num Lock with it.
            0x45 => Pause,
            0x56 => IntlBackslash,
            0x57 => F11,
            0x58 => F12,
            0x59 => NumpadEqual,
            0x64..=0x6E => [F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23][code as usize - 0x64],
            0x70 => KanaMode,
            0x73 => IntlRo,
            0x76 => F24,
            0x79 => Convert,
            0x7B => NonConvert,
            0x7D => IntlYen,
            0x7E => NumpadComma,
            0x54 => PrintScreen,
            0xE01C => NumpadEnter,
            0xE01D => ControlRight,
            0xE035 => NumpadDivide,
            0xE037 => PrintScreen,
            0xE038 => AltRight,
            0xE045 => NumLock,
            0xE047 => Home,
            0xE048 => ArrowUp,
            0xE049 => PageUp,
            0xE04B => ArrowLeft,
            0xE04D => ArrowRight,
            0xE04F => End,
            0xE050 => ArrowDown,
            0xE051 => PageDown,
            0xE052 => Insert,
            0xE053 => Delete,
            0xE05B => MetaLeft,
            0xE05C => MetaRight,
            0xE05D => ContextMenu,
            0x72 => Lang1,
            0x71 => Lang2,
            _ => Unidentified,
        }
    }

    /// The key that types this character on a US keyboard, unshifted:
    /// what tests type.
    pub fn from_us_char(c: char) -> KeyCode {
        const LETTERS: [KeyCode; 26] = [
            KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR,
            KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
        ];
        const DIGITS: [KeyCode; 10] = [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
        match c.to_ascii_lowercase() {
            c @ 'a'..='z' => LETTERS[c as usize - 'a' as usize],
            c @ '0'..='9' => DIGITS[c as usize - '0' as usize],
            ' ' => Space,
            '`' => Backquote,
            '-' => Minus,
            '=' => Equal,
            '[' => BracketLeft,
            ']' => BracketRight,
            '\\' => Backslash,
            ';' => Semicolon,
            '\'' => Quote,
            ',' => Comma,
            '.' => Period,
            '/' => Slash,
            _ => Unidentified,
        }
    }
}
