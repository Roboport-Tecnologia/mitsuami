//! A tab view: a `gtk::Notebook`, each page host a page, its tab a label
//! titled from the node's titles. The notebook stretches a page's child
//! over its page area, so each host sits in a host of its own, at the top
//! left, at the size the core gave it.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use gtk::graphene;
use gtk::prelude::*;
use mitsuami_core::{EventValue, Insets, NodeId, Point, Rect, Size, UiEvent};

use crate::host::{Events, Frames, Host};

/// The tab view node's native parts, and the core's view of them.
pub(crate) struct Tabs {
    pub notebook: gtk::Notebook,
    frames: Frames,
    titles: RefCell<Vec<String>>,
    /// The page the core shows, which may come before its page in a batch.
    selected: Cell<Option<usize>>,
    /// Where the core put each page, until the notebook has placed it.
    origins: RefCell<HashMap<gtk::Widget, Point>>,
}

impl Tabs {
    pub(crate) fn new(id: NodeId, events: Events, frames: Frames) -> Tabs {
        let notebook = gtk::Notebook::new();
        // It fires for the core's switches and for pages coming and going
        // too: the backend mutes events while it applies commands.
        notebook.connect_switch_page(move |_, _, index| {
            events.emit(id, UiEvent::Changed(EventValue::Index(index as usize)))
        });
        Tabs { notebook, frames, titles: RefCell::default(), selected: Cell::new(None), origins: RefCell::default() }
    }

    /// Titles pages by index, whichever of the two came first.
    pub(crate) fn set_titles(&self, titles: Vec<String>) {
        *self.titles.borrow_mut() = titles;
        self.relabel();
    }

    fn relabel(&self) {
        let titles = self.titles.borrow();
        for (index, page) in self.wrappers().iter().enumerate() {
            let title = titles.get(index).map_or("", String::as_str);
            match self.notebook.tab_label(page).and_downcast::<gtk::Label>() {
                Some(label) if label.label() != title => label.set_label(title),
                Some(_) => {}
                None => self.notebook.set_tab_label(page, Some(&gtk::Label::new(Some(title)))),
            }
        }
    }

    pub(crate) fn set_selected(&self, index: Option<usize>) {
        self.selected.set(index);
        self.show_selected();
    }

    /// Shows the core's page, once it's there: the notebook shows its first
    /// page when it gets one, and keeps showing a page others are put
    /// before.
    fn show_selected(&self) {
        if let Some(index) = self.selected.get().filter(|i| *i < self.notebook.n_pages() as usize) {
            self.notebook.set_current_page(Some(index as u32));
        }
    }

    pub(crate) fn insert(&self, index: usize, host: &gtk::Widget) {
        let wrapper = Host::new(self.frames.clone(), None);
        host.set_parent(&wrapper);
        self.place(host);
        let label = gtk::Label::new(Some(self.titles.borrow().get(index).map_or("", String::as_str)));
        self.notebook.insert_page(&wrapper, Some(&label), Some(index as u32));
        self.relabel();
        self.show_selected();
    }

    pub(crate) fn remove(&self, host: &gtk::Widget) {
        let Some(wrapper) = host.parent() else { return };
        host.unparent();
        if let Some(index) = self.notebook.page_num(&wrapper) {
            self.notebook.remove_page(Some(index));
        }
        // It goes back to its place in its next parent.
        if let Some(origin) = self.origins.borrow_mut().remove(host)
            && let Some(frame) = self.frames.borrow_mut().get_mut(host)
        {
            frame.origin = origin;
        }
        self.relabel();
        self.show_selected();
    }

    /// Keeps a page's size and puts it at the top left of its wrapper: the
    /// notebook places the wrapper. Called when a page's frame changes.
    pub(crate) fn place(&self, host: &gtk::Widget) {
        let mut frames = self.frames.borrow_mut();
        if let Some(frame) = frames.get_mut(host) {
            self.origins.borrow_mut().insert(host.clone(), frame.origin);
            frame.origin = Point::ZERO;
        }
    }

    /// The notebook's pages: the page hosts' wrappers, in order.
    fn wrappers(&self) -> Vec<gtk::Widget> {
        (0..self.notebook.n_pages()).filter_map(|i| self.notebook.nth_page(Some(i))).collect()
    }

    /// The page hosts, in order.
    pub(crate) fn pages(&self) -> Vec<gtk::Widget> {
        self.wrappers().iter().filter_map(WidgetExt::first_child).collect()
    }

    /// The tabs' titles, as they show them.
    pub(crate) fn titles(&self) -> Vec<String> {
        self.wrappers()
            .iter()
            .map(|page| self.notebook.tab_label_text(page).map(|t| t.to_string()).unwrap_or_default())
            .collect()
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        self.notebook.current_page().map(|i| i as usize)
    }

    /// Shows the page of the first tab with this title, as a click on it
    /// does, which the notebook reports if it wasn't shown. `false` if
    /// there's none.
    pub(crate) fn choose(&self, title: &str) -> bool {
        let Some(index) = self.titles().iter().position(|t| t == title) else { return false };
        self.notebook.set_current_page(Some(index as u32));
        true
    }

    /// Where the notebook shows a page, relative to itself, at the size the
    /// core gave it; nowhere if it shows another. Before the notebook has
    /// placed it (its window isn't shown yet), where the core put it.
    pub(crate) fn page_frame(&self, host: &gtk::Widget, size: Size) -> Rect {
        let Some(wrapper) = host.parent() else { return Rect::ZERO };
        let index = self.notebook.page_num(&wrapper);
        if index.is_none() || index != self.notebook.current_page() {
            return Rect::ZERO;
        }
        let at = host
            .is_mapped()
            .then(|| host.compute_point(&self.notebook, &graphene::Point::new(0.0, 0.0)))
            .flatten()
            .map(|p| Point::new(p.x(), p.y()));
        let origin = at.or_else(|| self.origins.borrow().get(host).copied()).unwrap_or(Point::ZERO);
        Rect { origin, size }
    }
}

/// Where a notebook's page area is: measured from a throwaway notebook
/// with a page much larger than its tab, allocated at its size. If GTK
/// can't place it, the border is taken as even all round.
pub(crate) fn insets() -> Insets {
    const SIDE: i32 = 1000;
    let notebook = gtk::Notebook::new();
    let page = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    page.set_size_request(SIDE, SIDE);
    notebook.append_page(&page, Some(&gtk::Label::new(Some("Tab"))));
    let width = notebook.measure(gtk::Orientation::Horizontal, -1).1;
    let height = notebook.measure(gtk::Orientation::Vertical, width).1;
    notebook.allocate(width, height, -1, None);
    let (width, height) = (width as f32, height as f32);
    match page.compute_bounds(&notebook) {
        Some(b) if b.width() > 0.0 => Insets::new(b.y(), width - b.x() - b.width(), height - b.y() - b.height(), b.x()),
        _ => {
            let border = (width - SIDE as f32) / 2.0;
            Insets::new(height - SIDE as f32 - border, border, border, border)
        }
    }
}
