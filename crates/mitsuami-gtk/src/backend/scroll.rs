//! Scroll views: adjustments, offsets and policies.

use std::cell::Cell;
use std::rc::Rc;

use gtk::{glib, prelude::*};
use mitsuami_core::{Insets, NodeId, Point, ScrollAxes, UiEvent};

use crate::host::{Events, Frames};

use super::{State, Widget, owning_node};

/// Keeps a scroll view's adjustments in step with the frames the core sent,
/// without waiting for GTK to allocate: `ScrollTo` in the same commit
/// needs the new range. The page is the viewport, inside the scroll bars.
pub(super) fn sync_scroll(frames: &Frames, scrolled: &gtk::ScrolledWindow, viewport: &gtk::Viewport, insets: Insets) {
    let frames = frames.borrow();
    let view = frames.get(scrolled.upcast_ref::<gtk::Widget>()).map(|f| f.inset(insets).size).unwrap_or_default();
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

/// Reports where the viewport is when classic scroll bars (not overlay
/// ones) come or go, or its frame changes: the core lays the content out
/// inside them. The viewport sets its adjustments' page sizes as it's
/// allocated, so that's when it's checked; a bar shown or hidden at a
/// page size that didn't change is checked once GTK has laid out.
pub(super) fn report_viewport(
    id: NodeId,
    events: &Events,
    scrolled: &gtk::ScrolledWindow,
    viewport: &gtk::Viewport,
    insets: &Rc<Cell<Insets>>,
) {
    let check: Rc<dyn Fn()> = {
        let (events, insets) = (events.clone(), insets.clone());
        let (scrolled, viewport) = (scrolled.downgrade(), viewport.downgrade());
        Rc::new(move || {
            let (Some(scrolled), Some(viewport)) = (scrolled.upgrade(), viewport.upgrade()) else { return };
            if let Some(now) = viewport_insets(&scrolled, &viewport)
                && insets.replace(now) != now
            {
                events.emit(id, UiEvent::ViewportInsets(now));
            }
        })
    };
    for adjustment in [scrolled.hadjustment(), scrolled.vadjustment()] {
        let check = check.clone();
        adjustment.connect_page_size_notify(move |_| check());
    }
    for bar in [scrolled.hscrollbar(), scrolled.vscrollbar()] {
        let check = check.clone();
        bar.connect_visible_notify(move |_| {
            let check = check.clone();
            glib::idle_add_local_once(move || check());
        });
    }
}

/// Where the viewport is in the scrolled window, as last allocated:
/// inside the room classic scroll bars and a frame take. `None` before
/// it's allocated.
pub(super) fn viewport_insets(scrolled: &gtk::ScrolledWindow, viewport: &gtk::Viewport) -> Option<Insets> {
    if scrolled.width() == 0 && scrolled.height() == 0 {
        return None;
    }
    let bounds = viewport.compute_bounds(scrolled)?;
    Some(Insets::new(
        bounds.y(),
        scrolled.width() as f32 - bounds.x() - bounds.width(),
        scrolled.height() as f32 - bounds.y() - bounds.height(),
        bounds.x(),
    ))
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
    pub(super) fn scroll_of(&self, widget: &gtk::Widget) -> Option<(gtk::ScrolledWindow, gtk::Viewport, Insets)> {
        let id = owning_node(&self.by_widget, Some(widget.clone()))?;
        let node = self.nodes.get(&id)?;
        let id = match &node.widget {
            Widget::Scroll { .. } => id,
            _ => node.parent?,
        };
        match &self.nodes.get(&id)?.widget {
            Widget::Scroll { scrolled, viewport, insets } => Some((scrolled.clone(), viewport.clone(), insets.get())),
            _ => None,
        }
    }
}
