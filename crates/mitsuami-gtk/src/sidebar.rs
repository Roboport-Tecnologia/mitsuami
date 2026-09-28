//! A window's sidebar, as GNOME Settings has it: a list box in GTK's
//! `navigation-sidebar` style, in the sidebar page of libadwaita's
//! navigation split view, whose content page holds the window's content.
//! In a narrow window the split view collapses into a stack of the two
//! pages: choosing an item shows the content, whose header bar has a back
//! button.
//!
//! libadwaita 1.9's `AdwSidebar` is the list GNOME apps move to; it needs
//! a floor above the 1.4 the split view needs (Ubuntu 24.04 has 1.5).

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use mitsuami_core::{EventValue, NodeId, SidebarItemData, SidebarSectionData, UiEvent};

use crate::host::{Events, Host};

/// A window narrower than this collapses its split view, as libadwaita's
/// own sidebar example does.
const COLLAPSE: &str = "max-width: 400sp";

/// What the list shows, shared by the backend and its signal handlers.
#[derive(Default)]
struct Data {
    sections: Vec<SidebarSectionData>,
    items: Vec<SidebarItemData>,
    /// One row per item, in order.
    rows: Vec<gtk::ListBoxRow>,
    /// The item shown selected: the list box can't be told "no change".
    selected: Option<usize>,
    /// The window's split view, once the sidebar is in one.
    split: Option<(adw::NavigationSplitView, adw::NavigationPage)>,
}

impl Data {
    /// The content page is titled after the item chosen, as GNOME
    /// Settings' pages are.
    fn title_content(&self) {
        if let Some((_, content)) = &self.split {
            let title = self.selected.and_then(|i| self.items.get(i)).map_or("", |i| i.title.as_str());
            content.set_title(title);
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
    pub scrolled: gtk::ScrolledWindow,
    pub list: gtk::ListBox,
    data: Rc<RefCell<Data>>,
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
        // Chosen in a collapsed split view, the content shows.
        let (d, e) = (data.clone(), events);
        list.connect_row_activated(move |_, _| {
            if let (false, Some((view, _))) = (e.is_muted(), &d.borrow().split) {
                view.set_show_content(true);
            }
        });
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scrolled.set_child(Some(&list));
        Sidebar { scrolled, list, data }
    }

    /// New items: the list is filled again, keeping the selected item,
    /// which the core sends again if it moved.
    pub(crate) fn set_sections(&self, sections: Vec<SidebarSectionData>) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        let (rows, selected) = {
            let mut data = self.data.borrow_mut();
            data.items = sections.iter().flat_map(|s| s.items.iter().cloned()).collect();
            data.sections = sections;
            data.rows = data.items.iter().map(row).collect();
            (data.rows.clone(), data.selected.filter(|i| *i < data.items.len()))
        };
        // Appending runs the header function, which reads the data.
        for row in &rows {
            self.list.append(row);
        }
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

/// A row: the item's icon and title.
fn row(item: &SidebarItemData) -> gtk::ListBoxRow {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    if let Some(icon) = &item.icon {
        content.append(&gtk::Image::from_icon_name(icon));
    }
    let label = gtk::Label::new(Some(&item.title));
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    content.append(&label);
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&content));
    row
}

/// A window split in two: the sidebar's page, titled after the window, and
/// the content's, which takes the window's header bar with its toolbar
/// items and menu button.
pub(crate) struct Split {
    pub sidebar: NodeId,
    bin: adw::BreakpointBin,
    pub view: adw::NavigationSplitView,
    sidebar_page: adw::NavigationPage,
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
        sidebar_view.add_top_bar(&adw::HeaderBar::new());
        sidebar_view.set_content(Some(&sidebar.scrolled));
        let title = window.title().map(|t| t.to_string()).unwrap_or_default();
        let sidebar_page = adw::NavigationPage::new(&sidebar_view, &title);
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
        view.set_sidebar(Some(&sidebar_page));
        view.set_content(Some(&content_page));
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
        sidebar.data.borrow_mut().split = Some((view.clone(), content_page));
        sidebar.data.borrow().title_content();
        Split { sidebar: id, bin, view, sidebar_page, sidebar_view, content, data: sidebar.data.clone() }
    }

    /// Gives the window its content and title bar back.
    pub(crate) fn remove(self, window: &gtk::Window, header: &adw::HeaderBar, host: &Host) {
        self.data.borrow_mut().split = None;
        window.set_child(None::<&gtk::Widget>);
        self.bin.set_child(None::<&gtk::Widget>);
        self.content.set_content(None::<&gtk::Widget>);
        self.sidebar_view.set_content(None::<&gtk::Widget>);
        self.content.remove(header);
        window.set_titlebar(Some(header));
        window.set_child(Some(host));
    }

    pub(crate) fn set_title(&self, title: &str) {
        self.sidebar_page.set_title(title);
    }

    /// The sidebar page, where it's shown.
    pub(crate) fn sidebar_page(&self) -> &adw::NavigationPage {
        &self.sidebar_page
    }

    /// How much wider the window is than its content: by the sidebar, whose
    /// width is a fraction of the window's, within limits (libadwaita's
    /// defaults: a quarter, 180 to 280), unless the window is so narrow the
    /// split collapses. Its units are taken as points, at the default text
    /// size.
    pub(crate) fn extra_width(&self, content: f32) -> f32 {
        let (min, max, fraction) = (
            self.view.min_sidebar_width() as f32,
            self.view.max_sidebar_width() as f32,
            self.view.sidebar_width_fraction() as f32,
        );
        let width = if fraction * (content + min) <= min {
            min
        } else if fraction * (content + max) >= max {
            max
        } else {
            fraction * content / (1.0 - fraction)
        };
        let collapsed = content + width <= 400.0;
        if collapsed { 0.0 } else { width }
    }
}
