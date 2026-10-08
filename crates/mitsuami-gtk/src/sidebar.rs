//! A window's sidebar, as GNOME Settings has it: a list box in GTK's
//! `navigation-sidebar` style, in the sidebar page of libadwaita's
//! navigation split view, whose content page holds the window's content.
//! In a narrow window the split view collapses into a stack of the two
//! pages: choosing an item shows the content, whose header bar has a back
//! button.
//!
//! libadwaita 1.9's `AdwSidebar` is the list GNOME apps move to; it needs
//! a floor above the 1.4 the split view needs (Ubuntu 24.04 has 1.5).
//!
//! An item's subtitle is a dimmed caption under its title, as an
//! `AdwActionRow`'s; its context menu is its row's, as any widget's is
//! (`ContextMenu`). A click on a row chooses it (and in a collapsed split
//! view shows the content), so activating an item is a double-click.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use mitsuami_core::{EventValue, NodeId, SidebarItemData, SidebarSectionData, UiEvent};

use crate::host::{Events, Host};
use crate::services::ContextMenu;

/// A window narrower than this (in units of the text size) collapses its
/// split view, as libadwaita's own sidebar example does.
const COLLAPSE_WIDTH: f64 = 400.0;
const COLLAPSE: &str = "max-width: 400sp";

/// What the list shows, shared by the backend and its signal handlers.
#[derive(Default)]
struct Data {
    sections: Vec<SidebarSectionData>,
    items: Vec<SidebarItemData>,
    /// One row per item, in order, and its context menu.
    rows: Vec<gtk::ListBoxRow>,
    menus: Vec<ContextMenu>,
    /// The item shown selected: the list box can't be told "no change".
    selected: Option<usize>,
    /// The window's split view, once the sidebar is in one.
    split: Option<SplitParts>,
}

/// What the list's handlers change in the window's split view.
struct SplitParts {
    view: adw::NavigationSplitView,
    content: adw::NavigationPage,
    header: adw::HeaderBar,
    window_title: String,
}

impl Data {
    /// The content page is titled after the item chosen, as GNOME
    /// Settings' pages are, or the window without one.
    fn title_content(&self) {
        if let Some(split) = &self.split {
            let item = self.selected.and_then(|i| self.items.get(i)).map(|i| i.title.as_str());
            set_title(&split.content, &split.header, item.unwrap_or(&split.window_title));
        }
    }

    /// The section an item is in, and whether it's the section's first.
    fn section_of(&self, item: usize) -> Option<(usize, bool)> {
        let mut start = 0;
        for (index, section) in self.sections.iter().enumerate() {
            if item < start + section.items.len() {
                return Some((index, item == start));
            }
            start += section.items.len();
        }
        None
    }
}

/// The sidebar node's native parts: the list in its scrolled window.
pub(crate) struct Sidebar {
    id: NodeId,
    pub scrolled: gtk::ScrolledWindow,
    pub list: gtk::ListBox,
    data: Rc<RefCell<Data>>,
    events: Events,
    /// Shown as the app wants it, if it said. A split view shows its
    /// sidebar whenever it isn't collapsed; collapsed, it's the page shown.
    shown: Rc<Cell<Option<bool>>>,
}

