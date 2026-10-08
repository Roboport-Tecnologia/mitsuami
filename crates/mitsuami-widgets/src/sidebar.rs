//! The window's sidebar, its sections and items.

use std::rc::Rc;

use mitsuami_core::services::{ItemMenus, Menu, MenuEntries};
use mitsuami_core::{
    CurrentWindow, Element, EventValue, NodeId, Prop, SidebarItemData, SidebarSectionData, Tweak, Ui, UiEvent, View,
    WidgetKind,
};
use mitsuami_reactive::{IntoValue, Signal, Value, effect, inject, on_cleanup, signal};

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
/// Items can be what the window is about rather than its pages, as
/// Mail's mailboxes or a VM manager's machines are: a `subtitle` under
/// the title, a `context_menu` each, `on_activate` for a double-click or
/// Enter, and `children_with` for items that come and go.
///
/// In `view!`: `<Sidebar selection=page>`, with
/// `<SidebarItem value=Page::General>"General"</SidebarItem>`s and
/// `<SidebarSection title="Network">`s as children.
pub struct Sidebar<T: 'static> {
    selection: Signal<T>,
    sections: Vec<SidebarSection<T>>,
    /// Built again whenever what it reads changes, after `sections`.
    dynamic: Option<Box<dyn Fn() -> Vec<SidebarSection<T>>>>,
    shown: Option<Signal<bool>>,
    activate: Option<Rc<dyn Fn(T)>>,
    tweak: Option<Tweak<Sidebar<T>>>,
}

