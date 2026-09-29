//! Scroll views: adjustments, offsets and policies.

use gtk::prelude::*;
use mitsuami_core::{Point, ScrollAxes};

use crate::host::Frames;

use super::{State, Widget, owning_node};

/// Keeps a scroll view's adjustments in step with the frames the core sent,
/// without waiting for GTK to allocate: `ScrollTo` in the same commit
/// needs the new range.
pub(super) fn sync_scroll(frames: &Frames, scrolled: &gtk::ScrolledWindow, viewport: &gtk::Viewport) {
    let frames = frames.borrow();
    let view = frames.get(scrolled.upcast_ref::<gtk::Widget>()).map(|f| f.size).unwrap_or_default();
    let content = viewport.child().and_then(|c| frames.get(&c).map(|f| f.size)).unwrap_or(view);
    for (adjustment, content, view) in
        [(scrolled.hadjustment(), content.width, view.width), (scrolled.vadjustment(), content.height, view.height)]
    {
        let (content, view) = (content as f64, view as f64);
        let upper = content.max(view);
        let value = adjustment.value().clamp(0.0, upper - view);
        adjustment.configure(value, 0.0, upper, view * 0.1, view * 0.9, view);
    }
}

/// Scrolls like the user would: GTK reports it through the adjustments.
pub(super) fn scroll_to(scrolled: &gtk::ScrolledWindow, offset: Point) {
    scrolled.hadjustment().set_value(offset.x as f64);
    scrolled.vadjustment().set_value(offset.y as f64);
}

pub(super) fn scroll_offset(scrolled: &gtk::ScrolledWindow) -> Point {
    Point::new(scrolled.hadjustment().value() as f32, scrolled.vadjustment().value() as f32)
}

/// Scroll bars (as the theme draws them) on the axes that scroll; without,
/// `External` still scrolls them by wheel, touchpad and touch.
pub(super) fn set_scroll_policy(scrolled: &gtk::ScrolledWindow, axes: ScrollAxes, show: bool) {
    let on = if show { gtk::PolicyType::Automatic } else { gtk::PolicyType::External };
    let policy = |scrolls: bool| if scrolls { on } else { gtk::PolicyType::Never };
    scrolled.set_policy(policy(axes.horizontal()), policy(axes.vertical()));
}

pub(super) fn scroll_axes(scrolled: &gtk::ScrolledWindow) -> ScrollAxes {
    match scrolled.policy() {
        (gtk::PolicyType::Never, _) => ScrollAxes::Vertical,
        (_, gtk::PolicyType::Never) => ScrollAxes::Horizontal,
        _ => ScrollAxes::Both,
    }
}

pub(super) fn scroll_bars(scrolled: &gtk::ScrolledWindow) -> bool {
    let (h, v) = scrolled.policy();
    h != gtk::PolicyType::External && v != gtk::PolicyType::External
}

impl State {
    /// The scroll view a widget is, or is the content of.
    pub(super) fn scroll_of(&self, widget: &gtk::Widget) -> Option<(gtk::ScrolledWindow, gtk::Viewport)> {
        let id = owning_node(&self.by_widget, Some(widget.clone()))?;
        let node = self.nodes.get(&id)?;
        let id = match &node.widget {
            Widget::Scroll { .. } => id,
            _ => node.parent?,
        };
        match &self.nodes.get(&id)?.widget {
            Widget::Scroll { scrolled, viewport } => Some((scrolled.clone(), viewport.clone())),
            _ => None,
        }
    }
}