impl Sidebar {
    pub(crate) fn new(id: NodeId, events: Events) -> Sidebar {
        let list = gtk::ListBox::new();
        list.add_css_class("navigation-sidebar");
        list.set_selection_mode(gtk::SelectionMode::Single);
        let data = Rc::new(RefCell::new(Data::default()));
        // A section's title heads it; sections without one are set apart
        // by a line, as GNOME Settings' are.
        let d = data.clone();
        list.set_header_func(move |row, _| {
            let data = d.borrow();
            let header = match data.section_of(row.index() as usize) {
                Some((section, true)) => match &data.sections[section].title {
                    Some(title) => {
                        let label = gtk::Label::new(Some(title));
                        label.set_xalign(0.0);
                        label.add_css_class("heading");
                        label.add_css_class("dim-label");
                        label.set_margin_start(12);
                        label.set_margin_top(if section == 0 { 6 } else { 18 });
                        label.set_margin_bottom(6);
                        Some(label.upcast::<gtk::Widget>())
                    }
                    None if section > 0 => Some(gtk::Separator::new(gtk::Orientation::Horizontal).upcast()),
                    None => None,
                },
                _ => None,
            };
            row.set_header(header.as_ref());
        });
        let (d, e) = (data.clone(), events.clone());
        list.connect_row_selected(move |list, row| {
            let mut data = d.borrow_mut();
            match row.map(|r| r.index() as usize) {
                // Ctrl+click takes the selection away; a sidebar keeps
                // one, as GNOME Settings' does.
                None if !e.is_muted() => {
                    if let Some(row) = data.selected.and_then(|i| data.rows.get(i)).cloned() {
                        drop(data);
                        list.select_row(Some(&row));
                    }
                }
                None => {}
                Some(item) if data.selected != Some(item) => {
                    data.selected = Some(item);
                    data.title_content();
                    // Muted while the backend selects it.
                    e.emit(id, UiEvent::Changed(EventValue::Index(item)));
                }
                Some(_) => {}
            }
        });
        // A double-click on a row activates its item; the first click chose it.
        let click = gtk::GestureClick::new();
        click.set_button(gtk::gdk::BUTTON_PRIMARY);
        let (l, e) = (list.clone(), events.clone());
        click.connect_released(move |_, presses, _, y| {
            if let (2, Some(row)) = (presses, l.row_at_y(y as i32)) {
                e.emit(id, UiEvent::SidebarItemActivated(row.index() as usize));
            }
        });
        list.add_controller(click);
        // Chosen in a collapsed split view, the content shows.
        let (d, e) = (data.clone(), events.clone());
        list.connect_row_activated(move |_, _| {
            if let (false, Some(split)) = (e.is_muted(), &d.borrow().split) {
                split.view.set_show_content(true);
            }
        });
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scrolled.set_child(Some(&list));
        Sidebar { id, scrolled, list, data, events, shown: Rc::default() }
    }

    /// Shown or hidden as the app wants: in a collapsed split view, the
    /// sidebar's page or the content's. A split view that isn't collapsed
    /// always shows its sidebar, so there it's kept for when it collapses.
    pub(crate) fn set_shown(&self, shown: bool) {
        self.shown.set(Some(shown));
        if let Some(split) = &self.data.borrow().split
            && split.view.is_collapsed()
        {
            split.view.set_show_content(!shown);
        }
    }

    /// As the split view shows it, collapsed; else as the app wants it.
    pub(crate) fn shown(&self) -> Option<bool> {
        match &self.data.borrow().split {
            Some(split) if split.view.is_collapsed() => Some(!split.view.shows_content()),
            _ => self.shown.get(),
        }
    }

    /// New items: rows already there show them in place, and the list
    /// gains or loses rows at its end, keeping the selected item, which
    /// the core sends again if it moved. A row that stays keeps the focus
    /// it has (a new title is a new label): GTK moves focus out of a row
    /// it removes only at the next frame, and from the start of the window.
    pub(crate) fn set_sections(&self, sections: Vec<SidebarSectionData>) {
        let (rows, added, removed, selected) = {
            let mut data = self.data.borrow_mut();
            data.items = sections.iter().flat_map(|s| s.items.iter().cloned()).collect();
            data.sections = sections;
            let kept = data.rows.len().min(data.items.len());
            let removed = data.rows.split_off(kept);
            data.menus.truncate(kept);
            let added: Vec<gtk::ListBoxRow> = data.items[kept..].iter().map(|_| gtk::ListBoxRow::new()).collect();
            for row in &added {
                let (events, id) = (self.events.clone(), self.id);
                let activate = move |item| events.emit(id, UiEvent::ContextMenuItem(item));
                let menu = ContextMenu::new(row.upcast_ref(), Rc::new(activate));
                data.rows.push(row.clone());
                data.menus.push(menu);
            }
            let Data { rows, menus, items, .. } = &mut *data;
            for ((row, menu), item) in rows.iter().zip(menus.iter_mut()).zip(items.iter()) {
                show_item(row, item);
                menu.set(row.upcast_ref(), &item.menu);
            }
            (data.rows.clone(), added, removed, data.selected.filter(|i| *i < data.items.len()))
        };
        // Focus in a row that goes moves to the last that stays.
        let focus = self.list.root().and_then(|r| r.focus());
        if let (Some(focus), Some(last)) = (focus, rows.last())
            && removed.iter().any(|r| focus == *r || focus.is_ancestor(r))
        {
            last.grab_focus();
        }
        for row in &removed {
            self.list.remove(row);
        }
        // Appending runs the header function, which reads the data.
        for row in &added {
            self.list.append(row);
        }
        self.list.invalidate_headers();
        self.set_selected(selected);
        self.data.borrow().title_content();
    }

    pub(crate) fn sections(&self) -> Vec<SidebarSectionData> {
        self.data.borrow().sections.clone()
    }

