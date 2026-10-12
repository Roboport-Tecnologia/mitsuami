//! Document tabs: a strip of tabs without pages.

use std::cell::Cell;
use std::rc::Rc;

use mitsuami_core::{
    DocumentTabsStyle, Element, ElementBuilder, EventValue, NodeId, Prop, Tweak, Ui, UiEvent, View, WidgetKind,
};
use mitsuami_reactive::{IntoValue, Signal, Value, effect, signal};

/// Tabs for the documents the app has open, as a browser's or a
/// terminal's: the platform's own (in the title bar on macOS, as Safari's
/// and Xcode's, and on Windows; libadwaita's tab bar on GNOME, a tab bar
/// on KDE). The tabs have no pages: the app shows the chosen document itself,
/// in one place, as a terminal draws its sessions on one surface.
///
/// Its tabs are [`DocumentTab`]s, usually built from the app's list of
/// documents with `children_with`. `selection` holds the value of the
/// chosen tab, and choosing another sets it. One is always chosen while
/// there are tabs, as on every platform: a value no tab has chooses the
/// first, and when the chosen tab goes, the one now in its place (the
/// last, if it was last); either sets `selection`.
///
/// The user closes tabs, moves them and asks for new ones where the app
/// lets them: `on_close`, `on_reorder` and `on_new_tab` each turn the
/// platform's own way on (close buttons, dragging, a new-tab button). A
/// close only asks: the tab stays until the app takes its document away.
///
/// ```ignore
/// let docs = signal(vec![Doc::new(1, "notes.txt")]);
/// let current = signal(1);
/// DocumentTabs::new(current)
///     .children_with(move || docs.with(|d| d.iter().map(|d| DocumentTab::new(d.name.clone(), d.id)).collect::<Vec<_>>()))
///     .on_close(move |id| docs.update(|d| d.retain(|d| d.id != id)))
///     .on_new_tab(move || /* open one, and choose it */)
/// ```
///
/// Where the platform puts its tabs in the window's chrome (macOS,
/// Windows), they take no room where they're declared; see
/// [`tabs_style`](Self::tabs_style).
///
/// In `view!`: `<DocumentTabs selection=current @close=… @new_tab=…>`,
/// with `<DocumentTab value=1>"notes.txt"</DocumentTab>`s as children.
pub struct DocumentTabs<T: 'static> {
    element: Element,
    selection: Signal<T>,
    tabs: Vec<DocumentTab<T>>,
    /// Built again whenever what it reads changes, after `tabs`.
    dynamic: Option<Box<dyn Fn() -> Vec<DocumentTab<T>>>>,
    close: Option<Rc<dyn Fn(T)>>,
    reorder: Option<Rc<dyn Fn(T, usize)>>,
    new_tab: Option<Rc<dyn Fn()>>,
}

impl<T: 'static> ElementBuilder for DocumentTabs<T> {
    fn element(&mut self) -> &mut Element {
        &mut self.element
    }
}

