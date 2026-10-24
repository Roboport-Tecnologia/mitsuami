//! A popover: content the platform shows by a container, over the window.

use mitsuami_core::{
    Children, Display, Element, ElementBuilder, FlexDirection, Length, NodeId, Prop, Ui, UiEvent, View, WidgetKind,
};
use mitsuami_reactive::{IntoValue, Signal};

/// The platform's popover (an `NSPopover`, a `gtk::Popover`, a XAML
/// `Flyout`, a `QQC2.Popup`), shown by its parent while `shown` is true.
/// Put it in the container it points at, after what that shows; it takes
/// no room there. Its children are laid out as a column's at their natural
/// size, and the platform places it by its parent, where it fits. The user
/// dismisses it as the platform lets them (a click outside, Escape), which
/// sets `shown` back to false.
///
/// ```ignore
/// let open = signal(false);
/// Row::new().children((
///     Button::new("Contains in").on_click(move || open.set(true)),
///     Popover::new(open).padding(Spacing::Md).child(branches),
/// ))
/// ```
pub struct Popover(Element);
widget!(Popover);

impl Popover {
    pub fn new(shown: Signal<bool>) -> Popover {
        let mut element = Element::new(WidgetKind::Popover);
        element.prop(shown.into_value(), Prop::PopoverShown);
        element.on(move |event| {
            if let UiEvent::PopoverShownChanged(on) = event {
                shown.set(*on);
            }
        });
        Popover(element).style(|s| {
            s.display = Display::Flex;
            s.flex_direction = FlexDirection::Column;
        })
    }

    /// Space between its children.
    pub fn gap(mut self, gap: impl IntoValue<Length>) -> Popover {
        self.0.style_prop(gap.into_value(), |s, v| {
            s.row_gap = v;
            s.column_gap = v;
        });
        self
    }

    pub fn children(mut self, children: impl Children) -> Popover {
        self.0.add_children(children);
        self
    }

    pub fn child(self, child: impl View) -> Popover {
        self.children(child)
    }

    /// `<Popover shown={open}>…</Popover>`
    #[doc(hidden)]
    pub fn __tag(shown: Signal<bool>) -> Popover {
        Popover::new(shown)
    }

    #[doc(hidden)]
    pub fn __children<C: Children>(self, children: impl FnOnce() -> C) -> Popover {
        self.children(children())
    }
}
