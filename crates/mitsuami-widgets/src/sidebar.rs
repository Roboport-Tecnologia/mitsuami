//! The window's sidebar, its sections and items.

use std::rc::Rc;

use mitsuami_core::{
    CurrentWindow, EventValue, NodeId, Prop, SidebarItemData, SidebarSectionData, Ui, UiEvent, View, WidgetKind,
};
use mitsuami_reactive::{IntoValue, Signal, Value, effect, inject, on_cleanup};

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
    shown: Option<Signal<bool>>,
}

impl<T: PartialEq + Clone + 'static> Sidebar<T> {
    pub fn new(selection: Signal<T>) -> Sidebar<T> {
        Sidebar { selection, sections: Vec::new(), shown: None }
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
        let Sidebar { selection, sections, shown: showing } = self;
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