impl<T: PartialEq + Clone + 'static> DocumentTabs<T> {
    pub fn new(selection: Signal<T>) -> DocumentTabs<T> {
        DocumentTabs {
            element: Element::new(WidgetKind::DocumentTabs),
            selection,
            tabs: Vec::new(),
            dynamic: None,
            close: None,
            reorder: None,
            new_tab: None,
        }
    }

    /// Its tabs, in order.
    pub fn children(mut self, tabs: impl DocumentTabsChildren<T>) -> DocumentTabs<T> {
        tabs.add_to(&mut self.tabs);
        self
    }

    pub fn tab(self, tab: DocumentTab<T>) -> DocumentTabs<T> {
        self.children(tab)
    }

    /// Tabs built by `tabs`, and built again whenever what it reads changes
    /// (the app's documents), after any `children`.
    pub fn children_with<E: DocumentTabsChildren<T>>(mut self, tabs: impl Fn() -> E + 'static) -> DocumentTabs<T> {
        self.dynamic = Some(Box::new(move || {
            let mut built = Vec::new();
            tabs().add_to(&mut built);
            built
        }));
        self
    }

    /// Called with a tab's value when the user asks to close it: its close
    /// button, a middle click, the platform's own way. Gives the tabs
    /// close buttons. The tab stays until the app takes it away, so it can
    /// ask about unsaved changes first.
    pub fn on_close(mut self, close: impl Fn(T) + 'static) -> DocumentTabs<T> {
        self.close = Some(Rc::new(close));
        self
    }

    /// Called with a tab's value and its new index when the user moves it
    /// (drags it, or moves it from the keyboard where the platform can).
    /// Lets the user move tabs. The tab already shows there: move the
    /// document in the app's list too.
    pub fn on_reorder(mut self, reorder: impl Fn(T, usize) + 'static) -> DocumentTabs<T> {
        self.reorder = Some(Rc::new(reorder));
        self
    }

    /// Called when the user asks for a new tab with the platform's new-tab
    /// button, which this gives the tabs: open a document, and choose it.
    pub fn on_new_tab(mut self, new_tab: impl Fn() + 'static) -> DocumentTabs<T> {
        self.new_tab = Some(Rc::new(new_tab));
        self
    }

    /// Where it shows its tabs: the platform's own place (the default), a
    /// strip where it's declared on every platform, the window's own (the
    /// title bar on macOS and Windows; GNOME and KDE have none, and show a
    /// strip), or the title's own row (Safari's compact tabs on macOS, the
    /// header bar's title on GNOME; KDE shows a strip). In `view!`, after
    /// `selection`.
    pub fn tabs_style(mut self, style: impl IntoValue<DocumentTabsStyle>) -> DocumentTabs<T> {
        self.element.prop(style.into_value(), Prop::DocumentTabsStyle);
        self
    }

    /// Raw platform settings: see [`Tweak`]. Pinned tabs, tabs dragged out
    /// to a window of their own, a tab's width: each platform's own.
    pub fn native(mut self, tweak: Tweak<DocumentTabs<T>>) -> DocumentTabs<T> {
        tweak.apply(&mut self.element);
        self
    }
}

impl<T: PartialEq + Clone + 'static> View for DocumentTabs<T> {
    fn build(self, ui: &Ui) -> NodeId {
        let DocumentTabs { mut element, selection, tabs, dynamic, close, reorder, new_tab } = self;
        if close.is_some() {
            element.prop(Value::Static(true), Prop::TabsClosable);
        }
        if reorder.is_some() {
            element.prop(Value::Static(true), Prop::TabsReorderable);
        }
        if new_tab.is_some() {
            element.prop(Value::Static(true), Prop::NewTabButton);
        }
        // The tabs' values in order, which an index from the platform
        // names: they change when `children_with` builds new tabs, and when
        // the user moves one.
        let values: Signal<Vec<T>> = signal(Vec::new());
        let value_at = move |index: usize| values.with_untracked(|v| v.get(index).cloned());
        element.on(move |event| match event {
            UiEvent::Changed(EventValue::Index(index)) => {
                if let Some(value) = value_at(*index) {
                    selection.set(value);
                }
            }
            UiEvent::TabCloseRequested(index) => {
                if let (Some(close), Some(value)) = (&close, value_at(*index)) {
                    close(value);
                }
            }
            // The platform shows it moved, and the core has its titles so:
            // the values follow, before the app hears.
            UiEvent::TabMoved { from, to } => {
                let Some(value) = value_at(*from) else { return };
                let mut now = values.get_untracked();
                if *to >= now.len() {
                    return;
                }
                let moved = now.remove(*from);
                now.insert(*to, moved);
                values.set(now);
                if let Some(reorder) = &reorder {
                    reorder(value, *to);
                }
            }
            UiEvent::NewTabRequested => {
                if let Some(new_tab) = &new_tab {
                    new_tab();
                }
            }
            _ => {}
        });
        let strip = element.build(ui);
        // The titles first: the selection is an index into them.
        let titled = ui.clone();
        effect(move || {
            let built = dynamic.as_ref().map(|tabs| tabs()).unwrap_or_default();
            let all: Vec<&DocumentTab<T>> = tabs.iter().chain(&built).collect();
            let now: Vec<T> = all.iter().map(|t| t.value.clone()).collect();
            if values.with_untracked(|v| *v != now) {
                values.set(now);
            }
            titled.set_prop(strip, Prop::TabTitles(all.iter().map(|t| t.title.get()).collect()));
        });
        // Where the chosen tab was, for the one that takes its place.
        let last = Rc::new(Cell::new(0));
        let chosen = ui.clone();
        effect(move || {
            let index = values.with(|v| {
                if v.is_empty() {
                    return None;
                }
                Some(selection.with(|value| v.iter().position(|v| v == value)).unwrap_or_else(|| {
                    let index = last.get().min(v.len() - 1);
                    selection.set(v[index].clone());
                    index
                }))
            });
            if let Some(index) = index {
                last.set(index);
            }
            chosen.set_prop(strip, Prop::SelectedIndex(index));
        });
        strip
    }
}

