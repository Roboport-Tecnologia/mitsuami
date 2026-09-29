//! A tab view: libadwaita's inline view switcher over an `adw::ViewStack`,
//! as GNOME apps switch between panes inside a window, or a
//! `gtk::Notebook` with libadwaita before 1.7 or the `notebook-tabs`
//! feature. Each page host is a page,
//! titled from the node's titles. Both stretch a page's child over their
//! page area, so each host sits in a host of its own, at the top left, at
//! the size the core gave it.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::OnceLock;

use gtk::glib::translate::from_glib;
use gtk::prelude::*;
use gtk::{glib, graphene};
use mitsuami_core::{EventValue, Insets, NodeId, Point, Rect, Size, UiEvent};

use crate::host::{Events, Frames, Host};

/// The tab view node's native parts, and the core's view of them.
pub(crate) struct Tabs {
    view: View,
    frames: Frames,
    titles: RefCell<Vec<String>>,
    /// The page the core shows, which may come before its page in a batch.
    selected: Cell<Option<usize>>,
    /// Where the core put each page, until the view has placed it.
    origins: RefCell<HashMap<gtk::Widget, Point>>,
}

impl Tabs {
    pub(crate) fn new(id: NodeId, events: Events, frames: Frames) -> Tabs {
        // It fires for the core's switches and for pages coming and going
        // too: the backend mutes events while it applies commands.
        let view = View::new(move |index| events.emit(id, UiEvent::Changed(EventValue::Index(index))));
        Tabs { view, frames, titles: RefCell::default(), selected: Cell::new(None), origins: RefCell::default() }
    }

    /// The widget the node is.
    pub(crate) fn root(&self) -> &gtk::Widget {
        self.view.root()
    }

    /// Titles pages by index, whichever of the two came first.
    pub(crate) fn set_titles(&self, titles: Vec<String>) {
        *self.titles.borrow_mut() = titles;
        self.retitle();
    }

    fn retitle(&self) {
        let titles = self.titles.borrow();
        for (index, page) in self.view.wrappers().iter().enumerate() {
            self.view.set_title(page, titles.get(index).map_or("", String::as_str));
        }
    }

    pub(crate) fn set_selected(&self, index: Option<usize>) {
        self.selected.set(index);
        self.show_selected();
    }

    /// Shows the core's page, once it's there: the view shows its first
    /// page when it gets one, and keeps showing a page others are put
    /// before.
    fn show_selected(&self) {
        if let Some(index) = self.selected.get().filter(|i| *i < self.view.wrappers().len()) {
            self.view.show(index);
        }
    }

    pub(crate) fn insert(&self, index: usize, host: &gtk::Widget) {
        let wrapper = Host::new(self.frames.clone(), None);
        host.set_parent(&wrapper);
        self.place(host);
        self.view.insert(index, wrapper.upcast_ref(), self.titles.borrow().get(index).map_or("", String::as_str));
        self.retitle();
        self.show_selected();
    }

    pub(crate) fn remove(&self, host: &gtk::Widget) {
        let Some(wrapper) = host.parent() else { return };
        host.unparent();
        self.view.remove(&wrapper);
        // It goes back to its place in its next parent.
        if let Some(origin) = self.origins.borrow_mut().remove(host)
            && let Some(frame) = self.frames.borrow_mut().get_mut(host)
        {
            frame.origin = origin;
        }
        self.retitle();
        self.show_selected();
    }

    /// Keeps a page's size and puts it at the top left of its wrapper: the
    /// view places the wrapper. Called when a page's frame changes.
    pub(crate) fn place(&self, host: &gtk::Widget) {
        let mut frames = self.frames.borrow_mut();
        if let Some(frame) = frames.get_mut(host) {
            self.origins.borrow_mut().insert(host.clone(), frame.origin);
            frame.origin = Point::ZERO;
        }
    }

    /// The page hosts, in order.
    pub(crate) fn pages(&self) -> Vec<gtk::Widget> {
        self.view.wrappers().iter().filter_map(WidgetExt::first_child).collect()
    }

    /// The tabs' titles, as they show them.
    pub(crate) fn titles(&self) -> Vec<String> {
        self.view.wrappers().iter().map(|page| self.view.title(page)).collect()
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        self.view.current()
    }