    pub(crate) fn set_selected(&self, item: Option<usize>) {
        let row = {
            let mut data = self.data.borrow_mut();
            data.selected = item;
            data.title_content();
            item.and_then(|i| data.rows.get(i)).cloned()
        };
        match row {
            Some(row) => self.list.select_row(Some(&row)),
            None => self.list.unselect_all(),
        }
    }

    /// The item the list shows selected.
    pub(crate) fn selected(&self) -> Option<usize> {
        self.list.selected_row().map(|r| r.index() as usize)
    }

    /// The actions of the item's menu that has this id, which choosing it
    /// in the menu activates.
    pub(crate) fn chooser(&self, id: u32) -> Option<gtk::gio::SimpleActionGroup> {
        let data = self.data.borrow();
        let item = data.items.iter().position(|i| mitsuami_core::services::menu_item_by_id(&i.menu, id).is_some())?;
        data.menus.get(item).map(ContextMenu::chooser)
    }

    /// Activates the chosen item, as a double-click on it does. `false` if
    /// none is chosen.
    pub(crate) fn activate(&self) -> bool {
        let Some(item) = self.selected() else { return false };
        self.events.emit(self.id, UiEvent::SidebarItemActivated(item));
        true
    }

    /// Selects the first item with this title as the user would, which the
    /// list reports: what a screen reader's select does. `false` if there's
    /// none.
    pub(crate) fn choose(&self, title: &str) -> bool {
        let row = {
            let data = self.data.borrow();
            data.items.iter().position(|i| i.title == title).and_then(|i| data.rows.get(i)).cloned()
        };
        let Some(row) = row else { return false };
        self.list.select_row(Some(&row));
        true
    }
}

/// Shows the item's icon, title and subtitle in a row.
fn show_item(row: &gtk::ListBoxRow, item: &SidebarItemData) {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    if let Some(icon) = &item.icon {
        content.append(&gtk::Image::from_icon_name(icon));
    }
    let label = |text: &str| {
        let label = gtk::Label::new(Some(text));
        label.set_xalign(0.0);
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        label
    };
    match &item.subtitle {
        Some(subtitle) => {
            let lines = gtk::Box::new(gtk::Orientation::Vertical, 0);
            lines.set_valign(gtk::Align::Center);
            lines.append(&label(&item.title));
            let second = label(subtitle);
            second.add_css_class("caption");
            second.add_css_class("dim-label");
            lines.append(&second);
            content.append(&lines);
        }
        None => content.append(&label(&item.title)),
    }
    row.set_child(Some(&content));
}

/// Titles a page and shows the title in its header bar. libadwaita wants
/// every page titled, so an untitled one takes the app's name, which its
/// header bar doesn't show.
fn set_title(page: &adw::NavigationPage, header: &adw::HeaderBar, title: &str) {
    header.set_show_title(!title.is_empty());
    if title.is_empty() {
        let name = gtk::glib::application_name().or_else(gtk::glib::prgname).unwrap_or_default();
        page.set_title(&name);
    } else {
        page.set_title(title);
    }
}

/// A window split in two: the sidebar's page, titled after the window, and
/// the content's, which takes the window's header bar with its toolbar
/// items and menu button.
pub(crate) struct Split {
    pub sidebar: NodeId,
    bin: adw::BreakpointBin,
    pub view: adw::NavigationSplitView,
    sidebar_page: adw::NavigationPage,
    sidebar_header: adw::HeaderBar,
    sidebar_view: adw::ToolbarView,
    content: adw::ToolbarView,
    data: Rc<RefCell<Data>>,
}

