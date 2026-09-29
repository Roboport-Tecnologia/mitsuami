//! Built-in widgets. Platform-free: each is an [`Element`] with typed props,
//! events and accessibility defaults. Backends decide how they look.

use std::rc::Rc;

use mitsuami_core::services::{MenuEntries, install_button_menu};
use std::path::PathBuf;

use mitsuami_core::{
    Align, AnyView, ButtonRole, ButtonStyle, Children, Color, CurrentWindow, Cursor, Display, Element, ElementBuilder,
    EventValue, FileDrop, FlexDirection, FontWeight, ImageFit, ImageSource, Justify, Length, Modality, NodeId,
    Orientation, Pixels, Point, Prop, ScrollAxes, SidebarItemData, SidebarSectionData, Size, SurfaceHandle,
    SurfaceInput, SurfaceSize, TextAlign, TextStyle, Track, Tweak, Ui, UiEvent, View, WidgetKind, WindowSize,
};
use mitsuami_reactive::{IntoValue, Signal, Value, computed, effect, inject, on_cleanup, provide, untrack};

macro_rules! widget {
    ($t:ty) => {
        impl ElementBuilder for $t {
            fn element(&mut self) -> &mut Element {
                &mut self.0
            }
        }

        impl View for $t {
            fn build(self, ui: &Ui) -> NodeId {
                self.0.build(ui)
            }
        }
    };
}

// ---------------------------------------------------------------- windows

/// A window the app opens while it runs: shown while `open` is true, with
/// its content built when it opens and disposed when it closes. Declare it
/// anywhere in the tree, next to the state that opens it; it closes with
/// the scope that declared it, and belongs to the window it's declared in.
///
/// The close button asks: [`bind`](Self::bind) closes it by setting the
/// flag, or [`on_close_request`](Self::on_close_request) lets the app
/// decide (to ask about unsaved changes, say). Without either, the close
/// button does nothing.
///
/// ```ignore
/// let editing = signal(false);
/// Column::new().children((
///     Button::new("Edit…").on_click(move || editing.set(true)),
///     Window::new("Machine").bind(editing).content(|| machine_form()),
/// ))
/// ```
///
/// In `view!`, its title is an attribute and its children are its content:
/// `<Window title="Machine" bind=editing>…</Window>`.
pub struct Window<T = Value<String>> {
    title: T,
    size: Value<WindowSize>,
    modality: Value<Option<Modality>>,
    open: Value<bool>,
    full_screen: Option<Signal<bool>>,
    min_size: Option<Value<Size>>,
    on_open: Option<Rc<dyn Fn()>>,
    on_close_request: Option<Rc<dyn Fn()>>,
    content: Option<Rc<dyn Fn() -> AnyView>>,
}

impl Window {
    /// Open, 480 wide, as tall as its content.
    pub fn new(title: impl IntoValue<String>) -> Window {
        Window {
            title: title.into_value(),
            size: Value::Static(WindowSize::FitHeight(480.0)),
            modality: Value::Static(None),
            open: Value::Static(true),
            full_screen: None,
            min_size: None,
            on_open: None,
            on_close_request: None,
            content: None,
        }
    }
}

impl<T> Window<T> {
    /// Its content size when it opens: a [`Size`](mitsuami_core::Size),
    /// [`WindowSize::FitHeight`] to fit the height to the content, or
    /// [`WindowSize::FollowHeight`] to follow it as it changes. Read each
    /// time it opens, as its modality is, so a change applies at the next
    /// opening.
    pub fn size(mut self, size: impl IntoValue<WindowSize>) -> Window<T> {
        self.size = size.into_value();
        self
    }

    /// Modal: while it's open, it blocks the window it's declared in
    /// ([`Modality::Window`], a sheet on macOS) or the whole app
    /// ([`Modality::Application`]), and stays above that window. Set
    /// before it opens. Like any dialog, Escape asks it to close, as its
    /// close button does (on AppKit a `ButtonRole::Cancel` button takes
    /// Escape first). A sheet has no close button: give its content a
    /// way out, e.g. that Cancel button.
    pub fn modal(mut self, modality: Modality) -> Window<T> {
        self.modality = Value::Static(Some(modality));
        self
    }

    /// Modal or not, as a value that can change: read each time it opens,
    /// so a change applies at the next opening.
    pub fn modality(mut self, modality: impl IntoValue<Option<Modality>>) -> Window<T> {
        self.modality = modality.into_value();
        self
    }

    /// Shown while true.
    pub fn open(mut self, open: impl IntoValue<bool>) -> Window<T> {
        self.open = open.into_value();
        self
    }

    /// Shown while the signal is true; the close button sets it false.
    pub fn bind(self, open: Signal<bool>) -> Window<T> {
        self.open(open).on_close_request(move || open.set(false))
    }

    /// In full screen while `full_screen` is true, the platform's own way:
    /// a Space of its own on macOS, the whole screen elsewhere, with the
    /// platform's own way out. The user can change it too (the title bar's
    /// button on macOS, the window manager's key), which sets the signal.
    pub fn full_screen(mut self, full_screen: Signal<bool>) -> Window<T> {
        self.full_screen = Some(full_screen);
        self
    }

    /// The smallest content size the user can make it. Smaller when it's
    /// set, it grows to it.
    pub fn min_size(mut self, size: impl IntoValue<Size>) -> Window<T> {
        self.min_size = Some(size.into_value());
        self
    }

    /// Called each time it opens, before its content is built: to start a
    /// form from what's saved, say.
    pub fn on_open(mut self, handler: impl Fn() + 'static) -> Window<T> {
        self.on_open = Some(Rc::new(handler));
        self
    }

    /// Called when the user asks to close it (the close button, ⌘W, Alt+F4).
    /// It stays open unless the app closes it.
    pub fn on_close_request(mut self, handler: impl Fn() + 'static) -> Window<T> {
        self.on_close_request = Some(Rc::new(handler));
        self
    }

    /// What it shows, built each time it opens.
    pub fn content<V: View>(mut self, content: impl Fn() -> V + 'static) -> Window<T> {
        self.content = Some(Rc::new(move || AnyView::new(content())));
        self
    }
}

impl View for Window {
    /// A placeholder in the tree, which takes no room; the window is
    /// top-level.
    fn build(self, ui: &Ui) -> NodeId {
        let placeholder = ui.create(WidgetKind::Fragment, Vec::new());
        let Window { title, size, modality, open, full_screen, min_size, on_open, on_close_request, content } = self;
        // The window it's declared in, which a modal window belongs to.
        let owner = inject::<CurrentWindow>().map(|w| w.0);
        let ui = ui.clone();
        let show = move || {
            let window = ui.create_window(String::new(), size.get());
            if let Some(modality) = modality.get() {
                // With no window to block, it blocks the app.
                let modality = if owner.is_some() { modality } else { Modality::Application };
                ui.set_prop(window, Prop::Modal { owner, modality });
            }
            let title = title.clone();
            let titled = ui.clone();
            effect(move || titled.set_prop(window, Prop::Title(title.get())));
            if let Some(min_size) = min_size.clone() {
                let ui = ui.clone();
                effect(move || ui.set_prop(window, Prop::MinSize(min_size.get())));
            }
            if let Some(full_screen) = full_screen {
                let filled = ui.clone();
                effect(move || filled.set_prop(window, Prop::FullScreen(full_screen.get())));
                ui.on_event(window, move |event| {
                    if let UiEvent::FullScreenChanged(on) = event {
                        full_screen.set(*on);
                    }
                });
            }
            provide(CurrentWindow(window));
            if let Some(handler) = &on_open {
                handler();
            }
            if let Some(content) = &content {
                let root = content().build(&ui);
                ui.append_child(window, root);
            }
            if let Some(handler) = on_close_request.clone() {
                ui.on_event(window, move |event| {
                    if *event == UiEvent::WindowCloseRequested {
                        handler();
                    }
                });
            }
            let ui = ui.clone();
            on_cleanup(move || ui.destroy(window));
        };
        // In a scope of its own even when it's always open, so what it
        // provides (its `CurrentWindow`) stays inside it.
        let open = computed(move || open.get());
        effect(move || {
            if open.get() {
                untrack(&show);
            }
        });
        placeholder
    }
}

/// Items for the toolbar of the window it's declared in: the bar across
/// the top that shows the window's title (the unified toolbar on macOS,
/// GTK's header bar, a command bar on WinUI, the page toolbar on KDE).
/// Declare it anywhere in the window's content; its items stay while it
/// does.
///
/// Each child is one item, at the bar's trailing end, in order. The
/// platform places them, spaces them and draws the bar; each item is as
/// big as its content. An item with nothing in it (a `Show` that shows
/// nothing) is hidden.
///
/// ```ignore
/// Column::new().children((
///     Toolbar::new().children((
///         Show::new(move || busy.get(), || Row::new().children((Spinner::new("Downloading"), "Downloading"))),
///         Text::new(move || status.get()),
///     )),
///     machine_list(),
/// ))
/// ```
#[derive(Default)]
pub struct Toolbar {
    items: Vec<AnyView>,
}

impl Toolbar {
    pub fn new() -> Toolbar {
        Toolbar::default()
    }

    /// The items, one per child.
    pub fn children(mut self, children: impl Children) -> Toolbar {
        children.into_views(&mut self.items);
        self
    }

    pub fn child(self, child: impl View) -> Toolbar {
        self.children(child)
    }
}

impl View for Toolbar {
    /// A placeholder in the tree, which takes no room; the items are the
    /// window's.
    fn build(self, ui: &Ui) -> NodeId {
        let placeholder = ui.create(WidgetKind::Fragment, Vec::new());
        let Some(CurrentWindow(window)) = inject::<CurrentWindow>() else {
            panic!("a Toolbar goes in a window's content");
        };
        let items: Vec<NodeId> = self
            .items
            .into_iter()
            .map(|view| {
                let item = ui.create(WidgetKind::ToolbarItem, Vec::new());
                let content = view.build(ui);
                ui.append_child(item, content);
                ui.append_child(window, item);
                item
            })
            .collect();
        let ui = ui.clone();
        on_cleanup(move || {
            for item in items {
                ui.destroy(item);
            }
        });
        placeholder
    }
}

/// The sidebar of the window it's declared in: the list down its leading
/// side that picks what the window shows, as in macOS's System Settings
/// and GNOME's Settings (a source list on macOS, libadwaita's navigation
/// split view on GTK, a NavigationView's pane on Windows, a first column
/// of Kirigami's page row on KDE). Declare it anywhere in the window's
/// content; it stays while it does.
///
/// Its items are [`SidebarItem`]s, in [`SidebarSection`]s or on their own.
/// `selection` holds the value of the chosen item, and choosing another
/// sets it; a value no item has chooses none. The window's content is
/// what's beside the sidebar: show the page for the selection there. The
/// platform draws the sidebar, sizes it, and collapses it in a narrow
/// window its own way. The window's size is its content's, and the
/// sidebar is added to it.
///
/// ```ignore
/// let page = signal(Page::General);
/// Column::new().children((
///     Sidebar::new(page).children((
///         SidebarItem::new("General", Page::General).icon(platform! {
///             macos => "gearshape",
///             gtk | kde => "preferences-system-symbolic",
///             windows => "\u{E713}",
///         }),
///         SidebarSection::new("Network").children((
///             SidebarItem::new("Wi-Fi", Page::WiFi),
///             SidebarItem::new("Bluetooth", Page::Bluetooth),
///         )),
///     )),
///     Show::new(move || page.get() == Page::General, general),
/// ))
/// ```
///
/// In `view!`: `<Sidebar selection=page>`, with
/// `<SidebarItem value=Page::General>"General"</SidebarItem>`s and
/// `<SidebarSection title="Network">`s as children.
pub struct Sidebar<T: 'static> {
    selection: Signal<T>,
    sections: Vec<SidebarSection<T>>,
}