    /// Shows the page of the first tab with this title, as a click on it
    /// does, which the view reports if it wasn't shown. `false` if there's
    /// none.
    pub(crate) fn choose(&self, title: &str) -> bool {
        let Some(index) = self.titles().iter().position(|t| t == title) else { return false };
        self.view.show(index);
        true
    }

    /// Gives the tabs keyboard focus.
    pub(crate) fn focus(&self) -> bool {
        self.view.focus()
    }

    /// Where the view shows a page, relative to itself, at the size the
    /// core gave it; nowhere if it shows another. Before the view has
    /// placed it (its window isn't shown yet), where the core put it.
    pub(crate) fn page_frame(&self, host: &gtk::Widget, size: Size) -> Rect {
        let Some(wrapper) = host.parent() else { return Rect::ZERO };
        let index = self.view.wrappers().iter().position(|w| *w == wrapper);
        if index.is_none() || index != self.view.current() {
            return Rect::ZERO;
        }
        let at = host
            .is_mapped()
            .then(|| host.compute_point(self.root(), &graphene::Point::new(0.0, 0.0)))
            .flatten()
            .map(|p| Point::new(p.x(), p.y()));
        let origin = at.or_else(|| self.origins.borrow().get(host).copied()).unwrap_or(Point::ZERO);
        Rect { origin, size }
    }
}

/// The native tab view: libadwaita's inline view switcher, centred over
/// its view stack, as in libadwaita's demo and GNOME's apps; or GTK's
/// notebook, a `gtk::Label` for each tab.
enum View {
    Switcher { root: gtk::Box, switcher: gtk::Widget, stack: adw::ViewStack },
    Notebook(gtk::Notebook),
}

impl View {
    fn new(changed: impl Fn(usize) + 'static) -> View {
        let Some(switcher) = switcher_type().map(|ty| glib::Object::with_type(ty).downcast::<gtk::Widget>().unwrap())
        else {
            let notebook = gtk::Notebook::new();
            notebook.connect_switch_page(move |_, _, index| changed(index as usize));
            return View::Notebook(notebook);
        };
        let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let stack = adw::ViewStack::new();
        stack.set_vexpand(true);
        switcher.set_property("stack", &stack);
        switcher.set_halign(gtk::Align::Center);
        root.append(&switcher);
        root.append(&stack);
        stack.connect_visible_child_notify(move |stack| {
            if let Some(index) = current(stack) {
                changed(index)
            }
        });
        View::Switcher { root, switcher, stack }
    }

    fn root(&self) -> &gtk::Widget {
        match self {
            View::Switcher { root, .. } => root.upcast_ref(),
            View::Notebook(notebook) => notebook.upcast_ref(),
        }
    }

    fn wrappers(&self) -> Vec<gtk::Widget> {
        match self {
            View::Switcher { stack, .. } => pages(stack).iter().map(adw::ViewStackPage::child).collect(),
            View::Notebook(notebook) => (0..notebook.n_pages()).filter_map(|i| notebook.nth_page(Some(i))).collect(),
        }
    }

    /// A view stack only appends: the pages after `index` go after it
    /// again, and the page shown stays shown.
    fn insert(&self, index: usize, wrapper: &gtk::Widget, title: &str) {
        match self {
            View::Switcher { stack, .. } => {
                let shown = stack.visible_child();
                let after: Vec<(gtk::Widget, String)> = pages(stack)
                    .iter()
                    .skip(index)
                    .map(|p| (p.child(), p.title().map(|t| t.to_string()).unwrap_or_default()))
                    .collect();
                for (child, _) in &after {
                    stack.remove(child);
                }
                stack.add_titled(wrapper, None, title);
                for (child, title) in &after {
                    stack.add_titled(child, None, title);
                }
                if let Some(shown) = shown {
                    stack.set_visible_child(&shown);
                }
            }
            View::Notebook(notebook) => {
                notebook.insert_page(wrapper, Some(&gtk::Label::new(Some(title))), Some(index as u32));
            }
        }
    }

    fn remove(&self, wrapper: &gtk::Widget) {
        match self {
            View::Switcher { stack, .. } => {
                if wrapper.parent().as_ref() == Some(stack.upcast_ref()) {
                    stack.remove(wrapper);
                }
            }
            View::Notebook(notebook) => {
                if let Some(index) = notebook.page_num(wrapper) {
                    notebook.remove_page(Some(index));
                }
            }
        }
    }

