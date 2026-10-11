//! Document tabs: libadwaita's tab bar, as GNOME Console and Text Editor
//! have it, over a tab view that stands in for the documents. The app's
//! content isn't in the view: each page holds an empty placeholder, and
//! the view stays hidden, so only the bar shows.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use mitsuami_core::{DocumentTabsStyle, EventValue, NodeId, Size, UiEvent};

use crate::host::Events;

/// The tab view's shortcuts that move the selected tab.
const MOVE_SHORTCUTS: adw::TabViewShortcuts = adw::TabViewShortcuts::CONTROL_SHIFT_PAGE_UP
    .union(adw::TabViewShortcuts::CONTROL_SHIFT_PAGE_DOWN)
    .union(adw::TabViewShortcuts::CONTROL_SHIFT_HOME)
    .union(adw::TabViewShortcuts::CONTROL_SHIFT_END);

/// The node's native parts, and what GTK can't hold of the core's view.
pub(crate) struct DocumentTabs {
    root: gtk::Box,
    bar: adw::TabBar,
    view: adw::TabView,
    new_tab: gtk::Button,
    shared: Rc<Shared>,
    style: Cell<Option<DocumentTabsStyle>>,
    /// The tab the core chose, applied once its page is there.
    selected: Cell<Option<usize>>,
}

/// What the view's handlers read.
#[derive(Default)]
struct Shared {
    /// The pages in the order the core last heard: a reordered page's old
    /// place, which the view doesn't say.
    order: RefCell<Vec<adw::TabPage>>,
    /// Kept: libadwaita's tabs always have close buttons, and always move.
    closable: Cell<Option<bool>>,
    reorderable: Cell<Option<bool>>,
    /// Set while the backend takes a page away, which closes it.
    removing: Cell<bool>,
}

impl DocumentTabs {
    pub(crate) fn new(id: NodeId, events: Events) -> DocumentTabs {
        let shared = Rc::new(Shared::default());
        let view = adw::TabView::new();
        // The documents are the app's, shown outside it.
        view.set_visible(false);
        view.remove_shortcuts(MOVE_SHORTCUTS);
        let bar = adw::TabBar::new();
        bar.set_view(Some(&view));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&bar);
        root.append(&view);

        // The user's choice; the backend's own are muted.
        let e = events.clone();
        view.connect_selected_page_notify(move |view| {
            if let Some(index) = view.selected_page().map(|p| view.page_position(&p)) {
                e.emit(id, UiEvent::Changed(EventValue::Index(index as usize)));
            }
        });
        // A close button, a middle click or Ctrl+W only ask: the app
        // takes the tab away. The backend's own removals go through.
        let (e, s) = (events.clone(), shared.clone());
        view.connect_close_page(move |view, page| {
            if s.removing.get() {
                view.close_page_finish(page, true);
                return glib::Propagation::Stop;
            }
            view.close_page_finish(page, false);
            if s.closable.get() == Some(true) {
                e.emit(id, UiEvent::TabCloseRequested(view.page_position(page) as usize));
            }
            glib::Propagation::Stop
        });
        // libadwaita's tabs can always be dragged: tabs that don't move go
        // back where they were, unreported.
        let (e, s) = (events.clone(), shared.clone());
        view.connect_page_reordered(move |view, page, to| {
            let Some(from) = s.order.borrow().iter().position(|p| p == page) else { return };
            let to = to as usize;
            if from == to {
                return;
            }
            if s.reorderable.get() != Some(true) {
                let (view, page) = (view.clone(), page.clone());
                glib::idle_add_local_once(move || {
                    view.reorder_page(&page, from as i32);
                });
                return;
            }
            let mut order = s.order.borrow_mut();
            let moved = order.remove(from);
            order.insert(to, moved);
            drop(order);
            e.emit(id, UiEvent::TabMoved { from, to });
        });
        // Dragged out of the window, a tab would need a window of its own,
        // which the app hasn't made: the drag is cancelled.
        view.connect_create_window(|_| None);

        // In the bar's own place for actions, as the header bar is the
        // app's.
        let new_tab = gtk::Button::from_icon_name("tab-new-symbolic");
        new_tab.add_css_class("flat");
        let label = mitsuami_core::l10n::tr("mitsuami-tab-new", &[]);
        new_tab.set_tooltip_text(Some(&label));
        new_tab.update_property(&[gtk::accessible::Property::Label(&label)]);
        let e = events.clone();
        new_tab.connect_clicked(move |_| e.emit(id, UiEvent::NewTabRequested));

        // The bar hides itself with one tab (its autohide), and shows
        // again with two: it's measured again then.
        if let Some(revealer) = revealer(&bar) {
            revealer.connect_reveal_child_notify(move |_| events.emit(id, UiEvent::Remeasure));
        }
        // New tabs get close buttons: hidden again where tabs don't close.
        let (s, b) = (shared.clone(), bar.downgrade());
        view.connect_n_pages_notify(move |_| {
            let (s, b) = (s.clone(), b.clone());
            glib::idle_add_local_once(move || {
                if let Some(bar) = b.upgrade() {
                    show_close_buttons(bar.upcast_ref(), s.closable.get() == Some(true));
                }
            });
        });

