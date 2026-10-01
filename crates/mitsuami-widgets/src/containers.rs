//! Layout containers: flexbox, grid and titled groups.

use std::rc::Rc;

use std::path::PathBuf;

use mitsuami_core::services::Shortcut;
use mitsuami_core::{
    Align, Children, Display, Element, ElementBuilder, FileDrop, FlexDirection, Justify, Length, NodeId, Prop, Track,
    Tweak, Ui, UiEvent, View, WidgetKind,
};
use mitsuami_reactive::{IntoValue, Value};

/// A layout host: flexbox (column by default) or grid.
pub struct Container(pub(crate) Element);
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

    /// Called with `true` when the pointer comes over it, or over anything
    /// inside it, and `false` when it leaves, as the platform tracks it:
    /// to show a row's buttons while it's hovered. Keyboards and
    /// assistive technology never hover, so whatever it shows must be
    /// reachable another way too (a context menu, a selected row's
    /// toolbar).
    pub fn on_hover(mut self, handler: impl Fn(bool) + 'static) -> Container {
        self.0.on_hover(handler);
        self
    }

    /// Runs `handler` when `key` is pressed while it, or a control
    /// inside it, has keyboard focus, and the focused control doesn't use
    /// the key itself: Space for a preview, Delete for Move to Trash.
    /// A key goes to the nearest node around the focused control that
    /// takes it. Nothing shows keys taken this way, so give the command a
    /// menu item or a button too, and pick the keys the platform's own apps
    /// use (`platform!`).
    ///
    /// ```ignore
    /// Column::new().on_key(Shortcut::primary(Key::Backspace), trash)
    /// ```
    pub fn on_key(mut self, key: impl Into<Shortcut>, handler: impl Fn() + 'static) -> Container {
        self.0.on_key(key.into(), handler);
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

    /// Called with `true` when the pointer comes over it, or over anything
    /// inside it, and `false` when it leaves, as the platform tracks it:
    /// to show a row's buttons while it's hovered. Keyboards and
    /// assistive technology never hover, so whatever it shows must be
    /// reachable another way too (a context menu, a selected row's
    /// toolbar).
    pub fn on_hover(mut self, handler: impl Fn(bool) + 'static) -> Group {
        self.0.on_hover(handler);
        self
    }

    /// Runs `handler` when `key` is pressed while it, or a control
    /// inside it, has keyboard focus, and the focused control doesn't use
    /// the key itself: Space for a preview, Delete for Move to Trash.
    /// A key goes to the nearest node around the focused control that
    /// takes it. Nothing shows keys taken this way, so give the command a
    /// menu item or a button too, and pick the keys the platform's own apps
    /// use (`platform!`).
    ///
    /// ```ignore
    /// Group::new().on_key(Shortcut::primary(Key::Backspace), trash)
    /// ```
    pub fn on_key(mut self, key: impl Into<Shortcut>, handler: impl Fn() + 'static) -> Group {
        self.0.on_key(key.into(), handler);
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
