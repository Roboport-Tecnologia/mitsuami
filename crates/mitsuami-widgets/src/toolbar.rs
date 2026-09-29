//! The window's toolbar.

use mitsuami_core::{AnyView, Children, CurrentWindow, NodeId, Ui, View, WidgetKind};
use mitsuami_reactive::{inject, on_cleanup};

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