impl Split {
    /// Takes the window's content and header bar into the split view. The
    /// window's own title bar gives way to the pages' header bars.
    pub(crate) fn new(
        window: &gtk::Window,
        header: &adw::HeaderBar,
        host: &Host,
        id: NodeId,
        sidebar: &Sidebar,
    ) -> Split {
        let sidebar_view = adw::ToolbarView::new();
        let sidebar_header = adw::HeaderBar::new();
        sidebar_view.add_top_bar(&sidebar_header);
        sidebar_view.set_content(Some(&sidebar.scrolled));
        let title = window.title().map(|t| t.to_string()).unwrap_or_default();
        let sidebar_page = adw::NavigationPage::new(&sidebar_view, "");
        set_title(&sidebar_page, &sidebar_header, &title);
        window.set_child(None::<&gtk::Widget>);
        // As `AdwWindow` does: a title bar that isn't shown, so GTK adds
        // none of its own.
        let hidden = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        hidden.set_visible(false);
        window.set_titlebar(Some(&hidden));
        let content = adw::ToolbarView::new();
        content.add_top_bar(header);
        content.set_content(Some(host));
        let content_page = adw::NavigationPage::new(&content, "");
        let view = adw::NavigationSplitView::new();
        // Titled before a shown window realizes it, which libadwaita checks.
        sidebar.data.borrow_mut().split = Some(SplitParts {
            view: view.clone(),
            content: content_page.clone(),
            header: header.clone(),
            window_title: title,
        });
        sidebar.data.borrow().title_content();
        view.set_sidebar(Some(&sidebar_page));
        view.set_content(Some(&content_page));
        // Collapsed, the page shown is the user's (a back button, an item
        // chosen) or the app's, whose changes are muted.
        let (e, shown) = (sidebar.events.clone(), sidebar.shown.clone());
        view.connect_show_content_notify(move |view| {
            if view.is_collapsed() && !e.is_muted() {
                shown.set(Some(!view.shows_content()));
                e.emit(id, UiEvent::SidebarShownChanged(!view.shows_content()));
            }
        });
        // Collapsing, it shows the page the app wants.
        let shown = sidebar.shown.clone();
        view.connect_collapsed_notify(move |view| {
            if let (true, Some(shown)) = (view.is_collapsed(), shown.get()) {
                view.set_show_content(!shown);
            }
        });
        let bin = adw::BreakpointBin::new();
        // GNOME's smallest window, which a breakpoint bin needs as its
        // own minimum.
        bin.set_size_request(360, 294);
        bin.set_child(Some(&view));
        if let Ok(condition) = adw::BreakpointCondition::parse(COLLAPSE) {
            let breakpoint = adw::Breakpoint::new(condition);
            breakpoint.add_setter(&view, "collapsed", Some(&true.to_value()));
            bin.add_breakpoint(breakpoint);
        }
        window.set_child(Some(&bin));
        Split {
            sidebar: id,
            bin,
            view,
            sidebar_page,
            sidebar_header,
            sidebar_view,
            content,
            data: sidebar.data.clone(),
        }
    }

    /// Gives the window its content and title bar back.
    pub(crate) fn remove(self, window: &gtk::Window, header: &adw::HeaderBar, host: &Host) {
        self.data.borrow_mut().split = None;
        window.set_child(None::<&gtk::Widget>);
        self.bin.set_child(None::<&gtk::Widget>);
        self.content.set_content(None::<&gtk::Widget>);
        self.sidebar_view.set_content(None::<&gtk::Widget>);
        self.content.remove(header);
        header.set_show_title(true);
        window.set_titlebar(Some(header));
        window.set_child(Some(host));
    }

    pub(crate) fn set_title(&self, title: &str) {
        set_title(&self.sidebar_page, &self.sidebar_header, title);
        if let Some(split) = &mut self.data.borrow_mut().split {
            split.window_title = title.to_owned();
        }
        self.data.borrow().title_content();
    }

    /// The sidebar page, where it's shown.
    pub(crate) fn sidebar_page(&self) -> &adw::NavigationPage {
        &self.sidebar_page
    }

    /// How much wider the window is than its content: by the sidebar, whose
    /// width is a fraction of the window's, within limits (libadwaita's
    /// defaults: a quarter, 180 to 280), unless the window is so narrow the
    /// split collapses. The limits are in units of the text size, which
    /// libadwaita converts with the display's `gtk-xft-dpi`: a display
    /// without one (Broadway, an X server without a settings daemon) makes
    /// them 0, and the sidebar is as narrow as its own minimum.
    pub(crate) fn extra_width(&self, content: f32) -> f32 {
        let settings = Some(self.view.settings());
        let unit = self.view.sidebar_width_unit();
        let to_px = |length: f64| unit.to_px(length, settings.as_ref()).ceil() as f32;
        let own = self.sidebar_page.measure(gtk::Orientation::Horizontal, -1).0 as f32;
        let min = own.max(to_px(self.view.min_sidebar_width()));
        let max = min.max(to_px(self.view.max_sidebar_width()));
        let fraction = self.view.sidebar_width_fraction() as f32;
        let width = if fraction * (content + min) <= min {
            min
        } else if fraction * (content + max) >= max {
            max
        } else {
            fraction * content / (1.0 - fraction)
        };
        let collapse = adw::LengthUnit::Sp.to_px(COLLAPSE_WIDTH, settings.as_ref()) as f32;
        let collapsed = content + width <= collapse;
        if collapsed { 0.0 } else { width }
    }
}