impl<T: PartialEq + Clone + 'static> Sidebar<T> {
    pub fn new(selection: Signal<T>) -> Sidebar<T> {
        Sidebar { selection, sections: Vec::new() }
    }

    /// Items and sections, in order. Items outside a section next to each
    /// other are one section without a heading.
    pub fn children(mut self, children: impl SidebarEntries<T>) -> Sidebar<T> {
        children.add_to(&mut self.sections);
        self
    }

    pub fn item(self, item: SidebarItem<T>) -> Sidebar<T> {
        self.children(item)
    }

    pub fn section(self, section: SidebarSection<T>) -> Sidebar<T> {
        self.children(section)
    }
}

impl<T: PartialEq + Clone + 'static> View for Sidebar<T> {
    /// A placeholder in the tree, which takes no room; the sidebar is the
    /// window's.
    fn build(self, ui: &Ui) -> NodeId {
        let placeholder = ui.create(WidgetKind::Fragment, Vec::new());
        let Some(CurrentWindow(window)) = inject::<CurrentWindow>() else {
            panic!("a Sidebar goes in a window's content");
        };
        let Sidebar { selection, sections } = self;
        let values: Rc<Vec<T>> =
            Rc::new(sections.iter().flat_map(|s| s.items.iter().map(|i| i.value.clone())).collect());
        let sidebar = ui.create(WidgetKind::Sidebar, Vec::new());
        ui.append_child(window, sidebar);
        // The items first: the selection is an index into them.
        let shown = ui.clone();
        effect(move || {
            let data = sections
                .iter()
                .map(|section| SidebarSectionData {
                    title: section.title.as_ref().map(|t| t.get()),
                    items: section
                        .items
                        .iter()
                        .map(|item| SidebarItemData { title: item.title.get(), icon: item.icon.clone() })
                        .collect(),
                })
                .collect();
            shown.set_prop(sidebar, Prop::Sections(data));
        });
        let (chosen, index_of) = (ui.clone(), values.clone());
        effect(move || {
            let index = selection.with(|value| index_of.iter().position(|v| v == value));
            chosen.set_prop(sidebar, Prop::SelectedIndex(index));
        });
        ui.on_event(sidebar, move |event| {
            if let UiEvent::Changed(EventValue::Index(index)) = event
                && let Some(value) = values.get(*index)
            {
                selection.set(value.clone());
            }
        });
        let ui = ui.clone();
        on_cleanup(move || ui.destroy(sidebar));
        placeholder
    }
}

/// A group of a [`Sidebar`]'s items, under a heading.
pub struct SidebarSection<T: 'static> {
    title: Option<Value<String>>,
    items: Vec<SidebarItem<T>>,
    /// Made for items outside any section, so the next ones join it.
    loose: bool,
}

impl<T: 'static> SidebarSection<T> {
    pub fn new(title: impl IntoValue<String>) -> SidebarSection<T> {
        SidebarSection { title: Some(title.into_value()), items: Vec::new(), loose: false }
    }

    /// Without a heading.
    pub fn untitled() -> SidebarSection<T> {
        SidebarSection { title: None, items: Vec::new(), loose: false }
    }

    pub fn title(mut self, title: impl IntoValue<String>) -> SidebarSection<T> {
        self.title = Some(title.into_value());
        self
    }

    pub fn item(mut self, item: SidebarItem<T>) -> SidebarSection<T> {
        self.items.push(item);
        self
    }

    /// Its items, in order.
    pub fn children(mut self, items: impl SidebarItems<T>) -> SidebarSection<T> {
        items.add_to(&mut self.items);
        self
    }
}

/// An item of a [`Sidebar`]: a title, the value choosing it gives the
/// sidebar's selection, and optionally an icon.
///
/// In `view!`, its text is its title:
/// `<SidebarItem value=Page::General icon="gearshape">"General"</SidebarItem>`.
pub struct SidebarItem<T: 'static> {
    title: Value<String>,
    icon: Option<String>,
    value: T,
}

impl<T: 'static> SidebarItem<T> {
    pub fn new(title: impl IntoValue<String>, value: T) -> SidebarItem<T> {
        SidebarItem { title: title.into_value(), icon: None, value }
    }

    /// The value choosing it gives the sidebar's selection.
    pub fn value<U: 'static>(self, value: U) -> SidebarItem<U> {
        SidebarItem { title: self.title, icon: self.icon, value }
    }

    /// The name of its icon in the platform's own set, picked with
    /// `platform!`: an SF Symbol on macOS (`"gearshape"`), a symbolic icon
    /// of the theme on GTK and KDE (`"preferences-system-symbolic"`), a
    /// Segoe Fluent Icons glyph on Windows (`"\u{E713}"`). Empty: none.
    /// A platform that can't find it shows none.
    pub fn icon(mut self, name: impl Into<String>) -> SidebarItem<T> {
        self.icon = Some(name.into()).filter(|n| !n.is_empty());
        self
    }
}

/// What a [`Sidebar`] takes as children: [`SidebarItem`]s,
/// [`SidebarSection`]s, and tuples, `Vec`s and `Option`s of them.
pub trait SidebarEntries<T: 'static> {
    fn add_to(self, sections: &mut Vec<SidebarSection<T>>);
}

impl<T: 'static> SidebarEntries<T> for SidebarItem<T> {
    fn add_to(self, sections: &mut Vec<SidebarSection<T>>) {
        match sections.last_mut().filter(|s| s.loose) {
            Some(section) => section.items.push(self),
            None => sections.push(SidebarSection { title: None, items: vec![self], loose: true }),
        }
    }
}

impl<T: 'static> SidebarEntries<T> for SidebarSection<T> {
    fn add_to(self, sections: &mut Vec<SidebarSection<T>>) {
        sections.push(self);
    }
}

impl<T: 'static, E: SidebarEntries<T>> SidebarEntries<T> for Vec<E> {
    fn add_to(self, sections: &mut Vec<SidebarSection<T>>) {
        for entry in self {
            entry.add_to(sections);
        }
    }
}

impl<T: 'static, E: SidebarEntries<T>> SidebarEntries<T> for Option<E> {
    fn add_to(self, sections: &mut Vec<SidebarSection<T>>) {
        if let Some(entry) = self {
            entry.add_to(sections);
        }
    }
}

impl<T: 'static> SidebarEntries<T> for () {
    fn add_to(self, _: &mut Vec<SidebarSection<T>>) {}
}

/// What a [`SidebarSection`] takes as children: [`SidebarItem`]s, and
/// tuples, `Vec`s and `Option`s of them.
pub trait SidebarItems<T: 'static> {
    fn add_to(self, items: &mut Vec<SidebarItem<T>>);
}

impl<T: 'static> SidebarItems<T> for SidebarItem<T> {
    fn add_to(self, items: &mut Vec<SidebarItem<T>>) {
        items.push(self);
    }
}

impl<T: 'static, I: SidebarItems<T>> SidebarItems<T> for Vec<I> {
    fn add_to(self, items: &mut Vec<SidebarItem<T>>) {
        for item in self {
            item.add_to(items);
        }
    }
}

impl<T: 'static, I: SidebarItems<T>> SidebarItems<T> for Option<I> {
    fn add_to(self, items: &mut Vec<SidebarItem<T>>) {
        if let Some(item) = self {
            item.add_to(items);
        }
    }
}

impl<T: 'static> SidebarItems<T> for () {
    fn add_to(self, _: &mut Vec<SidebarItem<T>>) {}
}

macro_rules! tuple_sidebar {
    ($($name:ident),+) => {
        impl<T: 'static, $($name: SidebarEntries<T>),+> SidebarEntries<T> for ($($name,)+) {
            #[allow(non_snake_case)]
            fn add_to(self, sections: &mut Vec<SidebarSection<T>>) {
                let ($($name,)+) = self;
                $($name.add_to(sections);)+
            }
        }

        impl<T: 'static, $($name: SidebarItems<T>),+> SidebarItems<T> for ($($name,)+) {
            #[allow(non_snake_case)]
            fn add_to(self, items: &mut Vec<SidebarItem<T>>) {
                let ($($name,)+) = self;
                $($name.add_to(items);)+
            }
        }
    };
}

tuple_sidebar!(A);
tuple_sidebar!(A, B);
tuple_sidebar!(A, B, C);
tuple_sidebar!(A, B, C, D);
tuple_sidebar!(A, B, C, D, E);
tuple_sidebar!(A, B, C, D, E, F);
tuple_sidebar!(A, B, C, D, E, F, G);
tuple_sidebar!(A, B, C, D, E, F, G, H);
tuple_sidebar!(A, B, C, D, E, F, G, H, I);
tuple_sidebar!(A, B, C, D, E, F, G, H, I, J);
tuple_sidebar!(A, B, C, D, E, F, G, H, I, J, K);
tuple_sidebar!(A, B, C, D, E, F, G, H, I, J, K, L);

// ------------------------------------------------------------------ tabs

/// Pages, one shown at a time, with a tab for each to pick it: the
/// platform's own tab view (an NSTabView on macOS, a notebook on GTK, a
/// selector bar over its pages on Windows, a tab bar on KDE). It's for a
/// fixed set of views, as in a settings window, not documents the user
/// opens and closes.
///
/// Its pages are [`Tab`]s. `selection` holds the value of the page shown,
/// and picking a tab sets it; a value no tab has shows the first, as every
/// platform's tab view shows one. Every page stays built while it's
/// hidden, so what's in it (a half-typed field, a scroll position) is
/// still there when it's picked again. The view is as big as its biggest
/// page, and the platform's tab strip and border are added to it; put
/// padding on the pages.
///
/// ```ignore
/// let page = signal(Page::General);
/// Tabs::new(page).children((
///     Tab::new("General", Page::General).padding(16).children((…)),
///     Tab::new("Advanced", Page::Advanced).padding(16).children((…)),
/// ))
/// ```
///
/// In `view!`: `<Tabs selection=page>`, with
/// `<Tab title="General" value=Page::General>…</Tab>`s as children.
pub struct Tabs<T: 'static> {
    element: Element,
    selection: Signal<T>,
    tabs: Vec<Tab<T>>,
}

impl<T: 'static> ElementBuilder for Tabs<T> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl<T: PartialEq + Clone + 'static> Tabs<T> {
    pub fn new(selection: Signal<T>) -> Tabs<T> {
        Tabs { element: Element::new(WidgetKind::Tabs), selection, tabs: Vec::new() }
    }

    /// Its pages, in order.
    pub fn children(mut self, tabs: impl TabsChildren<T>) -> Tabs<T> {
        tabs.add_to(&mut self.tabs);
        self
    }

    pub fn tab(self, tab: Tab<T>) -> Tabs<T> {
        self.children(tab)
    }
}

