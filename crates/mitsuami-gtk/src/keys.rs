//! A node's keys (`Prop::Keys`): a shortcut controller on the node's
//! widget, in the bubble phase. GTK hands a key to the focused widget and
//! then up its ancestors, so the widgets on the way use it first (a list
//! view's arrows and Space, an entry's editing keys), and only keys none
//! of them use reach the node.

use gtk::prelude::*;
use gtk::{gdk, glib};
use mitsuami_core::services::Shortcut;
use mitsuami_core::{NodeId, UiEvent};

use crate::host::Events;
use crate::services::{key_of, trigger};

/// The modifiers GTK's key triggers compare.
const HELD: gdk::ModifierType = gdk::ModifierType::CONTROL_MASK
    .union(gdk::ModifierType::SHIFT_MASK)
    .union(gdk::ModifierType::ALT_MASK)
    .union(gdk::ModifierType::SUPER_MASK)
    .union(gdk::ModifierType::META_MASK);

/// The node's keys, in place of the ones it had: one shortcut each, which
/// reports the key and stops it there. The node keeps its controller,
/// made on its first keys.
pub(crate) fn set_keys(
    kept: &mut Option<gtk::ShortcutController>,
    widget: &gtk::Widget,
    id: NodeId,
    events: &Events,
    keys: &[Shortcut],
) {
    let controller = kept.get_or_insert_with(|| {
        let controller = gtk::ShortcutController::new();
        controller.set_scope(gtk::ShortcutScope::Local);
        controller.set_propagation_phase(gtk::PropagationPhase::Bubble);
        widget.add_controller(controller.clone());
        controller
    });
    while let Some(old) = controller.item(0).and_downcast::<gtk::Shortcut>() {
        controller.remove_shortcut(&old);
    }
    for &shortcut in keys {
        let Some((key, modifiers)) = gtk::accelerator_parse(trigger(&shortcut)) else { continue };
        let events = events.clone();
        let action = gtk::CallbackAction::new(move |_, _| {
            events.emit(id, UiEvent::Key(shortcut));
            glib::Propagation::Stop
        });
        controller.add_shortcut(gtk::Shortcut::new(Some(gtk::KeyvalTrigger::new(key, modifiers)), Some(action)));
    }
}

/// The keys the node's controller takes, read back from its triggers.
pub(crate) fn keys(controller: &gtk::ShortcutController) -> Vec<Shortcut> {
    (0..controller.n_items())
        .filter_map(|i| controller.item(i).and_downcast::<gtk::Shortcut>())
        .filter_map(|s| s.trigger().and_downcast::<gtk::KeyvalTrigger>())
        .filter_map(|t| {
            let modifiers = t.modifiers();
            Some(Shortcut {
                key: key_of(t.keyval())?,
                primary: modifiers.contains(gdk::ModifierType::CONTROL_MASK),
                shift: modifiers.contains(gdk::ModifierType::SHIFT_MASK),
                alt: modifiers.contains(gdk::ModifierType::ALT_MASK),
            })
        })
        .collect()
}

/// A key pressed with `focus` focused. GTK 4 can't inject key events, so
/// this runs the shortcut controllers the event would pass, in GTK's
/// order: the capture phase from the window down, then the target and
/// bubble phases back up. The first shortcut that takes the key acts: a
/// widget's own binding, a node's keys, the window's (Tab moving focus,
/// the app's menus). Whether one took it.
pub(crate) fn press(focus: &gtk::Widget, shortcut: Shortcut) -> bool {
    let Some((key, modifiers)) = gtk::accelerator_parse(trigger(&shortcut)) else { return false };
    let path: Vec<gtk::Widget> = std::iter::successors(Some(focus.clone()), |w| w.parent()).collect();
    let run = |widget: &gtk::Widget, phase| run(widget, phase, key, modifiers);
    path.iter().rev().any(|w| run(w, gtk::PropagationPhase::Capture))
        || run(focus, gtk::PropagationPhase::Target)
        || path.iter().any(|w| run(w, gtk::PropagationPhase::Bubble))
}

/// A widget's local shortcut controllers of that phase, newest first as
/// GTK runs them. Managed and global ones run from their shortcut
/// manager's (the window's) controllers instead.
fn run(widget: &gtk::Widget, phase: gtk::PropagationPhase, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
    // GTK skips shortcuts on insensitive widgets.
    if !widget.is_sensitive() {
        return false;
    }
    let controllers = widget.observe_controllers();
    let controllers: Vec<gtk::ShortcutController> = (0..controllers.n_items())
        .filter_map(|i| controllers.item(i).and_downcast::<gtk::ShortcutController>())
        .filter(|c| c.propagation_phase() == phase && c.scope() == gtk::ShortcutScope::Local)
        .collect();
    controllers.iter().any(|controller| {
        let matched: Vec<gtk::Shortcut> = (0..controller.n_items())
            .filter_map(|i| controller.item(i).and_downcast::<gtk::Shortcut>())
            .filter(|s| s.trigger().is_some_and(|t| matches(&t, key, modifiers)))
            .collect();
        let last = matched.len().saturating_sub(1);
        matched.iter().enumerate().any(|(i, shortcut)| {
            let flags = if i == last { gtk::ShortcutActionFlags::EXCLUSIVE } else { gtk::ShortcutActionFlags::empty() };
            shortcut.action().is_some_and(|action| action.activate(flags, widget, shortcut.arguments().as_ref()))
        })
    })
}

/// Whether a trigger fires for the key, as a key press would match it:
/// letters in either case (bindings name Shift+A with the capital).
/// Mnemonics aren't simulated.
fn matches(trigger: &gtk::ShortcutTrigger, key: gdk::Key, modifiers: gdk::ModifierType) -> bool {
    if let Some(t) = trigger.downcast_ref::<gtk::KeyvalTrigger>() {
        t.keyval().to_lower() == key.to_lower() && t.modifiers() & HELD == modifiers & HELD
    } else if let Some(t) = trigger.downcast_ref::<gtk::AlternativeTrigger>() {
        matches(&t.first(), key, modifiers) || matches(&t.second(), key, modifiers)
    } else {
        false
    }
}