impl<T: PartialEq + Clone + 'static> Sidebar<T> {
    pub fn new(selection: Signal<T>) -> Sidebar<T> {
        Sidebar { selection, sections: Vec::new(), dynamic: None, shown: None, activate: None, tweak: None }
    }

    /// Items and sections built by `entries`, and built again whenever what
    /// it reads changes (a list of documents, of machines), after any
    /// `children`. An item whose value is gone stops being chosen, and the
    /// selection is left as it is.
    pub fn children_with<E: SidebarEntries<T>>(mut self, entries: impl Fn() -> E + 'static) -> Sidebar<T> {
        self.dynamic = Some(Box::new(move || {
            let mut sections = Vec::new();
            entries().add_to(&mut sections);
            sections
        }));
        self
    }

    /// Called with an item's value when it's activated, where the
    /// platform's sidebar has that, as a list's row is: double-clicked or
    /// Enter pressed on it (AppKit, GTK, Kirigami; WinUI's double-tap).
    /// The item is chosen first.
    pub fn on_activate(mut self, activate: impl Fn(T) + 'static) -> Sidebar<T> {
        self.activate = Some(Rc::new(activate));
        self
    }

    /// Shown beside the window's content while `shown` is true, hidden
    /// otherwise, as the platform shows and hides a sidebar. The user can
    /// change it too (the platform's toggle, its divider dragged away),
    /// which sets the signal. Where the platform's sidebar can't be hidden
    /// in a wide window (GTK), it's the page shown in a narrow one.
    pub fn shown(mut self, shown: Signal<bool>) -> Sidebar<T> {
        self.shown = Some(shown);
        self
    }

    /// Raw platform settings: see [`Tweak`]. How wide a sidebar is, say,
    /// is each platform's own.
    pub fn native(mut self, tweak: Tweak<Sidebar<T>>) -> Sidebar<T> {
        self.tweak = Some(tweak);
        self
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
        let Sidebar { selection, sections, dynamic, shown: showing, activate, tweak } = self;
        // The items' values in order, which an index from the platform
        // names: they change when `children_with` builds new items.
        let values: Signal<Vec<T>> = signal(Vec::new());
        let menus = ItemMenus::new();
        let mut element = Element::new(WidgetKind::Sidebar);
        if let Some(tweak) = tweak {
            tweak.apply(&mut element);
        }
        let sidebar = element.build(ui);
        ui.append_child(window, sidebar);
        // The items first: the selection is an index into them.
        let (shown, item_menus) = (ui.clone(), menus.clone());
        effect(move || {
            let built = dynamic.as_ref().map(|entries| entries()).unwrap_or_default();
            let all: Vec<&SidebarSection<T>> = sections.iter().chain(&built).collect();
            let items = all.iter().flat_map(|s| &s.items);
            let mut entries =
                item_menus.collect(&items.clone().map(|i| i.menu.as_ref()).collect::<Vec<_>>()).into_iter();
            let data = all
                .iter()
                .map(|section| SidebarSectionData {
                    title: section.title.as_ref().map(|t| t.get()),
                    items: section
                        .items
                        .iter()
                        .map(|item| SidebarItemData {
                            title: item.title.get(),
                            icon: item.icon.clone(),
                            subtitle: item.subtitle.as_ref().map(|s| s.get()),
                            menu: entries.next().unwrap_or_default(),
                        })
                        .collect(),
                })
                .collect();
            let now: Vec<T> = items.map(|i| i.value.clone()).collect();
            if values.with_untracked(|v| *v != now) {
                values.set(now);
            }
            shown.set_prop(sidebar, Prop::Sections(data));
        });
        let chosen = ui.clone();
        effect(move || {
            let index = selection.with(|value| values.with(|v| v.iter().position(|v| v == value)));
            chosen.set_prop(sidebar, Prop::SelectedIndex(index));
        });
        let value_at = move |index: usize| values.with_untracked(|v| v.get(index).cloned());
        ui.on_event(sidebar, move |event| match event {
            UiEvent::Changed(EventValue::Index(index)) => {
                if let Some(value) = value_at(*index) {
                    selection.set(value);
                }
            }
            UiEvent::SidebarItemActivated(index) => {
                if let (Some(activate), Some(value)) = (&activate, value_at(*index)) {
                    activate(value);
                }
            }
            UiEvent::ContextMenuItem(_) => menus.handle(event),
            _ => {}
        });
        if let Some(showing) = showing {
            let shown = ui.clone();
            effect(move || shown.set_prop(sidebar, Prop::SidebarShown(showing.get())));
            ui.on_event(sidebar, move |event| {
                if let UiEvent::SidebarShownChanged(on) = event {
                    showing.set(*on);
                }
            });
        }
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
/// sidebar's selection, and optionally an icon, a subtitle and a context
/// menu.
///
/// In `view!`, its text is its title:
/// `<SidebarItem value=Page::General icon="gearshape">"General"</SidebarItem>`.
pub struct SidebarItem<T: 'static> {
    title: Value<String>,
    icon: Option<String>,
    subtitle: Option<Value<String>>,
    menu: Option<Menu>,
    value: T,
}

impl<T: 'static> SidebarItem<T> {
    pub fn new(title: impl IntoValue<String>, value: T) -> SidebarItem<T> {
        SidebarItem { title: title.into_value(), icon: None, subtitle: None, menu: None, value }
    }

    /// The value choosing it gives the sidebar's selection.
    pub fn value<U: 'static>(self, value: U) -> SidebarItem<U> {
        SidebarItem { title: self.title, icon: self.icon, subtitle: self.subtitle, menu: self.menu, value }
    }

    /// A second line under the title, in the platform's secondary style
    /// (a machine's system and state, a mailbox's account). The row is as
    /// tall as the two lines need.
    pub fn subtitle(mut self, subtitle: impl IntoValue<String>) -> SidebarItem<T> {
        self.subtitle = Some(subtitle.into_value());
        self
    }

    /// Its context menu, which the platform shows its own way on the item
    /// (a right-click, a long press, the menu key). Takes what
    /// [`ElementBuilder::context_menu`](mitsuami_core::ElementBuilder::context_menu)
    /// takes, with the same reactive titles and states.
    pub fn context_menu(mut self, entries: impl MenuEntries) -> SidebarItem<T> {
        self.menu = Some(Menu::new(String::new()).children(entries));
        self
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