impl<T: PartialEq + Clone + 'static> View for Tabs<T> {
    fn build(self, ui: &Ui) -> NodeId {
        let Tabs { mut element, selection, tabs } = self;
        let titles: Vec<Value<String>> = tabs.iter().map(|t| t.title.clone()).collect();
        let values: Rc<Vec<T>> = Rc::new(tabs.iter().map(|t| t.value.clone()).collect());
        // The titles first: the selection is an index into them.
        element.prop(Value::Dynamic(Rc::new(move || titles.iter().map(|t| t.get()).collect())), Prop::TabTitles);
        let shown = values.clone();
        element.prop(
            Value::Dynamic(Rc::new(move || {
                let index = selection.with(|value| shown.iter().position(|v| v == value));
                (!shown.is_empty()).then(|| index.unwrap_or(0))
            })),
            Prop::SelectedIndex,
        );
        element.on(move |event| {
            if let UiEvent::Changed(EventValue::Index(index)) = event
                && let Some(value) = values.get(*index)
            {
                selection.set(value.clone());
            }
        });
        for tab in tabs {
            element.add_children(tab.page);
        }
        element.build(ui)
    }
}

/// A page of [`Tabs`]: its title, the value picking it gives the tabs'
/// selection, and what it shows, in a column.
///
/// In `view!`, its children are its content:
/// `<Tab title="General" value=Page::General>…</Tab>`.
pub struct Tab<T: 'static> {
    title: Value<String>,
    value: T,
    page: Container,
}

impl<T: 'static> ElementBuilder for Tab<T> {
    fn element(&mut self) -> &mut Element {
        self.page.element()
    }
}

impl<T: 'static> Tab<T> {
    pub fn new(title: impl IntoValue<String>, value: T) -> Tab<T> {
        Tab { title: title.into_value(), value, page: Column::new() }
    }

    pub fn title(mut self, title: impl IntoValue<String>) -> Tab<T> {
        self.title = title.into_value();
        self
    }

    /// The value picking it gives the tabs' selection.
    pub fn value<U: 'static>(self, value: U) -> Tab<U> {
        Tab { title: self.title, value, page: self.page }
    }

    /// What it shows, in a column.
    pub fn children(mut self, children: impl Children) -> Tab<T> {
        self.page = self.page.children(children);
        self
    }

    pub fn child(self, child: impl View) -> Tab<T> {
        self.children(child)
    }

    /// Gap between its children.
    pub fn gap(mut self, gap: impl IntoValue<Length>) -> Tab<T> {
        self.page = self.page.gap(gap);
        self
    }
}

/// What [`Tabs`] take as children: [`Tab`]s, and tuples, `Vec`s and
/// `Option`s of them.
pub trait TabsChildren<T: 'static> {
    fn add_to(self, tabs: &mut Vec<Tab<T>>);
}

impl<T: 'static> TabsChildren<T> for Tab<T> {
    fn add_to(self, tabs: &mut Vec<Tab<T>>) {
        tabs.push(self);
    }
}

impl<T: 'static, C: TabsChildren<T>> TabsChildren<T> for Vec<C> {
    fn add_to(self, tabs: &mut Vec<Tab<T>>) {
        for tab in self {
            tab.add_to(tabs);
        }
    }
}

impl<T: 'static, C: TabsChildren<T>> TabsChildren<T> for Option<C> {
    fn add_to(self, tabs: &mut Vec<Tab<T>>) {
        if let Some(tab) = self {
            tab.add_to(tabs);
        }
    }
}

impl<T: 'static> TabsChildren<T> for () {
    fn add_to(self, _: &mut Vec<Tab<T>>) {}
}

macro_rules! tuple_tabs {
    ($($name:ident),+) => {
        impl<T: 'static, $($name: TabsChildren<T>),+> TabsChildren<T> for ($($name,)+) {
            #[allow(non_snake_case)]
            fn add_to(self, tabs: &mut Vec<Tab<T>>) {
                let ($($name,)+) = self;
                $($name.add_to(tabs);)+
            }
        }
    };
}

tuple_tabs!(A);
tuple_tabs!(A, B);
tuple_tabs!(A, B, C);
tuple_tabs!(A, B, C, D);
tuple_tabs!(A, B, C, D, E);
tuple_tabs!(A, B, C, D, E, F);
tuple_tabs!(A, B, C, D, E, F, G);
tuple_tabs!(A, B, C, D, E, F, G, H);
tuple_tabs!(A, B, C, D, E, F, G, H, I);
tuple_tabs!(A, B, C, D, E, F, G, H, I, J);
tuple_tabs!(A, B, C, D, E, F, G, H, I, J, K);
tuple_tabs!(A, B, C, D, E, F, G, H, I, J, K, L);

// ------------------------------------------------------------- containers

/// A layout host: flexbox (column by default) or grid.
pub struct Container(Element);
widget!(Container);

impl Default for Container {
    fn default() -> Container {
        Container::new()
    }
}

impl Container {
    pub fn new() -> Container {
        Container(Element::new(WidgetKind::Container))
    }

    pub fn children(mut self, children: impl Children) -> Container {
        self.0.add_children(children);
        self
    }

    pub fn child(self, child: impl View) -> Container {
        self.children(child)
    }

    /// Takes the files and folders `drop` says when they're dropped on it
    /// from the platform's file manager, and gives them to `on_drop`. The
    /// platform shows it'll copy them while they're over it; `on_drop_hover`
    /// says when, for the app's own highlight. Dragging isn't reachable
    /// from the keyboard or assistive technology: offer another way (an
    /// open dialog) too.
    pub fn file_drop(mut self, drop: impl IntoValue<FileDrop>) -> Container {
        let drop = match drop.into_value() {
            Value::Static(d) => Value::Static(Some(d)),
            dynamic => Value::Dynamic(Rc::new(move || Some(dynamic.get()))),
        };
        self.0.prop(drop, Prop::FileDrop);
        self
    }

    /// The files and folders dropped on it that it takes (`file_drop`).
    pub fn on_drop(mut self, handler: impl Fn(Vec<PathBuf>) + 'static) -> Container {
        self.0.on(move |event| {
            if let UiEvent::FilesDropped(paths) = event {
                handler(paths.clone());
            }
        });
        self
    }

    /// Whether files it takes are over it (`file_drop`).
    pub fn on_drop_hover(mut self, handler: impl Fn(bool) + 'static) -> Container {
        self.0.on(move |event| {
            if let UiEvent::DropHover(over) = event {
                handler(*over);
            }
        });
        self
    }

    pub fn flex_direction(mut self, direction: impl IntoValue<FlexDirection>) -> Container {
        self.0.style_prop(direction.into_value(), |s, v| s.flex_direction = v);
        self
    }

    /// Gap between rows and columns.
    pub fn gap(mut self, gap: impl IntoValue<Length>) -> Container {
        self.0.style_prop(gap.into_value(), |s, v| {
            s.row_gap = v;
            s.column_gap = v;
        });
        self
    }

    pub fn row_gap(mut self, gap: impl IntoValue<Length>) -> Container {
        self.0.style_prop(gap.into_value(), |s, v| s.row_gap = v);
        self
    }

    pub fn column_gap(mut self, gap: impl IntoValue<Length>) -> Container {
        self.0.style_prop(gap.into_value(), |s, v| s.column_gap = v);
        self
    }

    /// Cross-axis alignment of children (`align-items`).
    pub fn align(mut self, align: impl IntoValue<Align>) -> Container {
        self.0.style_prop(align.into_value(), |s, v| s.align_items = Some(v));
        self
    }

    /// Main-axis distribution of children (`justify-content`).
    pub fn justify(mut self, justify: impl IntoValue<Justify>) -> Container {
        self.0.style_prop(justify.into_value(), |s, v| s.justify_content = Some(v));
        self
    }

    pub fn wrap(self) -> Container {
        self.style(|s| s.flex_wrap = true)
    }

    /// Switches to grid layout with these column tracks.
    pub fn columns<T: Into<Track>>(self, tracks: impl IntoIterator<Item = T>) -> Container {
        let tracks: Vec<Track> = tracks.into_iter().map(Into::into).collect();
        self.style(|s| {
            s.display = Display::Grid;
            s.grid_template_columns = tracks;
        })
    }

    /// Switches to grid layout with these row tracks.
    pub fn rows<T: Into<Track>>(self, tracks: impl IntoIterator<Item = T>) -> Container {
        let tracks: Vec<Track> = tracks.into_iter().map(Into::into).collect();
        self.style(|s| {
            s.display = Display::Grid;
            s.grid_template_rows = tracks;
        })
    }
}

/// Vertical flex container.
pub struct Column;

impl Column {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Container {
        Container::new().flex_direction(FlexDirection::Column)
    }
}

/// Horizontal flex container.
pub struct Row;

impl Row {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Container {
        Container::new().flex_direction(FlexDirection::Row)
    }
}

/// Grid container.
pub struct Grid;

impl Grid {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Container {
        Container::new().style(|s| s.display = Display::Grid)
    }
}

/// A box around related content, as the platform groups it: an `NSBox`
/// (its heading inside, at the top), libadwaita's card under a heading,
/// a Fluent card under a heading on Windows, a `QQC2.GroupBox`. The
/// heading is optional; it names the group to assistive technology.
///
/// Its children are laid out as a column's (`gap`, and any style), inside
/// the platform's border and margins; `padding` adds to those. Put a `Row`
/// or `Grid` in it for other layouts.
///
/// ```ignore
/// Group::new().title("CD drive").child(
///     Row::new().gap(Spacing::Md).children((Icon::new(disc), Text::new(title), eject)),
/// )
/// ```
pub struct Group(Element);
widget!(Group);

impl Default for Group {
    fn default() -> Group {
        Group::new()
    }
}

impl Group {
    pub fn new() -> Group {
        Group(Element::new(WidgetKind::Group)).style(|s| {
            s.display = Display::Flex;
            s.flex_direction = FlexDirection::Column;
        })
    }

    /// Its heading; empty: none.
    pub fn title(mut self, title: impl IntoValue<String>) -> Group {
        self.0.prop(title.into_value(), Prop::Title);
        self
    }

    pub fn children(mut self, children: impl Children) -> Group {
        self.0.add_children(children);
        self
    }

    pub fn child(self, child: impl View) -> Group {
        self.children(child)
    }

    /// Takes the files and folders `drop` says when they're dropped on it
    /// from the platform's file manager, and gives them to `on_drop`. The
    /// platform shows it'll copy them while they're over it; `on_drop_hover`
    /// says when, for the app's own highlight. Dragging isn't reachable
    /// from the keyboard or assistive technology: offer another way (an
    /// open dialog) too.
    pub fn file_drop(mut self, drop: impl IntoValue<FileDrop>) -> Group {
        let drop = match drop.into_value() {
            Value::Static(d) => Value::Static(Some(d)),
            dynamic => Value::Dynamic(Rc::new(move || Some(dynamic.get()))),
        };
        self.0.prop(drop, Prop::FileDrop);
        self
    }

    /// The files and folders dropped on it that it takes (`file_drop`).
    pub fn on_drop(mut self, handler: impl Fn(Vec<PathBuf>) + 'static) -> Group {
        self.0.on(move |event| {
            if let UiEvent::FilesDropped(paths) = event {
                handler(paths.clone());
            }
        });
        self
    }

    /// Whether files it takes are over it (`file_drop`).
    pub fn on_drop_hover(mut self, handler: impl Fn(bool) + 'static) -> Group {
        self.0.on(move |event| {
            if let UiEvent::DropHover(over) = event {
                handler(*over);
            }
        });
        self
    }

