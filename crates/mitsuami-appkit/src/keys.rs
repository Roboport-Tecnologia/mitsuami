//! A node's keys (`Prop::Keys`): keys the focused view doesn't use go up
//! the responder chain (`keyDown:` passed to the next responder, a view's
//! superview), and the nearest host or table that takes one reports it.

use mitsuami_core::services::Shortcut;
use mitsuami_core::{EventSink, KeyCode, NodeId, UiEvent};
use objc2::MainThreadMarker;
use objc2::Message;
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSView, NSWindow};
use objc2_foundation::{NSPoint, NSString};

use crate::classes::HostView;
use crate::services::{event_character, key_of};

/// What a node takes, and where it reports it.
pub(crate) struct Keys {
    pub(crate) id: NodeId,
    pub(crate) keys: Vec<Shortcut>,
    pub(crate) events: EventSink,
}

impl Keys {
    /// Reports the event's key if the node takes it.
    pub(crate) fn take(&self, event: &NSEvent) -> bool {
        shortcut_of(event).is_some_and(|shortcut| self.take_shortcut(shortcut))
    }

    /// Reports the shortcut if the node takes it.
    pub(crate) fn take_shortcut(&self, shortcut: Shortcut) -> bool {
        let taken = self.keys.contains(&shortcut);
        if taken {
            self.events.emit(self.id, UiEvent::Key(shortcut));
        }
        taken
    }
}

/// The shortcut a key event is: its key, a character as the layout types
/// it without modifiers (so Shift+[ is `[` with Shift), and ⌘, ⇧ and ⌥.
pub(crate) fn shortcut_of(event: &NSEvent) -> Option<Shortcut> {
    let first = |s: Option<Retained<NSString>>| s.and_then(|s| s.to_string().chars().next());
    let ignoring = first(event.charactersIgnoringModifiers())?;
    // Keys that type nothing are AppKit's function-key characters and
    // control characters here; the layout gives them other codes.
    let key = if ignoring.is_control() || ('\u{f700}'..='\u{f8ff}').contains(&ignoring) {
        ignoring
    } else {
        first(event.charactersByApplyingModifiers(NSEventModifierFlags::empty())).unwrap_or(ignoring)
    };
    let mut shortcut = Shortcut::new(key_of(key));
    let flags = event.modifierFlags();
    shortcut.primary = flags.contains(NSEventModifierFlags::Command);
    shortcut.shift = flags.contains(NSEventModifierFlags::Shift);
    shortcut.alt = flags.contains(NSEventModifierFlags::Option);
    Some(shortcut)
}

/// A key down of this shortcut in `window`, as the keyboard sends it.
pub(crate) fn key_event(window: &NSWindow, shortcut: Shortcut) -> Option<Retained<NSEvent>> {
    let code = (0..0x7F).find(|c| KeyCode::from_mac(*c) == KeyCode::from_key(shortcut.key))?;
    let mut flags = NSEventModifierFlags::empty();
    for (held, flag) in [
        (shortcut.primary, NSEventModifierFlags::Command),
        (shortcut.shift, NSEventModifierFlags::Shift),
        (shortcut.alt, NSEventModifierFlags::Option),
    ] {
        if held {
            flags |= flag;
        }
    }
    let character = event_character(shortcut.key);
    // Keys that type nothing are function keys (the arrows are on the
    // keypad too), as AppKit marks them.
    if ('\u{f700}'..='\u{f8ff}').contains(&character) {
        flags |= NSEventModifierFlags::Function;
        if ('\u{f700}'..='\u{f703}').contains(&character) {
            flags |= NSEventModifierFlags::NumericPad;
        }
    }
    let characters = NSString::from_str(&character.to_string());
    NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::new(0.0, 0.0),
        flags,
        0.0,
        window.windowNumber(),
        None,
        &characters,
        &characters,
        false,
        code,
    )
}

/// Whether a view from `view` up takes the shortcut: a key nothing takes
/// makes AppKit beep.
pub(crate) fn taken_around(view: &NSView, shortcut: Shortcut) -> bool {
    let mut at = Some(view.retain());
    while let Some(view) = at {
        if let Some(host) = view.downcast_ref::<HostView>()
            && host.takes(shortcut)
        {
            return true;
        }
        if crate::list::takes(&view, shortcut) {
            return true;
        }
        at = unsafe { view.superview() };
    }
    false
}

/// Reports the shortcut to the nearest host or table from `view` up that
/// takes it, as a key passed up the responder chain would be.
pub(crate) fn take_around(view: &NSView, shortcut: Shortcut) -> bool {
    let mut at = Some(view.retain());
    while let Some(view) = at {
        if let Some(host) = view.downcast_ref::<HostView>()
            && host.take(shortcut)
        {
            return true;
        }
        if crate::list::take(&view, shortcut) {
            return true;
        }
        at = unsafe { view.superview() };
    }
    false
}

/// Offers the key being handled to the nodes around a text view that has
/// no use for it. AppKit's text views (a text area's, a field's editor)
/// turn such a key into their `noop:` command and beep, rather than pass
/// it up the responder chain as other views do: ⌘↵ in a commit message
/// went nowhere.
pub(crate) fn take_unused(view: &NSView) -> bool {
    let app = NSApplication::sharedApplication(MainThreadMarker::from(view));
    app.currentEvent()
        .filter(|event| event.r#type() == NSEventType::KeyDown)
        .and_then(|event| shortcut_of(&event))
        .is_some_and(|shortcut| take_around(view, shortcut))
}
