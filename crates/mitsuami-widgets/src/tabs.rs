//! Tab views and their pages.

use std::rc::Rc;

use crate::{Column, Container};
use mitsuami_core::{
    Children, Element, ElementBuilder, EventValue, Length, NodeId, Prop, TabsStyle, Ui, UiEvent, View, WidgetKind,
};
use mitsuami_reactive::{IntoValue, Signal, Value};

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

    /// How it shows its tabs, where the platform has more than one way:
    /// navigation tabs (the default: libadwaita's view switcher,
    /// Kirigami's navigation bar) or a tab bar (GTK's notebook, Qt's tab
    /// bar) on GNOME and KDE. The others show theirs. In `view!`, after
    /// `selection`.
    pub fn tabs_style(mut self, style: impl IntoValue<TabsStyle>) -> Tabs<T> {
        self.element.prop(style.into_value(), Prop::TabsStyle);
        self
    }
}

impl<T: PartialEq + Clone + 'static> View for Tabs<T> {
    fn build(self, ui: &Ui) -> NodeId {
        let Tabs { mut element, selection, tabs } = self;
        let titles: Vec<Value<String>> = tabs.iter().map(|t| t.title.clone()).collect();
        let values: Rc<Vec<T>> = Rc::new(tabs.iter().map(|t| t.value.clone()).collect());
        // The titles first: the selection is an index into them.
        element.prop(Value::Dynamic(Rc::new(move || titles.iter().map(|t| t.get()).collect())), Prop::TabTitles);
        if tabs.iter().any(|t| t.icon.is_some()) {
            let icons: Vec<Option<Value<String>>> = tabs.iter().map(|t| t.icon.clone()).collect();
            element.prop(
                Value::Dynamic(Rc::new(move || {
                    icons.iter().map(|i| i.as_ref().map(|i| i.get()).unwrap_or_default()).collect()
                })),
                Prop::TabIcons,
            );
        }
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
    icon: Option<Value<String>>,
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
        Tab { title: title.into_value(), icon: None, value, page: Column::new() }
    }

    pub fn title(mut self, title: impl IntoValue<String>) -> Tab<T> {
        self.title = title.into_value();
        self
    }

    /// An icon on its tab, named in the platform's own set as for
    /// [`Icon`](crate::Icon); empty: none. Shown where the platform's tabs
    /// show one (libadwaita's view switcher, WinUI's selector bar, Qt's tabs);
    /// AppKit's tab view shows titles only.
    pub fn icon(mut self, name: impl IntoValue<String>) -> Tab<T> {
        self.icon = Some(name.into_value());
        self
    }

    /// The value picking it gives the tabs' selection.
    pub fn value<U: 'static>(self, value: U) -> Tab<U> {
        Tab { title: self.title, icon: self.icon, value, page: self.page }
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