    /// Space between its children.
    pub fn gap(mut self, gap: impl IntoValue<Length>) -> Group {
        self.0.style_prop(gap.into_value(), |s, v| {
            s.row_gap = v;
            s.column_gap = v;
        });
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Group>) -> Group {
        tweak.apply(&mut self.0);
        self
    }
}

/// A native scroll container. Its children go into a content box that
/// keeps its natural size, so it can be larger than the scroll view.
///
/// Give the scroll view a bounded size (a fixed height, or `grow` inside a
/// sized parent); otherwise it grows with its content and never scrolls.
/// As in CSS, its natural size is its content's, so in a flex container its
/// siblings shrink along with it unless they have `.shrink(0.0)`.
/// Also as in CSS, a flex item doesn't shrink below its content's width:
/// a horizontal scroll view in a column that grows in a row needs
/// `.min_width(0)` on that column, or both are as wide as the content, and
/// there's nothing to scroll.
pub struct ScrollView {
    outer: Element,
    content: Container,
}

impl ElementBuilder for ScrollView {
    fn element(&mut self) -> &mut Element {
        &mut self.outer
    }
}

impl View for ScrollView {
    fn build(mut self, ui: &Ui) -> NodeId {
        self.outer.add_children(self.content);
        self.outer.build(ui)
    }
}

impl Default for ScrollView {
    fn default() -> ScrollView {
        ScrollView::new()
    }
}

impl ScrollView {
    /// Scrolls vertically.
    pub fn new() -> ScrollView {
        ScrollView::with_axes(ScrollAxes::Vertical)
    }

    pub fn horizontal() -> ScrollView {
        ScrollView::with_axes(ScrollAxes::Horizontal)
    }

    pub fn both() -> ScrollView {
        ScrollView::with_axes(ScrollAxes::Both)
    }

    fn with_axes(axes: ScrollAxes) -> ScrollView {
        let outer = Element::new(WidgetKind::ScrollView);
        ScrollView { outer, content: Container::new().shrink(0.0) }.axes(axes)
    }

    /// The axes it scrolls along. In `view!`, where there's no constructor
    /// to pick them: `<ScrollView axes=ScrollAxes::Horizontal>`.
    pub fn axes(mut self, axes: ScrollAxes) -> ScrollView {
        self.outer.prop(axes.into_value(), Prop::ScrollAxes);
        self.outer.style.scroll_x = axes.horizontal();
        self.outer.style.scroll_y = axes.vertical();
        // The content stretches across the non-scrolling axis and keeps its
        // natural size along the scrolling ones.
        self.outer.style.flex_direction =
            if axes == ScrollAxes::Horizontal { FlexDirection::Row } else { FlexDirection::Column };
        // Horizontal content flows in a row; the other kinds in a column.
        let content = &mut self.content.0.style;
        content.flex_direction =
            if axes == ScrollAxes::Horizontal { FlexDirection::Row } else { FlexDirection::Column };
        content.align_self = (axes == ScrollAxes::Both).then_some(Align::Start);
        self
    }

    pub fn children(mut self, children: impl Children) -> ScrollView {
        self.content = self.content.children(children);
        self
    }

    pub fn child(self, child: impl View) -> ScrollView {
        self.children(child)
    }

    /// Whether it shows scroll bars, on by default. They're the platform's,
    /// shown as it shows them (overlay bars, say, as the user set on
    /// macOS). Without, it still scrolls by wheel, trackpad and touch, as a
    /// strip of photos or a carousel does.
    pub fn scroll_bars(mut self, show: impl IntoValue<bool>) -> ScrollView {
        self.outer.prop(show.into_value(), Prop::ScrollBars);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`]. What
    /// the platforms offer (elasticity and a border on AppKit, classic
    /// scroll bars on GTK, overshoot on Qt, always-expanded scroll bars and
    /// zoom on WinUI) is each one's own.
    pub fn native(mut self, tweak: Tweak<ScrollView>) -> ScrollView {
        tweak.apply(&mut self.outer);
        self
    }

    /// Called with the new offset whenever the content scrolls.
    pub fn on_scroll(mut self, handler: impl Fn(Point) + 'static) -> ScrollView {
        self.outer.on(move |event| {
            if let UiEvent::Scrolled(offset) = event {
                handler(*offset);
            }
        });
        self
    }
}

// ----------------------------------------------------------------- leaves

/// Static or reactive text.
pub struct Text(Element);
widget!(Text);

impl Text {
    pub fn new(text: impl IntoValue<String>) -> Text {
        let mut element = Element::new(WidgetKind::Text);
        element.prop(text.into_value(), Prop::Text);
        Text(element)
    }

    pub fn text_style(mut self, style: impl IntoValue<TextStyle>) -> Text {
        self.0.prop(style.into_value(), Prop::TextStyle);
        self
    }

    /// Shows at most this many lines, the last one cut off with an
    /// ellipsis where the text goes on, as the platform draws one; 0 shows
    /// them all. It's still read out in full.
    pub fn max_lines(mut self, lines: impl IntoValue<u32>) -> Text {
        self.0.prop(lines.into_value(), |n| Prop::MaxLines((n > 0).then_some(n)));
        self
    }

    /// Its colour. A semantic one (`Color::SecondaryLabel`, `Color::Error`,
    /// …) is the platform's own, and follows dark mode and high contrast;
    /// `Color::Rgba` is fixed, whatever the appearance.
    pub fn color(mut self, color: impl IntoValue<Color>) -> Text {
        self.0.prop(color.into_value(), Prop::TextColor);
        self
    }

    /// Its weight, in place of its text style's. A platform whose font
    /// lacks one uses the nearest it has.
    pub fn weight(mut self, weight: impl IntoValue<FontWeight>) -> Text {
        self.0.prop(weight.into_value(), Prop::FontWeight);
        self
    }

    pub fn italic(mut self, italic: impl IntoValue<bool>) -> Text {
        self.0.prop(italic.into_value(), Prop::Italic);
        self
    }

    /// Where its lines go across its frame: `Start` and `End` follow its
    /// direction (`TextDirection`). Like CSS's `text-align`, it shows only
    /// where the frame is wider than the text, e.g. stretched across a
    /// column, or wrapping.
    pub fn text_align(mut self, align: impl IntoValue<TextAlign>) -> Text {
        self.0.style_prop(align.into_value(), |s, align| s.text_align = Some(align));
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`]. What
    /// the platforms offer (style classes on GTK, Markdown on Qt, character
    /// spacing on WinUI, selection on all but Qt's labels) is each one's
    /// own.
    pub fn native(mut self, tweak: Tweak<Text>) -> Text {
        tweak.apply(&mut self.0);
        self
    }
}

pub struct Button(Element);
widget!(Button);

impl Button {
    pub fn new(label: impl IntoValue<String>) -> Button {
        let mut element = Element::new(WidgetKind::Button);
        element.prop(label.into_value(), Prop::Label);
        Button(element)
    }

    /// What the button does in its window: see [`ButtonRole`].
    pub fn role(mut self, role: impl IntoValue<ButtonRole>) -> Button {
        self.0.prop(role.into_value(), Prop::ButtonRole);
        self
    }

    /// How the button is drawn: see [`ButtonStyle`].
    pub fn button_style(mut self, style: impl IntoValue<ButtonStyle>) -> Button {
        self.0.prop(style.into_value(), Prop::ButtonStyle);
        self
    }

    /// An icon before its caption, named in the platform's own set as for
    /// [`Icon`]; empty: none. Each platform places and sizes it its own way.
    pub fn icon(mut self, name: impl IntoValue<String>) -> Button {
        self.0.prop(name.into_value(), Prop::Icon);
        self
    }

