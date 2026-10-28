//! A search field's suggestions: a list under the field that never takes
//! the keyboard, as GTK's own entry completion shows its matches. A
//! popover parented to the entry, without autohide or focus, holds a list
//! of rows: the title, and the detail as a dim caption under it. The keys
//! stay in the entry: Down and Up move through the list, Return picks the
//! row they reached, Escape hides the list, as a pick does, until the user
//! types again.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};
use mitsuami_core::{NodeId, Suggestion, UiEvent};

use crate::host::Events;

/// The most of the list shown before it scrolls.
const MAX_HEIGHT: i32 = 300;

#[derive(Clone)]
pub(crate) struct Suggestions(Rc<Inner>);

struct Inner {
    id: NodeId,
    events: Events,
    entry: gtk::SearchEntry,
    popover: gtk::Popover,
    list: gtk::ListBox,
    scrolled: gtk::ScrolledWindow,
    focus: gtk::EventControllerFocus,
    data: RefCell<Vec<Suggestion>>,
    /// Escape or a pick hid the list, which stays hidden until the user
    /// types again: the app's text after a pick has suggestions too.
    dismissed: Cell<bool>,
}

impl Suggestions {
    pub(crate) fn new(entry: &gtk::SearchEntry, id: NodeId, events: Events) -> Suggestions {
        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::Single);
        list.set_activate_on_single_click(true);
        list.set_focusable(false);
        list.add_css_class("navigation-sidebar");
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scrolled.set_propagate_natural_height(true);
        scrolled.set_max_content_height(MAX_HEIGHT);
        scrolled.set_child(Some(&list));
        let popover = gtk::Popover::new();
        popover.set_child(Some(&scrolled));
        popover.set_autohide(false);
        popover.set_has_arrow(false);
        popover.set_can_focus(false);
        popover.set_focusable(false);
        popover.set_position(gtk::PositionType::Bottom);
        popover.set_halign(gtk::Align::Start);
        popover.set_parent(entry);
        let focus = gtk::EventControllerFocus::new();
        entry.add_controller(focus.clone());
        let this = Suggestions(Rc::new(Inner {
            id,
            events,
            entry: entry.clone(),
            popover,
            list: list.clone(),
            scrolled: scrolled.clone(),
            focus: focus.clone(),
            data: RefCell::default(),
            dismissed: Cell::new(false),
        }));

