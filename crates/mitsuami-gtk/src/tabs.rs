//! A tab view: libadwaita's inline view switcher over an `adw::ViewStack`,
//! as GNOME apps switch between panes inside a window, or a
//! `gtk::Notebook` with `TabsStyle::TabBar` or libadwaita before 1.7. Each
//! page host is a page, titled from the node's titles. Both stretch a
//! page's child over their page area, so each host sits in a host of its
//! own, at the top left, at the size the core gave it. The view is in a
//! box that stands for the node, so a new style makes a new view in it,
//! with the same pages.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use gtk::glib::translate::from_glib;
use gtk::prelude::*;
use gtk::{glib, graphene};
use mitsuami_core::{EventValue, Insets, NodeId, Point, Rect, Size, TabsStyle, UiEvent};

use crate::host::{Events, Frames, Host};

/// The tab view node's native parts, and the core's view of them.
pub(crate) struct Tabs {
    root: gtk::Box,
    view: RefCell<View>,
    style: Cell<Option<TabsStyle>>,
    changed: Rc<dyn Fn(usize)>,
    frames: Frames,
    titles: RefCell<Vec<String>>,
    /// Icons by page, empty for none: a view stack page's icon name. A
    /// notebook's tabs are text, so there they're only kept.
    icons: RefCell<Vec<String>>,
    /// The page the core shows, which may come before its page in a batch.
    selected: Cell<Option<usize>>,
    /// Where the core put each page, until the view has placed it.
    origins: RefCell<HashMap<gtk::Widget, Point>>,
}

impl Tabs {
    pub(crate) fn new(id: NodeId, events: Events, frames: Frames) -> Tabs {
        // It fires for the core's switches and for pages coming and going
        // too: the backend mutes events while it applies commands.
        let changed: Rc<dyn Fn(usize)> =
            Rc::new(move |index| events.emit(id, UiEvent::Changed(EventValue::Index(index))));
        let view = View::new(Kind::of(TabsStyle::Automatic), changed.clone());
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(view.root());
        Tabs {
            root,
            view: RefCell::new(view),
            style: Cell::new(None),
            changed,
            frames,
            titles: RefCell::default(),
            icons: RefCell::default(),
            selected: Cell::new(None),
            origins: RefCell::default(),
        }
    }

    /// The widget the node is.
    pub(crate) fn root(&self) -> &gtk::Widget {
        self.root.upcast_ref()
    }

    /// Shows its tabs another way: a new view, with the same pages, titles
    /// and page shown. The core's switches are muted, so the new view's
    /// isn't reported.
    pub(crate) fn set_style(&self, style: TabsStyle) {
        self.style.set(Some(style));
        let kind = Kind::of(style);
        if self.view.borrow().kind() == kind {
            return;
        }
        // GTK keeps the focus on a text field that moves with its page,
        // but until it's focused again the window never finished drawing
        // (a capture found nothing drawn): it's focused again.
        let focus = self.root.root().and_then(|r| r.focus()).filter(|f| f.is_ancestor(&self.root));
        let new = View::new(kind, self.changed.clone());
        let old = self.view.replace(new);
        let wrappers = old.wrappers();
        for wrapper in &wrappers {
            old.remove(wrapper);
        }
        self.root.remove(old.root());
        let view = self.view.borrow();
        self.root.append(view.root());
        for (index, wrapper) in wrappers.iter().enumerate() {
            view.insert(index, wrapper, self.titles.borrow().get(index).map_or("", String::as_str));
        }
        drop(view);
        self.retitle();
        self.show_selected();
        if let Some(focus) = focus {
            focus.grab_focus();
        }
    }

    /// The style the app chose, as the view shows it: `Automatic` and
    /// `Navigation` are navigation tabs, unless libadwaita has none.
    pub(crate) fn style(&self) -> Option<TabsStyle> {
        let chosen = self.style.get()?;
        let shown = self.view.borrow().kind();
        Some(match shown {
            _ if Kind::of(chosen) == shown => chosen,
            Kind::TabBar => TabsStyle::TabBar,
            Kind::Navigation => TabsStyle::Navigation,
        })
    }

    /// Where its pages go, as its view has them.
    pub(crate) fn insets(&self) -> Insets {
        insets(self.view.borrow().kind())
    }

    /// Titles pages by index, whichever of the two came first.
    pub(crate) fn set_titles(&self, titles: Vec<String>) {
        *self.titles.borrow_mut() = titles;
        self.retitle();
    }

    /// Icons pages by index, as titles.
    pub(crate) fn set_icons(&self, icons: Vec<String>) {
        *self.icons.borrow_mut() = icons;
        self.retitle();
    }

    /// Titles and icons each page, which a view stack loses when it takes
    /// pages out to insert one.
    fn retitle(&self) {
        let (titles, icons) = (self.titles.borrow(), self.icons.borrow());
        let view = self.view.borrow();
        for (index, page) in view.wrappers().iter().enumerate() {
            view.set_title(page, titles.get(index).map_or("", String::as_str));
            view.set_icon(page, icons.get(index).map_or("", String::as_str));
        }
        view.show_icons(icons.iter().any(|i| !i.is_empty()));
    }