    fn set_title(&self, wrapper: &gtk::Widget, title: &str) {
        match self {
            View::Switcher { stack, .. } => {
                let page = stack.page(wrapper);
                if page.title().as_deref() != Some(title) {
                    page.set_title(Some(title));
                }
            }
            View::Notebook(notebook) => match notebook.tab_label(wrapper).and_downcast::<gtk::Label>() {
                Some(label) if label.label() != title => label.set_label(title),
                Some(_) => {}
                None => notebook.set_tab_label(wrapper, Some(&gtk::Label::new(Some(title)))),
            },
        }
    }

    fn title(&self, wrapper: &gtk::Widget) -> String {
        match self {
            View::Switcher { stack, .. } => stack.page(wrapper).title(),
            View::Notebook(notebook) => notebook.tab_label_text(wrapper),
        }
        .map(|t| t.to_string())
        .unwrap_or_default()
    }

    fn current(&self) -> Option<usize> {
        match self {
            View::Switcher { stack, .. } => current(stack),
            View::Notebook(notebook) => notebook.current_page().map(|i| i as usize),
        }
    }

    fn show(&self, index: usize) {
        match self {
            View::Switcher { stack, .. } => {
                if let Some(wrapper) = self.wrappers().get(index) {
                    stack.set_visible_child(wrapper);
                }
            }
            View::Notebook(notebook) => notebook.set_current_page(Some(index as u32)),
        }
    }

    /// The switcher's toggles take focus, the selected one first; a
    /// notebook's tabs, the notebook itself.
    fn focus(&self) -> bool {
        match self {
            View::Switcher { switcher, .. } => switcher.child_focus(gtk::DirectionType::TabForward),
            View::Notebook(notebook) => notebook.grab_focus(),
        }
    }
}

/// libadwaita's inline view switcher, if the libadwaita the app runs with
/// has one (1.7 and later) and the app didn't ask for notebooks. It's
/// looked up at run time, so the backend builds against libadwaita 1.4.
fn switcher_type() -> Option<glib::Type> {
    static TYPE: OnceLock<Option<glib::Type>> = OnceLock::new();
    if cfg!(feature = "notebook-tabs") {
        return None;
    }
    *TYPE.get_or_init(|| {
        // SAFETY: the symbol, if libadwaita has it, is its type's getter,
        // `GType adw_inline_view_switcher_get_type(void)`.
        unsafe {
            let getter = libc::dlsym(libc::RTLD_DEFAULT, c"adw_inline_view_switcher_get_type".as_ptr());
            (!getter.is_null()).then(|| {
                let getter: extern "C" fn() -> glib::ffi::GType = std::mem::transmute(getter);
                from_glib(getter())
            })
        }
    })
}

/// A view stack's pages, in order.
fn pages(stack: &adw::ViewStack) -> Vec<adw::ViewStackPage> {
    let pages = stack.pages();
    (0..pages.n_items()).filter_map(|i| pages.item(i).and_downcast()).collect()
}

/// The index of a view stack's page shown.
fn current(stack: &adw::ViewStack) -> Option<usize> {
    let shown = stack.visible_child()?;
    pages(stack).iter().position(|p| p.child() == shown)
}

/// Where a tab view's page area is: measured from a throwaway one with a
/// page much larger than its tab, allocated at its size. If GTK can't
/// place it, the border is taken as even all round.
pub(crate) fn insets() -> Insets {
    const SIDE: i32 = 1000;
    let view = View::new(|_| {});
    let page = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    page.set_size_request(SIDE, SIDE);
    view.insert(0, page.upcast_ref(), "Tab");
    let root = view.root();
    let width = root.measure(gtk::Orientation::Horizontal, -1).1;
    let height = root.measure(gtk::Orientation::Vertical, width).1;
    root.allocate(width, height, -1, None);
    let (width, height) = (width as f32, height as f32);
    match page.compute_bounds(root) {
        Some(b) if b.width() > 0.0 => Insets::new(b.y(), width - b.x() - b.width(), height - b.y() - b.height(), b.x()),
        _ => {
            let border = (width - SIDE as f32) / 2.0;
            Insets::new(height - SIDE as f32 - border, border, border, border)
        }
    }
}
