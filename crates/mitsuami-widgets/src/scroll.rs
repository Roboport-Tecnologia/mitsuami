//! Scroll views.

use crate::Container;
use mitsuami_core::{
    Align, Children, Element, ElementBuilder, FlexDirection, NodeId, Point, Prop, ScrollAxes, Tweak, Ui, UiEvent, View,
    WidgetKind,
};
use mitsuami_reactive::IntoValue;

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