    /// Shows only its icon. The caption stays its accessible name; a
    /// tooltip saying it too is up to the app, as platforms leave it.
    pub fn icon_only(mut self, only: impl IntoValue<bool>) -> Button {
        self.0.prop(only.into_value(), Prop::IconOnly);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Button>) -> Button {
        tweak.apply(&mut self.0);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> Button {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Button {
        self.0.on(move |event| {
            if *event == UiEvent::Click {
                handler();
            }
        });
        self
    }
}

/// A button that opens a menu of actions, as the platform makes one: a
/// pull-down `NSPopUpButton`, a `gtk::MenuButton`, a `DropDownButton`, a
/// `QQC2.Button` that opens a `QQC2.Menu`. Each draws its own arrow. The
/// menu is built as a context menu is, and an item's `on_select` runs when
/// it's chosen. It has no click of its own: clicking opens the menu.
///
/// ```ignore
/// MenuButton::new("Add").icon(add_icon).menu((
///     MenuItem::new("Disc image…").on_select(add_image),
///     MenuItem::new("Folder…").on_select(add_folder),
///     MenuSeparator,
///     MenuItem::new("Guest tools").on_select(add_tools),
/// ))
/// ```
pub struct MenuButton(Element);
widget!(MenuButton);

impl MenuButton {
    pub fn new(label: impl IntoValue<String>) -> MenuButton {
        let mut element = Element::new(WidgetKind::MenuButton);
        element.prop(label.into_value(), Prop::Label);
        MenuButton(element)
    }

    /// Its menu: `MenuItem`s, `MenuSeparator`s and submenus (`Menu`).
    pub fn menu(mut self, entries: impl MenuEntries + 'static) -> MenuButton {
        self.0.after_build(move |ui, id| install_button_menu(ui, id, entries));
        self
    }

    /// An icon before its caption, as for [`Button::icon`].
    pub fn icon(mut self, name: impl IntoValue<String>) -> MenuButton {
        self.0.prop(name.into_value(), Prop::Icon);
        self
    }

    /// Shows only its icon (and the platform's arrow). The caption stays
    /// its accessible name.
    pub fn icon_only(mut self, only: impl IntoValue<bool>) -> MenuButton {
        self.0.prop(only.into_value(), Prop::IconOnly);
        self
    }

    /// How the button is drawn: see [`ButtonStyle`].
    pub fn button_style(mut self, style: impl IntoValue<ButtonStyle>) -> MenuButton {
        self.0.prop(style.into_value(), Prop::ButtonStyle);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> MenuButton {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<MenuButton>) -> MenuButton {
        tweak.apply(&mut self.0);
        self
    }
}

/// Single-line text entry.
pub struct TextInput(Element);
widget!(TextInput);

impl Default for TextInput {
    fn default() -> TextInput {
        TextInput::new()
    }
}

impl TextInput {
    pub fn new() -> TextInput {
        TextInput(Element::new(WidgetKind::TextInput))
    }

    pub fn value(mut self, value: impl IntoValue<String>) -> TextInput {
        self.0.prop(value.into_value(), Prop::Value);
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<String>) -> TextInput {
        self.value(signal).on_input(move |text| signal.set(text))
    }

    pub fn placeholder(mut self, placeholder: impl IntoValue<String>) -> TextInput {
        self.0.prop(placeholder.into_value(), Prop::Placeholder);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> TextInput {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Shows the text, which can still be selected, copied and focused, but
    /// not edited. Unlike a disabled field, it looks and reads as usual.
    pub fn read_only(mut self, read_only: impl IntoValue<bool>) -> TextInput {
        self.0.prop(read_only.into_value(), Prop::ReadOnly);
        self
    }

    /// Called on every edit with the new text.
    pub fn on_input(mut self, handler: impl Fn(String) + 'static) -> TextInput {
        self.0.on(move |event| {
            if let UiEvent::Changed(EventValue::Text(text)) = event {
                handler(text.clone());
            }
        });
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`]. What
    /// the platforms offer (a borderless field on AppKit, icons on GTK, a
    /// length limit on GTK, Qt and WinUI, a header on WinUI) is each one's
    /// own.
    pub fn native(mut self, tweak: Tweak<TextInput>) -> TextInput {
        tweak.apply(&mut self.0);
        self
    }

    /// Called when the user confirms (Return / Enter).
    pub fn on_submit(mut self, handler: impl Fn() + 'static) -> TextInput {
        self.0.on(move |event| {
            if *event == UiEvent::Submit {
                handler();
            }
        });
        self
    }
}

/// Single-line entry of a password: the platform's password field, which
/// hides the text as the platform does (bullets, and a button to show it
/// where the platform has one) and never reads it out.
pub struct PasswordInput(Element);
widget!(PasswordInput);

impl Default for PasswordInput {
    fn default() -> PasswordInput {
        PasswordInput::new()
    }
}

impl PasswordInput {
    pub fn new() -> PasswordInput {
        PasswordInput(Element::new(WidgetKind::PasswordInput))
    }

    pub fn value(mut self, value: impl IntoValue<String>) -> PasswordInput {
        self.0.prop(value.into_value(), Prop::Value);
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<String>) -> PasswordInput {
        self.value(signal).on_input(move |text| signal.set(text))
    }

    pub fn placeholder(mut self, placeholder: impl IntoValue<String>) -> PasswordInput {
        self.0.prop(placeholder.into_value(), Prop::Placeholder);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> PasswordInput {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Raw platform settings: see [`Tweak`]. Password fields have no
    /// semantic options past a text field's: what the platforms offer
    /// (GTK's peek icon, WinUI's reveal mode, the bullet on Qt and WinUI)
    /// is each one's own.
    pub fn native(mut self, tweak: Tweak<PasswordInput>) -> PasswordInput {
        tweak.apply(&mut self.0);
        self
    }

    /// Called on every edit with the new text.
    pub fn on_input(mut self, handler: impl Fn(String) + 'static) -> PasswordInput {
        self.0.on(move |event| {
            if let UiEvent::Changed(EventValue::Text(text)) = event {
                handler(text.clone());
            }
        });
        self
    }

    /// Called when the user confirms (Return / Enter).
    pub fn on_submit(mut self, handler: impl Fn() + 'static) -> PasswordInput {
        self.0.on(move |event| {
            if *event == UiEvent::Submit {
                handler();
            }
        });
        self
    }
}

/// Single-line entry of search text: the platform's search field, with its
/// search icon and clear button. [`on_search`](Self::on_search) is called
/// when the platform asks for a search, as its own apps search.
pub struct SearchInput(Element);
widget!(SearchInput);

impl Default for SearchInput {
    fn default() -> SearchInput {
        SearchInput::new()
    }
}

impl SearchInput {
    pub fn new() -> SearchInput {
        SearchInput(Element::new(WidgetKind::SearchInput))
    }

    pub fn value(mut self, value: impl IntoValue<String>) -> SearchInput {
        self.0.prop(value.into_value(), Prop::Value);
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<String>) -> SearchInput {
        self.value(signal).on_input(move |text| signal.set(text))
    }

    /// Shown while it's empty, in place of the platform's own where it has
    /// one (AppKit, Qt). It names the field to assistive technology
    /// without a label.
    pub fn placeholder(mut self, placeholder: impl IntoValue<String>) -> SearchInput {
        self.0.prop(placeholder.into_value(), Prop::Placeholder);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> SearchInput {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Called on every edit with the new text, the clear button's too.
    pub fn on_input(mut self, handler: impl Fn(String) + 'static) -> SearchInput {
        self.0.on(move |event| {
            if let UiEvent::Changed(EventValue::Text(text)) = event {
                handler(text.clone());
            }
        });
        self
    }

    /// Called with the text when the platform asks for a search: as the
    /// user types, after a pause where the platform waits for one (AppKit,
    /// GTK, Qt) and at once where it doesn't (WinUI); on Return; and when
    /// the field is cleared. Not for text the app set.
    pub fn on_search(mut self, handler: impl Fn(String) + 'static) -> SearchInput {
        self.0.on(move |event| {
            if let UiEvent::Search(text) = event {
                handler(text.clone());
            }
        });
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`]. What
    /// the platforms offer (recent searches on AppKit, GTK's search delay,
    /// Kirigami's longer one, a header on WinUI) is each one's own.
    pub fn native(mut self, tweak: Tweak<SearchInput>) -> SearchInput {
        tweak.apply(&mut self.0);
        self
    }
}

/// Text over many lines: the platform's text area, whose lines wrap to its
/// width and where Return starts a new line. It's as tall as
/// [`lines`](Self::lines) of the platform's text, and scrolls past them.
pub struct TextArea(Element);
widget!(TextArea);

impl Default for TextArea {
    fn default() -> TextArea {
        TextArea::new()
    }
}

impl TextArea {
    /// Three lines tall.
    pub fn new() -> TextArea {
        let mut element = Element::new(WidgetKind::TextArea);
        element.prop(Value::Static(3), Prop::Lines);
        TextArea(element)
    }

    /// Its lines end in `\n`.
    pub fn value(mut self, value: impl IntoValue<String>) -> TextArea {
        self.0.prop(value.into_value(), Prop::Value);
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<String>) -> TextArea {
        self.value(signal).on_input(move |text| signal.set(text))
    }

    /// Shown while it's empty, where the platform's text areas show one
    /// (Qt, WinUI); AppKit's and GTK's have none. It still names the area
    /// to assistive technology without a label.
    pub fn placeholder(mut self, placeholder: impl IntoValue<String>) -> TextArea {
        self.0.prop(placeholder.into_value(), Prop::Placeholder);
        self
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> TextArea {
        self.0.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Shows the text, which can still be selected, copied and focused, but
    /// not edited. Unlike a disabled area, it looks and reads as usual.
    pub fn read_only(mut self, read_only: impl IntoValue<bool>) -> TextArea {
        self.0.prop(read_only.into_value(), Prop::ReadOnly);
        self
    }

    /// How many lines of text tall it is at its natural size: 3 unless
    /// set, and at least 1. The layout can still stretch or shrink it.
    pub fn lines(mut self, lines: impl IntoValue<u32>) -> TextArea {
        self.0.prop(lines.into_value(), |n| Prop::Lines(n.max(1)));
        self
    }

    /// Called on every edit with the new text.
    pub fn on_input(mut self, handler: impl Fn(String) + 'static) -> TextArea {
        self.0.on(move |event| {
            if let UiEvent::Changed(EventValue::Text(text)) = event {
                handler(text.clone());
            }
        });
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`]. What
    /// the platforms offer (spelling and substitutions on AppKit, GTK's
    /// wrap modes and monospace, Qt's text format, WinUI's spell check and
    /// header) is each one's own.
    pub fn native(mut self, tweak: Tweak<TextArea>) -> TextArea {
        tweak.apply(&mut self.0);
        self
    }
}

macro_rules! toggle {
    ($t:ident) => {
        impl $t {
            pub fn checked(mut self, checked: impl IntoValue<bool>) -> $t {
                self.0.prop(checked.into_value(), Prop::Checked);
                self
            }

            /// Two-way binding, Vue's `v-model`.
            pub fn bind(self, signal: Signal<bool>) -> $t {
                self.checked(signal).on_change(move |checked| signal.set(checked))
            }

            pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> $t {
                self.0.prop(enabled.into_value(), Prop::Enabled);
                self
            }

            pub fn on_change(mut self, handler: impl Fn(bool) + 'static) -> $t {
                self.0.on(move |event| {
                    if let UiEvent::Changed(EventValue::Bool(checked)) = event {
                        handler(*checked);
                    }
                });
                self
            }
        }
    };
}

pub struct Checkbox(Element);
widget!(Checkbox);
toggle!(Checkbox);

impl Checkbox {
    pub fn new(label: impl IntoValue<String>) -> Checkbox {
        let mut element = Element::new(WidgetKind::Checkbox);
        element.prop(label.into_value(), Prop::Label);
        Checkbox(element)
    }

    /// Shows the mixed state, whatever `checked` says: some of what the box
    /// stands for is checked, as in a "Select all" box. A click leaves it,
    /// checked or not as the platform decides (AppKit checks it, GTK flips
    /// `checked`), and reports that with `on_change`: work out `mixed`
    /// again from there.
    pub fn mixed(mut self, mixed: impl IntoValue<bool>) -> Checkbox {
        self.0.prop(mixed.into_value(), Prop::Mixed);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Checkbox>) -> Checkbox {
        tweak.apply(&mut self.0);
        self
    }
}

/// On/off switch. Most platforms draw no caption; the label is its
/// accessible name.
pub struct Switch(Element);
widget!(Switch);
toggle!(Switch);

impl Switch {
    pub fn new(label: impl IntoValue<String>) -> Switch {
        let mut element = Element::new(WidgetKind::Switch);
        element.prop(label.into_value(), Prop::Label);
        Switch(element)
    }

    /// Raw platform settings: see [`Tweak`]. Switches have no semantic
    /// options past `checked`: what the platforms offer (sizes, GTK's
    /// delayed state, WinUI's on and off text) is each one's own.
    pub fn native(mut self, tweak: Tweak<Switch>) -> Switch {
        tweak.apply(&mut self.0);
        self
    }
}

/// A pop-up menu of text options to choose one from: `NSPopUpButton`,
/// `ComboBox`, `gtk::DropDown`, `QQC2.ComboBox`. Its label is its
/// accessible name; like a text field's, it isn't drawn, so put a `Text`
/// next to it.
///
/// As with HTML's `<select>`, one option is always chosen (GTK can't show
/// none): the first, unless `selected` says otherwise. An index past the
/// options chooses the first too. Only a `Select` without options has
/// nothing chosen.
///
/// ```ignore
/// let color = signal(0);
/// Select::new("Color").options(["Red", "Green", "Blue"]).bind(color)
/// ```
pub struct Select {
    element: Element,
    options: Value<Vec<String>>,
    selected: Value<usize>,
}

impl ElementBuilder for Select {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for Select {
    fn build(mut self, ui: &Ui) -> NodeId {
        let chosen = |index: usize, count: usize| (count > 0).then_some(if index < count { index } else { 0 });
        let selected = match (self.selected, self.options.clone()) {
            (Value::Static(index), Value::Static(options)) => Value::Static(chosen(index, options.len())),
            (index, options) => Value::Dynamic(Rc::new(move || chosen(index.get(), options.get().len()))),
        };
        self.element.prop(self.options, Prop::Options);
        self.element.prop(selected, Prop::SelectedIndex);
        self.element.build(ui)
    }
}

impl Select {
    pub fn new(label: impl IntoValue<String>) -> Select {
        let mut element = Element::new(WidgetKind::Select);
        element.prop(label.into_value(), Prop::Label);
        Select { element, options: Value::Static(Vec::new()), selected: Value::Static(0) }
    }

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<Select label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> Select {
        self.element.prop(label.into_value(), Prop::Label);
        self
    }

    /// The options, in order: a list of strings, or a signal or closure
    /// giving one.
    pub fn options(mut self, options: impl IntoValue<Vec<String>>) -> Select {
        self.options = options.into_value();
        self
    }

    /// The index of the chosen option.
    pub fn selected(mut self, index: impl IntoValue<usize>) -> Select {
        self.selected = index.into_value();
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<usize>) -> Select {
        self.selected(signal).on_change(move |index| signal.set(index))
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> Select {
        self.element.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Raw platform settings: see [`Tweak`]. Selects have no semantic
    /// options past their options and choice: what the platforms offer (a
    /// borderless pop-up on AppKit and Qt, search on GTK, a header on
    /// WinUI) is each one's own.
    pub fn native(mut self, tweak: Tweak<Select>) -> Select {
        tweak.apply(&mut self.element);
        self
    }

    /// Called with the option's index when the user chooses one.
    pub fn on_change(mut self, handler: impl Fn(usize) + 'static) -> Select {
        self.element.on(move |event| {
            if let UiEvent::Changed(EventValue::Index(index)) = event {
                handler(*index);
            }
        });
        self
    }
}

/// Radio buttons, one for each of its options, to choose one from:
/// `NSButton`s of the radio type, `RadioButtons`, `gtk::CheckButton`s in a
/// group, `QQC2.RadioButton`s. Down a column, as each platform stacks
/// them. Its label is its accessible name; it isn't drawn, so put a `Text`
/// above it.
///
/// Unlike a `Select`, a radio group can have none chosen, as on every
/// platform: until the user picks one, if `selected` says `None` or an
/// index past the options. The user can't go back to none.
///
/// ```ignore
/// let size = signal(Some(1));
/// RadioGroup::new("Size").options(["Small", "Medium", "Large"]).bind(size)
/// ```
pub struct RadioGroup {
    element: Element,
    options: Value<Vec<String>>,
    selected: Value<Option<usize>>,
}

impl ElementBuilder for RadioGroup {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for RadioGroup {
    fn build(mut self, ui: &Ui) -> NodeId {
        let chosen = |index: Option<usize>, count: usize| index.filter(|i| *i < count);
        let selected = match (self.selected, self.options.clone()) {
            (Value::Static(index), Value::Static(options)) => Value::Static(chosen(index, options.len())),
            (index, options) => Value::Dynamic(Rc::new(move || chosen(index.get(), options.get().len()))),
        };
        self.element.prop(self.options, Prop::Options);
        self.element.prop(selected, Prop::SelectedIndex);
        self.element.build(ui)
    }
}

impl RadioGroup {
    pub fn new(label: impl IntoValue<String>) -> RadioGroup {
        let mut element = Element::new(WidgetKind::RadioGroup);
        element.prop(label.into_value(), Prop::Label);
        RadioGroup { element, options: Value::Static(Vec::new()), selected: Value::Static(None) }
    }

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<RadioGroup label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> RadioGroup {
        self.element.prop(label.into_value(), Prop::Label);
        self
    }

    /// The options, in order: a list of strings, or a signal or closure
    /// giving one.
    pub fn options(mut self, options: impl IntoValue<Vec<String>>) -> RadioGroup {
        self.options = options.into_value();
        self
    }

    /// The index of the chosen option, `None` for none.
    pub fn selected(mut self, index: impl IntoValue<Option<usize>>) -> RadioGroup {
        self.selected = index.into_value();
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<Option<usize>>) -> RadioGroup {
        self.selected(signal).on_change(move |index| signal.set(Some(index)))
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> RadioGroup {
        self.element.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Raw platform settings: see [`Tweak`]. The tweak gets the group's
    /// container (an `NSStackView`, the `RadioButtons`, a `gtk::Box`, a
    /// QML `ColumnLayout`), which holds the buttons.
    pub fn native(mut self, tweak: Tweak<RadioGroup>) -> RadioGroup {
        tweak.apply(&mut self.element);
        self
    }

    /// Called with the option's index when the user chooses one.
    pub fn on_change(mut self, handler: impl Fn(usize) + 'static) -> RadioGroup {
        self.element.on(move |event| {
            if let UiEvent::Changed(EventValue::Index(index)) = event {
                handler(*index);
            }
        });
        self
    }
}

/// A slider: a number in a range, as the platform's slider shows it,
/// horizontal or vertical. Its label is its accessible name. What its step does is the platform's:
/// AppKit shows it as tick marks the knob stops at, WinUI snaps to it, GTK
/// and Qt move by it from the keyboard.
///
/// ```ignore
/// let volume = signal(50.0);
/// Slider::new("Volume").range(0.0, 100.0).step(10.0).bind(volume)
/// ```
pub struct Slider {
    element: Element,
    range: Value<(f64, f64)>,
    value: Value<f64>,
}

impl ElementBuilder for Slider {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for Slider {
    fn build(mut self, ui: &Ui) -> NodeId {
        // Clamped as the native slider clamps it.
        let clamp = |v: f64, (min, max): (f64, f64)| v.max(min).min(max);
        let value = match (self.value, self.range.clone()) {
            (Value::Static(v), Value::Static(range)) => Value::Static(clamp(v, range)),
            (value, range) => Value::Dynamic(Rc::new(move || clamp(value.get(), range.get()))),
        };
        self.element.prop(self.range, |(min, max)| Prop::Range { min, max });
        self.element.prop(value, Prop::Number);
        self.element.build(ui)
    }
}

impl Slider {
    /// From 0 to 100, at 0.
    pub fn new(label: impl IntoValue<String>) -> Slider {
        let mut element = Element::new(WidgetKind::Slider);
        element.prop(label.into_value(), Prop::Label);
        Slider { element, range: Value::Static((0.0, 100.0)), value: Value::Static(0.0) }
    }

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<Slider label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> Slider {
        self.element.prop(label.into_value(), Prop::Label);
        self
    }

    pub fn range(mut self, min: f64, max: f64) -> Slider {
        self.range = Value::Static((min, max));
        self
    }

    /// A range that changes: `(min, max)`.
    pub fn range_with(mut self, range: impl IntoValue<(f64, f64)>) -> Slider {
        self.range = range.into_value();
        self
    }

    /// The step the platform snaps to (where its sliders snap) and moves by
    /// from the keyboard. Without one, the platform's default.
    pub fn step(mut self, step: impl IntoValue<f64>) -> Slider {
        let step = step.into_value();
        let step = match step {
            Value::Static(s) => Value::Static(Some(s)),
            dynamic => Value::Dynamic(Rc::new(move || Some(dynamic.get()))),
        };
        self.element.prop(step, Prop::Step);
        self
    }

    pub fn value(mut self, value: impl IntoValue<f64>) -> Slider {
        self.value = value.into_value();
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<f64>) -> Slider {
        self.value(signal).on_change(move |value| signal.set(value))
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> Slider {
        self.element.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Which way it runs; larger values are up when vertical.
    pub fn orientation(mut self, orientation: impl IntoValue<Orientation>) -> Slider {
        self.element.prop(orientation.into_value(), Prop::Orientation);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Slider>) -> Slider {
        tweak.apply(&mut self.element);
        self
    }

    /// Called with the new value as the user moves the slider.
    pub fn on_change(mut self, handler: impl Fn(f64) + 'static) -> Slider {
        self.element.on(move |event| {
            if let UiEvent::Changed(EventValue::Number(value)) = event {
                handler(*value);
            }
        });
        self
    }
}

/// A field for a whole number, with buttons that step it up and down, as
/// the platform's spin box shows it. Its label is its accessible name; show
/// one beside it with a `Text`. Typing reports the number when the edit is
/// done (Return, or leaving the field), as each platform commits one.
/// Whole numbers in an `i32`, because Qt's spin box holds an `int`.
///
/// ```ignore
/// let memory = signal(64);
/// NumberInput::new("Memory (MB)").range(16, 512).step(16).bind(memory)
/// ```
pub struct NumberInput {
    element: Element,
    range: Value<(i32, i32)>,
    value: Value<i32>,
}

impl ElementBuilder for NumberInput {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for NumberInput {
    fn build(mut self, ui: &Ui) -> NodeId {
        // Clamped as the native spin box clamps it.
        let clamp = |v: i32, (min, max): (i32, i32)| v.max(min).min(max);
        let value = match (self.value, self.range.clone()) {
            (Value::Static(v), Value::Static(range)) => Value::Static(clamp(v, range)),
            (value, range) => Value::Dynamic(Rc::new(move || clamp(value.get(), range.get()))),
        };
        self.element.prop(self.range, |(min, max)| Prop::Range { min: min.into(), max: max.into() });
        self.element.prop(value, |v| Prop::Number(v.into()));
        self.element.build(ui)
    }
}

impl NumberInput {
    /// From 0 to 100, at 0.
    pub fn new(label: impl IntoValue<String>) -> NumberInput {
        let mut element = Element::new(WidgetKind::NumberInput);
        element.prop(label.into_value(), Prop::Label);
        NumberInput { element, range: Value::Static((0, 100)), value: Value::Static(0) }
    }

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<NumberInput label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> NumberInput {
        self.element.prop(label.into_value(), Prop::Label);
        self
    }

    pub fn range(mut self, min: i32, max: i32) -> NumberInput {
        self.range = Value::Static((min, max));
        self
    }

    /// A range that changes: `(min, max)`.
    pub fn range_with(mut self, range: impl IntoValue<(i32, i32)>) -> NumberInput {
        self.range = range.into_value();
        self
    }

    /// What the buttons and arrow keys add or take away. Without one, the
    /// platform's default (1 on every platform).
    pub fn step(mut self, step: impl IntoValue<i32>) -> NumberInput {
        let step = step.into_value();
        let step = match step {
            Value::Static(s) => Value::Static(Some(s.into())),
            dynamic => Value::Dynamic(Rc::new(move || Some(dynamic.get().into()))),
        };
        self.element.prop(step, Prop::Step);
        self
    }

    pub fn value(mut self, value: impl IntoValue<i32>) -> NumberInput {
        self.value = value.into_value();
        self
    }

    /// Two-way binding, Vue's `v-model`.
    pub fn bind(self, signal: Signal<i32>) -> NumberInput {
        self.value(signal).on_change(move |value| signal.set(value))
    }

    pub fn enabled(mut self, enabled: impl IntoValue<bool>) -> NumberInput {
        self.element.prop(enabled.into_value(), Prop::Enabled);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<NumberInput>) -> NumberInput {
        tweak.apply(&mut self.element);
        self
    }

    /// Called with the new number when the user steps it or finishes
    /// typing one.
    pub fn on_change(mut self, handler: impl Fn(i32) + 'static) -> NumberInput {
        self.element.on(move |event| {
            if let UiEvent::Changed(EventValue::Number(value)) = event {
                // Backends report whole numbers in range; `as` saturates.
                handler(value.round() as i32);
            }
        });
        self
    }
}

/// A progress bar, as the platform draws one: how far along a task is, from
/// 0 to 1, or, until a value is given (or while `indeterminate`), an
/// animated bar for work of unknown length. Its label is its accessible
/// name.
///
/// ```ignore
/// Progress::new("Upload").value(move || sent.get() / total)
/// ```
pub struct Progress {
    element: Element,
    value: Option<Value<f64>>,
    indeterminate: Value<bool>,
}

impl ElementBuilder for Progress {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for Progress {
    fn build(mut self, ui: &Ui) -> NodeId {
        let progress = match (self.value, self.indeterminate) {
            (None, _) => Value::Static(None),
            (Some(Value::Static(v)), Value::Static(unknown)) => Value::Static((!unknown).then_some(v.clamp(0.0, 1.0))),
            (Some(value), unknown) => {
                Value::Dynamic(Rc::new(move || (!unknown.get()).then(|| value.get().clamp(0.0, 1.0))))
            }
        };
        self.element.prop(progress, Prop::Progress);
        self.element.build(ui)
    }
}

impl Progress {
    pub fn new(label: impl IntoValue<String>) -> Progress {
        let mut element = Element::new(WidgetKind::Progress);
        element.prop(label.into_value(), Prop::Label);
        Progress { element, value: None, indeterminate: Value::Static(false) }
    }

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<Progress label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> Progress {
        self.element.prop(label.into_value(), Prop::Label);
        self
    }

    /// How far along, from 0 to 1.
    pub fn value(mut self, value: impl IntoValue<f64>) -> Progress {
        self.value = Some(value.into_value());
        self
    }

    /// Shows work of unknown length instead of the value while true.
    pub fn indeterminate(mut self, indeterminate: impl IntoValue<bool>) -> Progress {
        self.indeterminate = indeterminate.into_value();
        self
    }

    /// Raw platform settings: see [`Tweak`]. Progress bars have no semantic
    /// options past the value: what the platforms offer (sizes on AppKit,
    /// text on GTK, paused and error states on WinUI) is each one's own.
    pub fn native(mut self, tweak: Tweak<Progress>) -> Progress {
        tweak.apply(&mut self.element);
        self
    }
}

/// A picture, in the platform's image view: from a file, or from pixels
/// in memory. It's as large as the image (in points: pixels over their
/// scale) unless the layout sizes it; then `fit` says how it fills the
/// frame, or the platform does as it does by default. Its label is its
/// accessible name; without one it's decorative.
///
/// ```ignore
/// Image::file("logo.png").label("2ksbox")
/// Image::new(move || ImageSource::Pixels(frame.get())).label("Preview")
/// ```
pub struct Image(Element);

widget!(Image);

impl Image {
    pub fn new(source: impl IntoValue<ImageSource>) -> Image {
        let mut element = Element::new(WidgetKind::Image);
        element.prop(source.into_value(), Prop::Image);
        Image(element)
    }

    /// An image file, read by the platform.
    pub fn file(path: impl Into<std::path::PathBuf>) -> Image {
        Image::new(ImageSource::File(path.into()))
    }

    /// Pixels in memory.
    pub fn pixels(pixels: impl IntoValue<Pixels>) -> Image {
        let pixels = pixels.into_value();
        Image::new(match pixels {
            Value::Static(p) => Value::Static(ImageSource::Pixels(p)),
            dynamic => Value::Dynamic(Rc::new(move || ImageSource::Pixels(dynamic.get()))),
        })
    }

    /// Its accessible name: what the picture shows.
    pub fn label(mut self, label: impl IntoValue<String>) -> Image {
        self.0.prop(label.into_value(), Prop::Label);
        self
    }

    /// How it fills a frame of another size than its own.
    pub fn fit(mut self, fit: impl IntoValue<ImageFit>) -> Image {
        self.0.prop(fit.into_value(), Prop::ImageFit);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Image>) -> Image {
        tweak.apply(&mut self.0);
        self
    }
}

/// An icon from the platform's own set, by its name there: an SF Symbol
/// on macOS, a themed icon's name on Linux (Adwaita's symbolic ones on
/// GNOME, Breeze's on KDE), a Segoe Fluent Icons glyph on Windows. Names
/// differ, so pick one per platform with `platform!`. Symbolic icons are
/// drawn in the colour the platform gives icons, which follows dark mode.
/// It's as large as the platform makes icons unless
/// `icon_size` says otherwise. Its label is its accessible name; without
/// one it's decorative.
///
/// ```ignore
/// Icon::new(platform! {
///     macos => "trash",
///     gtk => "user-trash-symbolic",
///     kde => "edit-delete",
///     windows => "\u{E74D}",
/// })
/// ```
pub struct Icon(Element);

widget!(Icon);

impl Icon {
    pub fn new(name: impl IntoValue<String>) -> Icon {
        let mut element = Element::new(WidgetKind::Icon);
        element.prop(name.into_value(), Prop::Icon);
        Icon(element)
    }

    /// Its accessible name: what the icon stands for.
    pub fn label(mut self, label: impl IntoValue<String>) -> Icon {
        self.0.prop(label.into_value(), Prop::Label);
        self
    }

    /// Its colour, in place of the one the platform gives icons: a
    /// semantic one (`Accent`, `SecondaryLabel`, `Error`, …) follows the
    /// appearance; `Rgba` is fixed. Symbolic icons take it (SF Symbols,
    /// Adwaita's and Breeze's `-symbolic` icons, Segoe Fluent glyphs);
    /// icons in full colour keep theirs.
    pub fn color(mut self, color: impl IntoValue<Color>) -> Icon {
        self.0.prop(color.into_value(), Prop::TextColor);
        self
    }

    /// How big it is, in points: an SF Symbol's point size, as a font's
    /// (the symbol's own shape sets its frame), a square's side elsewhere.
    pub fn icon_size(mut self, points: impl IntoValue<f32>) -> Icon {
        self.0.prop(points.into_value(), Prop::IconSize);
        self
    }

    /// Raw platform settings, past the semantic ones: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Icon>) -> Icon {
        tweak.apply(&mut self.0);
        self
    }
}

/// A surface the app draws on with its own GPU API (wgpu, Vulkan, Metal,
/// Direct3D), at its own pace and on its own thread, as it would draw on a
/// window of its own: an `NSView` backed by a `CAMetalLayer` on AppKit.
/// It has no natural size; the layout sizes it.
///
/// `on_ready` gets its [`SurfaceHandle`] once the native surface exists,
/// and `on_resize` its size in pixels whenever that or its scale changes;
/// the handle's `size()` has it too, for a render thread. The handle keeps
/// the native surface alive after the widget is gone, so drop it (and the
/// GPU surface made on it) when the app is done presenting.
///
/// The surface sits above the window's own content, as the platform
/// layers it. Its label is its accessible name.
///
/// With `on_input` it takes keys and the pointer: a click or Tab focuses
/// it, and it gets every key the window doesn't take first for its menus'
/// shortcuts, Tab too (Control+Tab leaves it on AppKit and GTK, as it
/// leaves a text view). `pointer_lock` hides and holds the cursor and
/// reports its moves; `keyboard_grab` takes the system's shortcuts and
/// the app's own as well. The platform ends both when the window stops
/// being the active one (the grab also when the surface loses focus), and
/// sets their signals back to `false`: the app locks again, say, on the
/// next click. `cursor` is the cursor over it: the platform's, none, or
/// the app's own image.
///
/// ```ignore
/// GpuSurface::new()
///     .label("Machine")
///     .on_ready(move |surface| renderer.send(Message::Surface(surface)))
///     .on_resize(move |size| renderer.send(Message::Resize(size)))
///     .on_input(move |input| machine.send(Message::Input(input)))
///     .pointer_lock(captured)
///     .keyboard_grab(captured)
/// ```
pub struct GpuSurface(Element);

widget!(GpuSurface);

impl Default for GpuSurface {
    fn default() -> GpuSurface {
        GpuSurface::new()
    }
}

impl GpuSurface {
    pub fn new() -> GpuSurface {
        GpuSurface(Element::new(WidgetKind::GpuSurface))
    }

    /// Its accessible name: what the app draws there.
    pub fn label(mut self, label: impl IntoValue<String>) -> GpuSurface {
        self.0.prop(label.into_value(), Prop::Label);
        self
    }

    /// The native surface exists: make the GPU surface on it. Called once.
    pub fn on_ready(mut self, handler: impl Fn(SurfaceHandle) + 'static) -> GpuSurface {
        self.0.on(move |event| {
            if let UiEvent::SurfaceReady(surface) = event {
                handler(surface.clone());
            }
        });
        self
    }

    /// Its size in pixels, or its scale, changed: configure the GPU
    /// surface for it.
    pub fn on_resize(mut self, handler: impl Fn(SurfaceSize) + 'static) -> GpuSurface {
        self.0.on(move |event| {
            if let UiEvent::SurfaceResized(size) = event {
                handler(*size);
            }
        });
        self
    }

    /// Takes keys and the pointer, and reports them: a key down or up, the
    /// pointer's moves, buttons and scrolling over it, and while it's
    /// locked, how far it moved. Keys down when it loses focus, or its
    /// window stops being the active one, are reported released then.
    pub fn on_input(mut self, handler: impl Fn(SurfaceInput) + 'static) -> GpuSurface {
        self.0.prop(Value::Static(true), Prop::TakesInput);
        self.0.on(move |event| {
            if let UiEvent::SurfaceInput(input) = event {
                handler(*input);
            }
        });
        self
    }

    /// Hides and holds the cursor while `locked` is true, reporting its
    /// moves as `SurfaceInput::Motion`, and before the host's acceleration
    /// as `SurfaceInput::RawMotion`. The platform ends it when the
    /// window stops being the active one, and `locked` goes back to false.
    pub fn pointer_lock(mut self, locked: Signal<bool>) -> GpuSurface {
        self.0.prop(locked.into_value(), Prop::PointerLock);
        self.0.on(move |event| {
            if *event == UiEvent::PointerLockEnded {
                locked.set(false);
            }
        });
        self
    }

    /// The pointer's cursor over it, while it isn't locked: the
    /// platform's own, none, or an image of the app's (the one a machine
    /// gives its pointer, say).
    pub fn cursor(mut self, cursor: impl IntoValue<Cursor>) -> GpuSurface {
        self.0.prop(cursor.into_value(), Prop::Cursor);
        self
    }

    /// Takes every key while `grabbed` is true, the system's shortcuts
    /// (as far as the platform lets an app) and the window's own too, and
    /// focuses the surface. The platform ends it when the surface or its
    /// window loses focus, and `grabbed` goes back to false.
    pub fn keyboard_grab(mut self, grabbed: Signal<bool>) -> GpuSurface {
        self.0.prop(grabbed.into_value(), Prop::KeyboardGrab);
        self.0.on(move |event| {
            if *event == UiEvent::KeyboardGrabEnded {
                grabbed.set(false);
            }
        });
        self
    }
}

/// A spinner, as the platform draws one, for work of unknown length: a
/// spinning `NSProgressIndicator`, `gtk::Spinner`, `ProgressRing`,
/// `QQC2.BusyIndicator`. It spins while running (from the start, unless
/// told otherwise), and shows nothing while stopped, keeping its place. Its
/// label is its accessible name.
///
/// ```ignore
/// Spinner::new("Loading").running(move || loading.get())
/// ```
pub struct Spinner {
    element: Element,
    running: Value<bool>,
}

impl ElementBuilder for Spinner {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl View for Spinner {
    fn build(mut self, ui: &Ui) -> NodeId {
        self.element.prop(self.running, Prop::Running);
        self.element.build(ui)
    }
}

impl Spinner {
    pub fn new(label: impl IntoValue<String>) -> Spinner {
        let mut element = Element::new(WidgetKind::Spinner);
        element.prop(label.into_value(), Prop::Label);
        Spinner { element, running: Value::Static(true) }
    }

    /// Its label: the accessible name, which `new` takes. In `view!`,
    /// `<Spinner label="…"/>`.
    pub fn label(mut self, label: impl IntoValue<String>) -> Spinner {
        self.element.prop(label.into_value(), Prop::Label);
        self
    }

    /// Spins while true; shows nothing while false.
    pub fn running(mut self, running: impl IntoValue<bool>) -> Spinner {
        self.running = running.into_value();
        self
    }

    /// Raw platform settings: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Spinner>) -> Spinner {
        tweak.apply(&mut self.element);
        self
    }
}

/// A line between groups of content, as the platform draws one: a
/// separator `NSBox`, `gtk::Separator`, `Kirigami.Separator`, and on
/// WinUI, which has no separator control, a `Border` in the divider brush,
/// as Fluent apps draw one. Horizontal unless told otherwise, as thick as
/// the platform makes it; its length is the layout's, so it spans a
/// column (or a row, vertical) that stretches its children, as they do
/// by default.
///
/// ```ignore
/// Column::new().children((general(), Separator::new(), advanced()))
/// Row::new().children((back(), Separator::vertical(), forward()))
/// ```
pub struct Separator(Element);

widget!(Separator);

impl Separator {
    pub fn new() -> Separator {
        let mut element = Element::new(WidgetKind::Separator);
        element.prop(Value::Static(Orientation::Horizontal), Prop::Orientation);
        Separator(element)
    }

    /// A vertical one, between things side by side.
    pub fn vertical() -> Separator {
        Separator::new().orientation(Orientation::Vertical)
    }

    /// Which way it runs.
    pub fn orientation(mut self, orientation: impl IntoValue<Orientation>) -> Separator {
        self.0.prop(orientation.into_value(), Prop::Orientation);
        self
    }

    /// Raw platform settings: see [`Tweak`].
    pub fn native(mut self, tweak: Tweak<Separator>) -> Separator {
        tweak.apply(&mut self.0);
        self
    }
}

impl Default for Separator {
    fn default() -> Separator {
        Separator::new()
    }
}

// ----------------------------------------------------------- view! tags
//
// `view!` builds `<Tag …>children</Tag>` as
// `Tag::__tag().….__children(move || children)`: containers take their
// children, text and buttons their text.

impl Group {
    /// `<Group title="CD drive">…</Group>`
    #[doc(hidden)]
    pub fn __tag() -> Group {
        Group::new()
    }

    #[doc(hidden)]
    pub fn __children<C: Children>(self, children: impl FnOnce() -> C) -> Group {
        self.children(children())
    }
}

impl Container {
    #[doc(hidden)]
    pub fn __tag() -> Container {
        Container::new()
    }

    #[doc(hidden)]
    pub fn __children<C: Children>(self, children: impl FnOnce() -> C) -> Container {
        self.children(children())
    }
}

impl Window {
    /// `<Window title="Machine" bind=editing>…</Window>`: a `Window<()>`
    /// until `title` is set, and only then a `View`.
    #[doc(hidden)]
    pub fn __tag() -> Window<()> {
        let Window { title: _, size, modality, open, full_screen, min_size, on_open, on_close_request, content } =
            Window::new(String::new());
        Window { title: (), size, modality, open, full_screen, min_size, on_open, on_close_request, content }
    }

    /// Its content, built each time it opens.
    #[doc(hidden)]
    pub fn __children<V: View>(self, content: impl Fn() -> V + 'static) -> Window {
        self.content(content)
    }
}

impl Window<()> {
    pub fn title(self, title: impl IntoValue<String>) -> Window {
        let Window { title: (), size, modality, open, full_screen, min_size, on_open, on_close_request, content } =
            self;
        Window {
            title: title.into_value(),
            size,
            modality,
            open,
            full_screen,
            min_size,
            on_open,
            on_close_request,
            content,
        }
    }
}

impl Column {
    #[doc(hidden)]
    pub fn __tag() -> Container {
        Column::new()
    }
}

impl Row {
    #[doc(hidden)]
    pub fn __tag() -> Container {
        Row::new()
    }
}

impl Grid {
    #[doc(hidden)]
    pub fn __tag() -> Container {
        Grid::new()
    }
}

impl Toolbar {
    #[doc(hidden)]
    pub fn __tag() -> Toolbar {
        Toolbar::new()
    }

    #[doc(hidden)]
    pub fn __children<C: Children>(self, children: impl FnOnce() -> C) -> Toolbar {
        self.children(children())
    }
}

impl Sidebar<()> {
    #[doc(hidden)]
    pub fn __tag() -> SidebarWithoutSelection {
        SidebarWithoutSelection
    }
}

/// A `<Sidebar>` tag before its `selection`.
#[doc(hidden)]
pub struct SidebarWithoutSelection;

impl SidebarWithoutSelection {
    pub fn selection<T: PartialEq + Clone + 'static>(self, selection: Signal<T>) -> Sidebar<T> {
        Sidebar::new(selection)
    }
}

impl<T: PartialEq + Clone + 'static> Sidebar<T> {
    #[doc(hidden)]
    pub fn __children<E: SidebarEntries<T>>(self, children: impl FnOnce() -> E) -> Sidebar<T> {
        self.children(children())
    }
}

impl<T: 'static> SidebarSection<T> {
    #[doc(hidden)]
    pub fn __tag() -> SidebarSection<T> {
        SidebarSection::untitled()
    }

    #[doc(hidden)]
    pub fn __children<I: SidebarItems<T>>(self, items: impl FnOnce() -> I) -> SidebarSection<T> {
        self.children(items())
    }
}

impl SidebarItem<()> {
    /// Without a value until `value=` gives it one.
    #[doc(hidden)]
    pub fn __tag() -> SidebarItem<()> {
        SidebarItem::new(String::new(), ())
    }
}

impl<T: 'static> SidebarItem<T> {
    /// `<SidebarItem value=…>"General"</SidebarItem>`: its text is its title.
    #[doc(hidden)]
    pub fn __children<S: IntoValue<String>>(mut self, title: impl FnOnce() -> S) -> SidebarItem<T> {
        self.title = title().into_value();
        self
    }
}

impl Tabs<()> {
    #[doc(hidden)]
    pub fn __tag() -> TabsWithoutSelection {
        TabsWithoutSelection
    }
}

/// A `<Tabs>` tag before its `selection`.
#[doc(hidden)]
pub struct TabsWithoutSelection;

impl TabsWithoutSelection {
    pub fn selection<T: PartialEq + Clone + 'static>(self, selection: Signal<T>) -> Tabs<T> {
        Tabs::new(selection)
    }
}

impl<T: PartialEq + Clone + 'static> Tabs<T> {
    #[doc(hidden)]
    pub fn __children<C: TabsChildren<T>>(self, children: impl FnOnce() -> C) -> Tabs<T> {
        self.children(children())
    }
}

impl Tab<()> {
    /// Without a value until `value=` gives it one.
    #[doc(hidden)]
    pub fn __tag() -> Tab<()> {
        Tab::new(String::new(), ())
    }
}

impl<T: 'static> Tab<T> {
    #[doc(hidden)]
    pub fn __children<C: Children>(self, children: impl FnOnce() -> C) -> Tab<T> {
        self.children(children())
    }
}

impl ScrollView {
    #[doc(hidden)]
    pub fn __tag() -> ScrollView {
        ScrollView::new()
    }

    #[doc(hidden)]
    pub fn __children<C: Children>(self, children: impl FnOnce() -> C) -> ScrollView {
        self.children(children())
    }
}

/// Widgets whose one child is their text: `<Text>"Hello"</Text>`.
macro_rules! text_tag {
    ($t:ident, $prop:ident) => {
        impl $t {
            #[doc(hidden)]
            pub fn __tag() -> $t {
                $t(Element::new(WidgetKind::$t))
            }

            #[doc(hidden)]
            pub fn __children<S: IntoValue<String>>(mut self, text: impl FnOnce() -> S) -> $t {
                self.0.prop(text().into_value(), Prop::$prop);
                self
            }
        }
    };
}

text_tag!(Text, Text);
text_tag!(Button, Label);
text_tag!(MenuButton, Label);
text_tag!(Checkbox, Label);
text_tag!(Switch, Label);

impl TextInput {
    #[doc(hidden)]
    pub fn __tag() -> TextInput {
        TextInput::new()
    }
}

impl PasswordInput {
    #[doc(hidden)]
    pub fn __tag() -> PasswordInput {
        PasswordInput::new()
    }
}

impl SearchInput {
    #[doc(hidden)]
    pub fn __tag() -> SearchInput {
        SearchInput::new()
    }
}

impl TextArea {
    #[doc(hidden)]
    pub fn __tag() -> TextArea {
        TextArea::new()
    }
}

impl Select {
    /// `<Select a11y_label="Color" options=["Red", "Green"] bind=color/>`
    #[doc(hidden)]
    pub fn __tag() -> Select {
        Select {
            element: Element::new(WidgetKind::Select),
            options: Value::Static(Vec::new()),
            selected: Value::Static(0),
        }
    }
}

impl RadioGroup {
    /// `<RadioGroup a11y_label="Size" options=["Small", "Large"] bind=size/>`
    #[doc(hidden)]
    pub fn __tag() -> RadioGroup {
        RadioGroup {
            element: Element::new(WidgetKind::RadioGroup),
            options: Value::Static(Vec::new()),
            selected: Value::Static(None),
        }
    }
}

impl Slider {
    /// `<Slider a11y_label="Volume" bind=volume/>`
    #[doc(hidden)]
    pub fn __tag() -> Slider {
        Slider {
            element: Element::new(WidgetKind::Slider),
            range: Value::Static((0.0, 100.0)),
            value: Value::Static(0.0),
        }
    }
}

impl Image {
    /// `<Image source=ImageSource::File("logo.png".into()) label="Logo"/>`
    #[doc(hidden)]
    pub fn __tag() -> Image {
        Image(Element::new(WidgetKind::Image))
    }

    #[doc(hidden)]
    pub fn source(mut self, source: impl IntoValue<ImageSource>) -> Image {
        self.0.prop(source.into_value(), Prop::Image);
        self
    }
}

impl Icon {
    /// `<Icon name="trash" label="Delete"/>`
    #[doc(hidden)]
    pub fn __tag() -> Icon {
        Icon(Element::new(WidgetKind::Icon))
    }

    #[doc(hidden)]
    pub fn name(mut self, name: impl IntoValue<String>) -> Icon {
        self.0.prop(name.into_value(), Prop::Icon);
        self
    }
}

impl GpuSurface {
    /// `<GpuSurface label="Machine" @ready=… @resize=… @input=… pointer_lock=captured/>`
    #[doc(hidden)]
    pub fn __tag() -> GpuSurface {
        GpuSurface::new()
    }
}

impl NumberInput {
    /// `<NumberInput a11y_label="Copies" bind=copies/>`
    #[doc(hidden)]
    pub fn __tag() -> NumberInput {
        NumberInput {
            element: Element::new(WidgetKind::NumberInput),
            range: Value::Static((0, 100)),
            value: Value::Static(0),
        }
    }
}

impl Spinner {
    /// `<Spinner a11y_label="Loading" running=loading/>`
    #[doc(hidden)]
    pub fn __tag() -> Spinner {
        Spinner { element: Element::new(WidgetKind::Spinner), running: Value::Static(true) }
    }
}

impl Separator {
    /// `<Separator/>`, `<Separator orientation=Orientation::Vertical/>`
    #[doc(hidden)]
    pub fn __tag() -> Separator {
        Separator::new()
    }
}

impl Progress {
    /// `<Progress a11y_label="Upload" value=done/>`
    #[doc(hidden)]
    pub fn __tag() -> Progress {
        Progress { element: Element::new(WidgetKind::Progress), value: None, indeterminate: Value::Static(false) }
    }
}