/// A tab of [`DocumentTabs`]: its title, and the value choosing it gives
/// the tabs' selection.
///
/// In `view!`, its text is its title:
/// `<DocumentTab value=1>"notes.txt"</DocumentTab>`.
pub struct DocumentTab<T: 'static> {
    title: Value<String>,
    value: T,
}

impl<T: 'static> DocumentTab<T> {
    pub fn new(title: impl IntoValue<String>, value: T) -> DocumentTab<T> {
        DocumentTab { title: title.into_value(), value }
    }

    pub fn title(mut self, title: impl IntoValue<String>) -> DocumentTab<T> {
        self.title = title.into_value();
        self
    }

    /// The value choosing it gives the tabs' selection.
    pub fn value<U: 'static>(self, value: U) -> DocumentTab<U> {
        DocumentTab { title: self.title, value }
    }
}

/// What [`DocumentTabs`] take as children: [`DocumentTab`]s, and tuples,
/// `Vec`s and `Option`s of them.
pub trait DocumentTabsChildren<T: 'static> {
    fn add_to(self, tabs: &mut Vec<DocumentTab<T>>);
}

impl<T: 'static> DocumentTabsChildren<T> for DocumentTab<T> {
    fn add_to(self, tabs: &mut Vec<DocumentTab<T>>) {
        tabs.push(self);
    }
}

impl<T: 'static, C: DocumentTabsChildren<T>> DocumentTabsChildren<T> for Vec<C> {
    fn add_to(self, tabs: &mut Vec<DocumentTab<T>>) {
        for tab in self {
            tab.add_to(tabs);
        }
    }
}

impl<T: 'static, C: DocumentTabsChildren<T>> DocumentTabsChildren<T> for Option<C> {
    fn add_to(self, tabs: &mut Vec<DocumentTab<T>>) {
        if let Some(tab) = self {
            tab.add_to(tabs);
        }
    }
}

impl<T: 'static> DocumentTabsChildren<T> for () {
    fn add_to(self, _: &mut Vec<DocumentTab<T>>) {}
}

macro_rules! tuple_document_tabs {
    ($($name:ident),+) => {
        impl<T: 'static, $($name: DocumentTabsChildren<T>),+> DocumentTabsChildren<T> for ($($name,)+) {
            #[allow(non_snake_case)]
            fn add_to(self, tabs: &mut Vec<DocumentTab<T>>) {
                let ($($name,)+) = self;
                $($name.add_to(tabs);)+
            }
        }
    };
}

tuple_document_tabs!(A);
tuple_document_tabs!(A, B);
tuple_document_tabs!(A, B, C);
tuple_document_tabs!(A, B, C, D);
tuple_document_tabs!(A, B, C, D, E);
tuple_document_tabs!(A, B, C, D, E, F);
tuple_document_tabs!(A, B, C, D, E, F, G);
tuple_document_tabs!(A, B, C, D, E, F, G, H);

impl DocumentTabs<()> {
    #[doc(hidden)]
    pub fn __tag() -> DocumentTabsWithoutSelection {
        DocumentTabsWithoutSelection
    }
}

/// A `<DocumentTabs>` tag before its `selection`.
#[doc(hidden)]
pub struct DocumentTabsWithoutSelection;

impl DocumentTabsWithoutSelection {
    pub fn selection<T: PartialEq + Clone + 'static>(self, selection: Signal<T>) -> DocumentTabs<T> {
        DocumentTabs::new(selection)
    }
}

impl<T: PartialEq + Clone + 'static> DocumentTabs<T> {
    #[doc(hidden)]
    pub fn __children<C: DocumentTabsChildren<T>>(self, tabs: impl FnOnce() -> C) -> DocumentTabs<T> {
        self.children(tabs())
    }
}

impl DocumentTab<()> {
    /// Without a value until `value=` gives it one.
    #[doc(hidden)]
    pub fn __tag() -> DocumentTab<()> {
        DocumentTab::new(String::new(), ())
    }
}

impl<T: 'static> DocumentTab<T> {
    /// `<DocumentTab value=…>"notes.txt"</DocumentTab>`: its text is its
    /// title.
    #[doc(hidden)]
    pub fn __children<S: IntoValue<String>>(mut self, title: impl FnOnce() -> S) -> DocumentTab<T> {
        self.title = title().into_value();
        self
    }
}