        // A click on a row picks it.
        let weak = Rc::downgrade(&this.0);
        list.connect_row_activated(move |_, row| {
            if let Some(inner) = weak.upgrade() {
                Suggestions(inner).choose(row.index().max(0) as usize);
            }
        });
        // The list shows while the field has focus.
        for enter in [true, false] {
            let weak = Rc::downgrade(&this.0);
            let sync = move |_: &gtk::EventControllerFocus| {
                if let Some(inner) = weak.upgrade() {
                    Suggestions(inner).sync();
                }
            };
            match enter {
                true => focus.connect_enter(sync),
                false => focus.connect_leave(sync),
            };
        }
        // Before the entry's text sees them: the arrows move through the
        // list, Return picks, Escape hides it, while it's shown.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&this.0);
        keys.connect_key_pressed(move |_, key, _, _| {
            let Some(inner) = weak.upgrade() else { return glib::Propagation::Proceed };
            Suggestions(inner).key(key)
        });
        entry.add_controller(keys);
        // The user's typing, not the app's text (set while events are
        // muted), shows a hidden list again.
        let weak = Rc::downgrade(&this.0);
        entry.connect_changed(move |_| {
            if let Some(inner) = weak.upgrade()
                && !inner.events.is_muted()
            {
                inner.dismissed.set(false);
                Suggestions(inner).sync();
            }
        });
        this
    }

    /// The app's suggestions for the text, shown unless Escape or a pick
    /// hid the list since the user last typed.
    pub(crate) fn set(&self, suggestions: &[Suggestion]) {
        let inner = &self.0;
        if *inner.data.borrow() != suggestions {
            while let Some(row) = inner.list.row_at_index(0) {
                inner.list.remove(&row);
            }
            for suggestion in suggestions {
                inner.list.append(&row(suggestion));
            }
            *inner.data.borrow_mut() = suggestions.to_vec();
        }
        self.sync();
    }

    pub(crate) fn get(&self) -> Vec<Suggestion> {
        self.0.data.borrow().clone()
    }

    /// Picks the suggestion at `index`, as a click does: reports it, and
    /// hides the list. False if there's none there.
    pub(crate) fn choose(&self, index: usize) -> bool {
        let inner = &self.0;
        if index >= inner.data.borrow().len() {
            return false;
        }
        if !inner.events.is_muted() {
            inner.events.emit(inner.id, UiEvent::SuggestionChosen(index));
        }
        inner.dismissed.set(true);
        self.sync();
        true
    }

    /// The popover goes before the entry: GTK wants a widget's children
    /// gone before it's finalized.
    pub(crate) fn close(&self) {
        self.0.popover.unparent();
    }

    /// Shows the list while there's one, the field has focus, and Escape
    /// hasn't hidden it; hides it otherwise.
    fn sync(&self) {
        let inner = &self.0;
        let shown = !inner.data.borrow().is_empty() && inner.focus.contains_focus() && !inner.dismissed.get();
        if shown {
            // As wide as the field, at least.
            inner.popover.set_size_request(inner.entry.width(), -1);
            if !inner.popover.is_visible() {
                inner.list.unselect_all();
                inner.popover.popup();
            } else {
                // A shown popover is laid out in its surface's size, and
                // only `present` measures it again: the entry's own
                // allocation doesn't, so it grew or shrank only when shown
                // anew (the maintainer's report, 2026-10-24).
                inner.popover.present();
            }
        } else if inner.popover.is_visible() {
            inner.popover.popdown();
        }
    }

    fn key(&self, key: gdk::Key) -> glib::Propagation {
        let inner = &self.0;
        if !inner.popover.is_visible() {
            return glib::Propagation::Proceed;
        }
        let selected = inner.list.selected_row().map(|row| row.index());
        let count = inner.data.borrow().len() as i32;
        match key {
            gdk::Key::Down | gdk::Key::KP_Down => {
                let next = selected.map_or(0, |i| (i + 1).min(count - 1));
                self.select(next);
            }
            gdk::Key::Up | gdk::Key::KP_Up => {
                let next = selected.map_or(0, |i| (i - 1).max(0));
                self.select(next);
            }
            gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => match selected {
                // Picked, not a search.
                Some(index) => {
                    self.choose(index as usize);
                }
                None => return glib::Propagation::Proceed,
            },
            gdk::Key::Escape => {
                inner.dismissed.set(true);
                self.sync();
            }
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    }

    /// Selects a row and scrolls to it: the list can't take focus, which
    /// would scroll it there itself.
    fn select(&self, index: i32) {
        let inner = &self.0;
        let Some(row) = inner.list.row_at_index(index) else { return };
        inner.list.select_row(Some(&row));
        let adjustment = inner.scrolled.vadjustment();
        if let Some(bounds) = row.compute_bounds(&inner.list) {
            let (top, bottom) = (bounds.y() as f64, (bounds.y() + bounds.height()) as f64);
            let (value, page) = (adjustment.value(), adjustment.page_size());
            if top < value {
                adjustment.set_value(top);
            } else if bottom > value + page {
                adjustment.set_value(bottom - page);
            }
        }
    }
}

/// A row: the title, and the detail as a dim caption under it.
fn row(suggestion: &Suggestion) -> gtk::ListBoxRow {
    let lines = gtk::Box::new(gtk::Orientation::Vertical, 2);
    lines.set_margin_top(4);
    lines.set_margin_bottom(4);
    let title = gtk::Label::new(Some(&suggestion.title));
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    lines.append(&title);
    if let Some(detail) = &suggestion.detail {
        let detail = gtk::Label::new(Some(detail));
        detail.set_xalign(0.0);
        detail.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        detail.add_css_class("dim-label");
        detail.add_css_class("caption");
        lines.append(&detail);
    }
    let row = gtk::ListBoxRow::new();
    row.set_focusable(false);
    row.set_child(Some(&lines));
    row
}
