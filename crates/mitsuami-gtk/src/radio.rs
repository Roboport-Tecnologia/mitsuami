//! A radio group: check buttons in one group, down a box, as GTK makes
//! radio buttons. The box is the group to assistive technology.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use mitsuami_core::{EventValue, NodeId, UiEvent};

use crate::host::Events;

/// Between the buttons, as GNOME's dialogs space related controls.
const SPACING: i32 = 6;

#[derive(Clone)]
pub(crate) struct RadioGroup {
    pub column: gtk::Box,
    buttons: Rc<RefCell<Vec<gtk::CheckButton>>>,
    events: Events,
    id: NodeId,
}

impl RadioGroup {
    pub(crate) fn new(events: Events, id: NodeId) -> RadioGroup {
        let column = glib::Object::builder::<gtk::Box>()
            .property("orientation", gtk::Orientation::Vertical)
            .property("spacing", SPACING)
            .property("accessible-role", gtk::AccessibleRole::RadioGroup)
            .build();
        RadioGroup { column, buttons: Rc::default(), events, id }
    }

    /// A button for each option. The ones there already keep their place
    /// (and whether they're chosen), with the option now at it.
    pub(crate) fn set_options(&self, options: &[String]) {
        let mut buttons = self.buttons.borrow_mut();
        while buttons.len() > options.len() {
            let button = buttons.pop().unwrap();
            button.set_group(None::<&gtk::CheckButton>);
            self.column.remove(&button);
        }
        for (index, option) in options.iter().enumerate() {
            if let Some(button) = buttons.get(index) {
                button.set_label(Some(option));
                continue;
            }
            let button = gtk::CheckButton::with_label(option);
            button.set_group(buttons.first());
            // Only the one the user turns on: the one turned off with it
            // says nothing, and GTK doesn't let a click turn one off.
            let (events, id) = (self.events.clone(), self.id);
            button.connect_toggled(move |b| {
                if b.is_active() {
                    events.emit(id, UiEvent::Changed(EventValue::Index(index)));
                }
            });
            self.column.append(&button);
            buttons.push(button);
        }
    }

    pub(crate) fn options(&self) -> Vec<String> {
        self.buttons.borrow().iter().map(|b| b.label().map(|l| l.to_string()).unwrap_or_default()).collect()
    }

    pub(crate) fn set_selected(&self, index: Option<usize>) {
        let buttons = self.buttons.borrow();
        match index.and_then(|i| buttons.get(i)) {
            Some(button) => button.set_active(true),
            None => buttons.iter().for_each(|b| b.set_active(false)),
        }
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        self.buttons.borrow().iter().position(|b| b.is_active())
    }

    pub(crate) fn has(&self, option: &str) -> bool {
        self.button(option).is_some()
    }

    fn button(&self, option: &str) -> Option<gtk::CheckButton> {
        self.buttons.borrow().iter().find(|b| b.label().as_deref() == Some(option)).cloned()
    }

    /// Presses the first button with this option, as the user clicks it:
    /// on the chosen one, that does nothing.
    pub(crate) fn choose(&self, option: &str) {
        if let Some(button) = self.button(option) {
            button.emit_by_name::<()>("activate", &[]);
        }
    }

    /// Focuses the chosen button, or the first, as Tab reaches the group.
    pub(crate) fn focus(&self) -> bool {
        let buttons = self.buttons.borrow();
        let button = buttons.iter().find(|b| b.is_active()).or(buttons.first()).cloned();
        drop(buttons);
        button.is_some_and(|b| b.grab_focus())
    }
}