        DocumentTabs { root, bar, view, new_tab, shared, style: Cell::new(None), selected: Cell::new(None) }
    }

    /// The widget the node is.
    pub(crate) fn root(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    /// Every style is a strip here: GNOME has no window tabs.
    pub(crate) fn set_style(&self, style: DocumentTabsStyle) {
        self.style.set(Some(style));
    }

    pub(crate) fn style(&self) -> Option<DocumentTabsStyle> {
        self.style.get()
    }

    /// Makes the pages match the titles: a page taken away or put in where
    /// the titles first differ, so a closed tab goes with its own
    /// animation, then each page titled.
    pub(crate) fn set_titles(&self, titles: &[String]) {
        let mut shown = self.titles();
        while shown.len() > titles.len() {
            let at = first_difference(&shown, titles);
            self.remove(at);
            shown.remove(at);
        }
        while shown.len() < titles.len() {
            let at = first_difference(&shown, titles);
            let page = self.view.insert(&gtk::Box::new(gtk::Orientation::Horizontal, 0), at as i32);
            page.set_title(&titles[at]);
            self.shared.order.borrow_mut().insert(at, page);
            shown.insert(at, titles[at].clone());
        }
        for (page, title) in self.shared.order.borrow().iter().zip(titles) {
            if page.title().as_str() != title {
                page.set_title(title);
            }
        }
        self.show_selected();
    }

    fn remove(&self, index: usize) {
        let page = self.shared.order.borrow_mut().remove(index);
        self.shared.removing.set(true);
        self.view.close_page(&page);
        self.shared.removing.set(false);
    }

    pub(crate) fn set_selected(&self, index: Option<usize>) {
        self.selected.set(index);
        self.show_selected();
    }

    fn show_selected(&self) {
        let page = self.selected.get().and_then(|i| self.shared.order.borrow().get(i).cloned());
        if let Some(page) = page
            && self.view.selected_page().as_ref() != Some(&page)
        {
            self.view.set_selected_page(&page);
        }
    }

    pub(crate) fn set_closable(&self, closable: bool) {
        self.shared.closable.set(Some(closable));
        show_close_buttons(self.bar.upcast_ref(), closable);
    }

    pub(crate) fn set_reorderable(&self, reorderable: bool) {
        self.shared.reorderable.set(Some(reorderable));
        if reorderable {
            self.view.add_shortcuts(MOVE_SHORTCUTS);
        } else {
            self.view.remove_shortcuts(MOVE_SHORTCUTS);
        }
    }

    pub(crate) fn set_new_tab_button(&self, shown: bool) {
        self.bar.set_end_action_widget(shown.then_some(&self.new_tab));
    }

    /// The tabs' titles, as the view has them.
    pub(crate) fn titles(&self) -> Vec<String> {
        (0..self.view.n_pages()).map(|i| self.view.nth_page(i).title().to_string()).collect()
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        self.view.selected_page().map(|p| self.view.page_position(&p) as usize)
    }

    pub(crate) fn closable(&self) -> Option<bool> {
        self.shared.closable.get()
    }

    pub(crate) fn reorderable(&self) -> Option<bool> {
        self.shared.reorderable.get()
    }

    pub(crate) fn has_new_tab_button(&self) -> bool {
        self.bar.end_action_widget().is_some()
    }

    /// The bar's natural width, and its height as it will be: none while
    /// it hides itself, though its revealer is still sliding.
    pub(crate) fn measure(&self) -> Size {
        let Some(revealer) = revealer(&self.bar) else {
            let width = self.bar.measure(gtk::Orientation::Horizontal, -1).1;
            return Size::new(width as f32, self.bar.measure(gtk::Orientation::Vertical, width).1 as f32);
        };
        let Some(child) = revealer.child() else { return Size::ZERO };
        let width = child.measure(gtk::Orientation::Horizontal, -1).1;
        let height = if revealer.reveals_child() { child.measure(gtk::Orientation::Vertical, width).1 } else { 0 };
        Size::new(width as f32, height as f32)
    }

    /// Chooses the first tab with this title, as a click on it does,
    /// which the view reports. `false` if there's none.
    pub(crate) fn choose(&self, title: &str) -> bool {
        let Some(index) = self.titles().iter().position(|t| t == title) else { return false };
        self.view.set_selected_page(&self.view.nth_page(index as i32));
        true
    }

    /// Presses a tab's close button: the view asks, and the app hears.
    pub(crate) fn close(&self, index: usize) -> bool {
        if self.shared.closable.get() != Some(true) || index >= self.view.n_pages() as usize {
            return false;
        }
        self.view.close_page(&self.view.nth_page(index as i32));
        true
    }

    /// Moves a tab, as a drag does, which the view reports.
    pub(crate) fn move_tab(&self, from: usize, to: usize) -> bool {
        let count = self.view.n_pages() as usize;
        if self.shared.reorderable.get() != Some(true) || from >= count || to >= count {
            return false;
        }
        from == to || self.view.reorder_page(&self.view.nth_page(from as i32), to as i32)
    }

    /// Presses the new-tab button, if it has one.
    pub(crate) fn new_tab(&self) -> bool {
        if !self.has_new_tab_button() {
            return false;
        }
        self.new_tab.emit_clicked();
        true
    }

    /// Gives the tabs keyboard focus: the selected one.
    pub(crate) fn focus(&self) -> bool {
        self.bar.child_focus(gtk::DirectionType::TabForward)
    }
}

/// The revealer the bar slides in and out with (its template's only
/// child).
fn revealer(bar: &adw::TabBar) -> Option<gtk::Revealer> {
    bar.first_child().and_downcast()
}

/// Where the two lists first differ, or the shorter one's end.
fn first_difference(shown: &[String], titles: &[String]) -> usize {
    shown.iter().zip(titles).position(|(a, b)| a != b).unwrap_or(shown.len().min(titles.len()))
}

/// Shows or hides the tabs' close buttons (`tab-close-button`), which
/// libadwaita gives every tab that isn't pinned.
fn show_close_buttons(widget: &gtk::Widget, shown: bool) {
    let mut child = widget.first_child();
    while let Some(w) = child {
        if w.has_css_class("tab-close-button") {
            w.set_visible(shown);
        } else {
            show_close_buttons(&w, shown);
        }
        child = w.next_sibling();
    }
}
