//! A node's keys (`Prop::Keys`): a `KeyDown` handler on its element. XAML
//! raises `KeyDown` on the focused element, then on each element around
//! it, and a control that uses the key (a list view's arrows and Space, a
//! text box's typing) marks it handled on the way, which handlers after it
//! don't see. So a node sees only the keys the controls inside it passed
//! on, and the nearest node that takes one handles it for those around.
//!
//! Not `KeyboardAccelerator`s: XAML looks at an element's accelerators
//! before raising its `KeyDown`, so a list's would take its arrows ahead
//! of the list view itself.
//!
//! A text box's or search box's own keys come before its editing: they're
//! taken on `PreviewKeyDown`, which tunnels to it before its `KeyDown`.
//! An auto-suggest box's open suggestion list keeps its arrows, Return and
//! Escape.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami_core::services::Shortcut;
use mitsuami_core::{NodeId, UiEvent};
use windows_core::{EventRevoker, Interface};

use super::menus::key_of;
use super::{Events, R};
use crate::bindings as w;

/// What a node takes, and the handler that takes it.
pub(super) struct Keys {
    id: NodeId,
    events: Events,
    taken: Rc<RefCell<Vec<Shortcut>>>,
    _key_down: EventRevoker,
}

impl Keys {
    pub(super) fn new(id: NodeId, events: Events, element: &w::UIElement, keys: Vec<Shortcut>) -> R<Keys> {
        let taken = Rc::new(RefCell::new(keys));
        let key_down = element.cast::<w::IUIElement>()?.KeyDown({
            let (events, taken) = (events.clone(), taken.clone());
            move |_, args| {
                let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()) else { return };
                let Some(shortcut) = args.Key().ok().and_then(held) else { return };
                if take(&events, id, &taken, shortcut) {
                    _ = args.SetHandled(true);
                }
            }
        })?;
        Ok(Keys { id, events, taken, _key_down: key_down })
    }

    /// A text field's own keys, ahead of its editing; `suggestions` is the
    /// auto-suggest box whose open list keeps its keys.
    pub(super) fn before(
        id: NodeId,
        events: Events,
        element: &w::UIElement,
        suggestions: Option<w::AutoSuggestBox>,
        keys: Vec<Shortcut>,
    ) -> R<Keys> {
        let taken = Rc::new(RefCell::new(keys));
        let key_down = element.cast::<w::IUIElement>()?.PreviewKeyDown({
            let (events, taken) = (events.clone(), taken.clone());
            move |_, args| {
                let Some(args) = args.as_ref().and_then(|a| a.cast::<w::IKeyRoutedEventArgs>().ok()) else { return };
                let Some(shortcut) = args.Key().ok().and_then(held) else { return };
                let listed = suggestions.as_ref().is_some_and(|s| s.IsSuggestionListOpen().unwrap_or(false))
                    && matches!(
                        shortcut.key,
                        mitsuami_core::Key::Up
                            | mitsuami_core::Key::Down
                            | mitsuami_core::Key::Enter
                            | mitsuami_core::Key::Escape
                    );
                if !listed && take(&events, id, &taken, shortcut) {
                    _ = args.SetHandled(true);
                }
            }
        })?;
        Ok(Keys { id, events, taken, _key_down: key_down })
    }

    pub(super) fn set(&self, keys: Vec<Shortcut>) {
        *self.taken.borrow_mut() = keys;
    }

    pub(super) fn keys(&self) -> Vec<Shortcut> {
        self.taken.borrow().clone()
    }

    /// Reports the shortcut if the node takes it, as its handler does.
    pub(super) fn take(&self, shortcut: Shortcut) -> bool {
        take(&self.events, self.id, &self.taken, shortcut)
    }
}

fn take(events: &Events, id: NodeId, taken: &RefCell<Vec<Shortcut>>, shortcut: Shortcut) -> bool {
    let takes = taken.borrow().contains(&shortcut);
    if takes {
        // A surface that heard of the key first holds it back for this.
        crate::surface::key_taken();
        events.emit(id, UiEvent::Key(shortcut));
    }
    takes
}

/// The shortcut a key is with the modifiers held now: Control is primary,
/// Alt (`VK_MENU`) is alt. `KeyDown` doesn't carry them; the key state
/// is the one of the key message XAML is handling.
fn held(key: w::VirtualKey) -> Option<Shortcut> {
    let down = |key: i32| unsafe { w::GetKeyState(key) } < 0;
    let mut shortcut = Shortcut::new(key_of(key)?);
    shortcut.primary = down(w::VK_CONTROL);
    shortcut.shift = down(w::VK_SHIFT);
    shortcut.alt = down(w::VK_MENU);
    Some(shortcut)
}