    /// The tabs' icons, as a view stack shows them; a notebook's are kept.
    pub(crate) fn icons(&self) -> Vec<String> {
        let view = self.view.borrow();
        match &*view {
            View::Switcher { stack, .. } => view.wrappers().iter().map(|w| icon_name(stack, w)).collect(),
            View::Notebook(_) => self.icons.borrow().clone(),
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
        if let Some(index) = self.selected.get().filter(|i| *i < self.view.borrow().wrappers().len()) {
            self.view.borrow().show(index);
        }
    }

    pub(crate) fn insert(&self, index: usize, host: &gtk::Widget) {
        let wrapper = Host::new(self.frames.clone(), None);
        host.set_parent(&wrapper);
        self.place(host);
        self.view.borrow().insert(
            index,
            wrapper.upcast_ref(),
            self.titles.borrow().get(index).map_or("", String::as_str),
        );
        self.retitle();
        self.show_selected();
    }

    pub(crate) fn remove(&self, host: &gtk::Widget) {
        let Some(wrapper) = host.parent() else { return };
        host.unparent();
        self.view.borrow().remove(&wrapper);
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
        self.view.borrow().wrappers().iter().filter_map(WidgetExt::first_child).collect()
    }

    /// The tabs' titles, as they show them.
    pub(crate) fn titles(&self) -> Vec<String> {
        self.view.borrow().wrappers().iter().map(|page| self.view.borrow().title(page)).collect()
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        self.view.borrow().current()
    }

    /// Shows the page of the first tab with this title, as a click on it
    /// does, which the view reports if it wasn't shown. `false` if there's
    /// none.
    pub(crate) fn choose(&self, title: &str) -> bool {
        let Some(index) = self.titles().iter().position(|t| t == title) else { return false };
        self.view.borrow().show(index);
        true
    }

    /// Gives the tabs keyboard focus.
    pub(crate) fn focus(&self) -> bool {
        self.view.borrow().focus()
    }

    /// Where the view shows a page, relative to itself, at the size the
    /// core gave it; nowhere if it shows another. Before the view has
    /// placed it (its window isn't shown yet), where the core put it.
    pub(crate) fn page_frame(&self, host: &gtk::Widget, size: Size) -> Rect {
        let Some(wrapper) = host.parent() else { return Rect::ZERO };
        let index = self.view.borrow().wrappers().iter().position(|w| *w == wrapper);
        if index.is_none() || index != self.view.borrow().current() {
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

/// Which view a style shows, named as the styles are: libadwaita's switcher
/// for navigation tabs, GTK's notebook for a tab bar.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Navigation,
    TabBar,
}

impl Kind {
    /// A tab bar where libadwaita has no switcher.
    fn of(style: TabsStyle) -> Kind {
        match style {
            TabsStyle::Automatic | TabsStyle::Navigation if switcher_type().is_some() => Kind::Navigation,
            _ => Kind::TabBar,
        }
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
    fn new(kind: Kind, changed: Rc<dyn Fn(usize)>) -> View {
        let switcher = switcher_type().filter(|_| kind == Kind::Navigation);
        let Some(switcher) = switcher.map(|ty| glib::Object::with_type(ty).downcast::<gtk::Widget>().unwrap()) else {
            let notebook = gtk::Notebook::new();
            notebook.set_vexpand(true);
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
        root.set_vexpand(true);
        View::Switcher { root, switcher, stack }
    }

    fn kind(&self) -> Kind {
        match self {
            View::Switcher { .. } => Kind::Navigation,
            View::Notebook(_) => Kind::TabBar,
        }
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

    fn set_icon(&self, wrapper: &gtk::Widget, icon: &str) {
        if let View::Switcher { stack, .. } = self
            && icon_name(stack, wrapper) != icon
        {
            stack.page(wrapper).set_icon_name(Some(icon).filter(|i| !i.is_empty()));
        }
    }

    /// An inline view switcher shows labels unless told to show icons too.
    fn show_icons(&self, icons: bool) {
        let View::Switcher { switcher, .. } = self else { return };
        // Its mode is an enum of the switcher's, which is looked up at run
        // time too.
        let mode = switcher
            .find_property("display-mode")
            .and_then(|p| glib::EnumClass::with_type(p.value_type()))
            .and_then(|class| class.to_value_by_nick(if icons { "both" } else { "labels" }));
        if let Some(mode) = mode {
            switcher.set_property_from_value("display-mode", &mode);
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
/// has one (1.7 and later). It's looked up at run time, so the backend
/// builds against libadwaita 1.4.
fn switcher_type() -> Option<glib::Type> {
    static TYPE: OnceLock<Option<glib::Type>> = OnceLock::new();
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

/// A view stack page's icon name; empty for none.
fn icon_name(stack: &adw::ViewStack, wrapper: &gtk::Widget) -> String {
    stack.page(wrapper).icon_name().map(|i| i.to_string()).unwrap_or_default()
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

/// Where the default tab view's page area is, for the metrics.
pub(crate) fn default_insets() -> Insets {
    insets(Kind::of(TabsStyle::Automatic))
}

/// Where a tab view's page area is, measured once for each kind.
fn insets(kind: Kind) -> Insets {
    thread_local! {
        static MEASURED: RefCell<Vec<(Kind, Insets)>> = const { RefCell::new(Vec::new()) };
    }
    if let Some(insets) = MEASURED.with(|m| m.borrow().iter().find(|(k, _)| *k == kind).map(|(_, i)| *i)) {
        return insets;
    }
    let insets = measure_insets(kind);
    MEASURED.with(|m| m.borrow_mut().push((kind, insets)));
    insets
}

/// Measured from a throwaway view with a page much larger than its tab,
/// allocated at its size. If GTK can't place it, the border is taken as
/// even all round.
fn measure_insets(kind: Kind) -> Insets {
    const SIDE: i32 = 1000;
    let view = View::new(kind, Rc::new(|_| {}));
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
