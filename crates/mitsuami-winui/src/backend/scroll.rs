//! Scroll views: their axes, offsets and the wheel.

use std::cell::Cell;
use std::time::{Duration, Instant};

use mitsuami_core::{Insets, NodeId, Point, ScrollAxes, UiEvent};
use windows_core::{EventRevoker, Interface};

use super::{Events, R, ok};
use crate::bindings as w;

/// Where a scroll viewer's viewport is in it. XAML's template overlays its
/// scroll bars on the content (its presenter spans their row and column),
/// so this is zero unless a template or style changes that; the viewport
/// is at the top left, scroll viewers being left to right.
pub(super) fn viewport_insets(scroll: &w::IScrollViewer) -> Insets {
    let Ok(element) = scroll.cast::<w::IFrameworkElement>() else { return Insets::ZERO };
    let (width, height) = (scroll.ViewportWidth().unwrap_or(0.0), scroll.ViewportHeight().unwrap_or(0.0));
    if width <= 0.0 || height <= 0.0 {
        return Insets::ZERO;
    }
    let right = (element.ActualWidth().unwrap_or(0.0) - width).max(0.0);
    let bottom = (element.ActualHeight().unwrap_or(0.0) - height).max(0.0);
    Insets::new(0.0, right as f32, bottom as f32, 0.0)
}

/// Reports where the viewport is once per change: as the scroll viewer is
/// resized, and as its view changes.
pub(super) fn report_viewport(emitter: &Events, id: NodeId, last: &Cell<Insets>, scroll: &w::IScrollViewer) {
    let insets = viewport_insets(scroll);
    if last.replace(insets) != insets {
        emitter.emit(id, UiEvent::ViewportInsets(insets));
    }
}

/// Reports a scroll offset once per change, for the same reason.
pub(super) fn report_offset(emitter: &Events, id: NodeId, last: &Cell<Point>, scroll: &w::IScrollViewer) {
    let offset =
        Point::new(scroll.HorizontalOffset().unwrap_or(0.0) as f32, scroll.VerticalOffset().unwrap_or(0.0) as f32);
    if last.replace(offset) != offset {
        emitter.emit(id, UiEvent::Scrolled(offset));
    }
}

/// Scrolls without animation and applies it now, so the offset (and the
/// `Scrolled` report) doesn't wait for XAML's next layout pass.
pub(super) fn scroll_now(
    emitter: &Events,
    id: NodeId,
    last: &Cell<Point>,
    scroll: &w::IScrollViewer,
    to: Point,
) -> R<()> {
    // The content's size may be new too: lay out first so the scrollable
    // extent is current, or XAML clamps the offset to the old one.
    let element: w::IUIElement = scroll.cast()?;
    element.UpdateLayout()?;
    scroll.ChangeViewWithOptionalAnimation(Some(to.x as f64), Some(to.y as f64), None, true)?;
    element.UpdateLayout()?;
    report_offset(emitter, id, last, scroll);
    Ok(())
}

/// Scrolling on the given axes, with scroll bars (`Auto`: XAML's, which
/// collapse to thin indicators) or without (`Hidden`: it still scrolls, by
/// wheel, touchpad and touch). `Disabled` is for axes that don't scroll.
pub(super) fn set_scrolling(scroll: &w::IScrollViewer, axes: ScrollAxes, show: bool) -> R<()> {
    let visible = if show { w::ScrollBarVisibility::Auto } else { w::ScrollBarVisibility::Hidden };
    let hidden = w::ScrollBarVisibility::Disabled;
    let (on, off) = (w::ScrollMode::Enabled, w::ScrollMode::Disabled);
    scroll.SetHorizontalScrollBarVisibility(if axes.horizontal() { visible } else { hidden })?;
    scroll.SetVerticalScrollBarVisibility(if axes.vertical() { visible } else { hidden })?;
    scroll.SetHorizontalScrollMode(if axes.horizontal() { on } else { off })?;
    scroll.SetVerticalScrollMode(if axes.vertical() { on } else { off })
}

pub(super) fn scroll_axes(scroll: &w::IScrollViewer) -> R<ScrollAxes> {
    let shows = |v: w::ScrollBarVisibility| v != w::ScrollBarVisibility::Disabled;
    Ok(match (shows(scroll.HorizontalScrollBarVisibility()?), shows(scroll.VerticalScrollBarVisibility()?)) {
        (true, true) => ScrollAxes::Both,
        (true, false) => ScrollAxes::Horizontal,
        _ => ScrollAxes::Vertical,
    })
}

/// How far a wheel notch scrolls: XAML's 3 lines of 16 px.
const WHEEL_NOTCH: f64 = 48.0;

/// Shift+wheel scrolls sideways in Windows' own apps (Explorer, Edge) and
/// on the other platforms, but not in XAML's `ScrollViewer`
/// (microsoft-ui-xaml#8553, closed as not planned). The content sees the
/// wheel before the scroll viewer does: scroll it there, as far as a notch
/// scrolls down, if it can scroll sideways.
pub(super) fn shift_wheel(scroll: &w::ScrollViewer, content: &w::UIElement) -> R<EventRevoker> {
    let scroll: w::IScrollViewer = scroll.cast()?;
    // Where the last notch sent the view, and when: XAML applies a scroll
    // at its next layout, so notches that come before it add up.
    let last: Cell<Option<(f64, Instant)>> = Cell::new(None);
    content.cast::<w::IUIElement>()?.PointerWheelChanged(move |_, args| {
        let Some(args) = args.as_ref() else { return };
        let width = scroll.ScrollableWidth().unwrap_or(0.0);
        if unsafe { w::GetKeyState(w::VK_SHIFT) } >= 0 || width <= 0.0 {
            return;
        }
        let Ok(props) = args.GetCurrentPoint(None).and_then(|p| p.cast::<w::IPointerPoint>()?.Properties()) else {
            return;
        };
        let props: w::IPointerPointProperties = ok(props.cast(), "cast to PointerPointProperties");
        if props.IsHorizontalMouseWheel().unwrap_or(true) {
            return;
        }
        let from = match last.get() {
            Some((x, at)) if at.elapsed() < Duration::from_millis(250) => x,
            _ => scroll.HorizontalOffset().unwrap_or(0.0),
        };
        let delta = props.MouseWheelDelta().unwrap_or(0) as f64 / 120.0;
        let x = (from - delta * WHEEL_NOTCH).clamp(0.0, width);
        last.set(Some((x, Instant::now())));
        _ = scroll.ChangeViewWithOptionalAnimation(Some(x), None, None, true);
        _ = args.SetHandled(true);
    })
}

pub(super) fn scroll_bars(scroll: &w::IScrollViewer) -> R<bool> {
    let hidden = w::ScrollBarVisibility::Hidden;
    Ok(scroll.HorizontalScrollBarVisibility()? != hidden && scroll.VerticalScrollBarVisibility()? != hidden)
}
