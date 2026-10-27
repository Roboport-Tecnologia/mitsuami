//! Scrolling a list or tree while its own rows are dragged near its top or
//! bottom edge. XAML's lists scroll themselves during a drag, within 100
//! epx of either edge (`ListViewBase::OnDragOver`), which in SourceGit's
//! short branch trees was anywhere, so rows only ever landed at the top or
//! the bottom (the maintainer, 2026-10-27). Its handlers run too late to
//! stop it, and it only scrolls along an axis the scroll viewer lets the
//! user scroll: the list's vertical scroll mode is off for the drag
//! (`hold`), and it scrolls here instead, within a row's height of the
//! edge, at most a fifth of the view, faster the closer the pointer is, at
//! XAML's slowest speed at the zone's inner side.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use windows_core::{EventRevoker, Interface};

use crate::bindings as w;

type R<T> = windows_core::Result<T>;

/// How far in from the edge the zone reaches: about a Fluent row.
const ZONE: f64 = 32.0;
/// Speeds at the zone's inner side and at the edge, in epx a second:
/// XAML's slowest, and a top speed that a short view can still follow.
const SLOWEST: f64 = 150.0;
const FASTEST: f64 = 1000.0;
/// How often it moves while the pointer is in the zone: once a frame.
const TICK_MICROS: i64 = 16_000;

#[derive(Default)]
struct State {
    /// Epx a second, down positive; zero while the pointer is outside the
    /// zone.
    velocity: Cell<f64>,
    scroll: RefCell<Option<w::IScrollViewer>>,
    timer: RefCell<Option<(w::IDispatcherQueueTimer, EventRevoker)>>,
    /// Whether the timer runs: starting it again restarts its interval,
    /// and the pointer is followed more often than that (`track`).
    running: Cell<bool>,
    /// The scroll viewer held for a drag, and its mode before.
    held: RefCell<Option<(w::IScrollViewer, w::ScrollMode)>>,
}

/// A list's edge scrolling, shared by its handlers.
#[derive(Clone, Default)]
pub(crate) struct EdgeScroll(Rc<State>);

impl EdgeScroll {
    /// A drag of its own rows starts: XAML's edge scrolling is off until
    /// it ends (`release`). Scrolling from code still works.
    pub(crate) fn hold(&self, scroll: &w::IScrollViewer) {
        if self.0.held.borrow().is_some() {
            return;
        }
        let Ok(mode) = scroll.VerticalScrollMode() else { return };
        if scroll.SetVerticalScrollMode(w::ScrollMode::Disabled).is_ok() {
            *self.0.held.borrow_mut() = Some((scroll.clone(), mode));
        }
    }

    /// The drag dropped or ended: stops, and the user scrolls again.
    pub(crate) fn release(&self) {
        self.stop();
        if let Some((scroll, mode)) = self.0.held.borrow_mut().take() {
            _ = scroll.SetVerticalScrollMode(mode);
        }
    }

    /// The pointer at `at` in `list`, whose scroll viewer `scroll` is:
    /// scrolls while it's in a zone, stops when it leaves.
    pub(crate) fn drag_at(&self, scroll: &w::IScrollViewer, list: &w::UIElement, at: w::Point) {
        let Ok(element) = scroll.cast::<w::UIElement>() else { return };
        let Some(origin) =
            element.TransformToVisual(list).and_then(|t| t.TransformPoint(w::Point { x: 0.0, y: 0.0 })).ok()
        else {
            return;
        };
        let height = scroll.ViewportHeight().unwrap_or(0.0);
        let zone = ZONE.min(height / 5.0);
        let y = f64::from(at.y - origin.y);
        let speed = |distance: f64| SLOWEST + (FASTEST - SLOWEST) * (1.0 - (distance / zone).clamp(0.0, 1.0));
        let velocity = if zone <= 0.0 || !(0.0..=height).contains(&y) {
            0.0
        } else if y < zone {
            -speed(y)
        } else if y > height - zone {
            speed(height - y)
        } else {
            0.0
        };
        self.0.velocity.set(velocity);
        *self.0.scroll.borrow_mut() = Some(scroll.clone());
        if velocity == 0.0 {
            self.stop();
        } else {
            _ = self.start();
        }
    }

    /// The drag left, dropped or ended.
    pub(crate) fn stop(&self) {
        self.0.velocity.set(0.0);
        if let Some((timer, _)) = self.0.timer.borrow().as_ref() {
            _ = timer.Stop();
        }
        self.0.running.set(false);
    }

    fn start(&self) -> R<()> {
        if self.0.running.get() {
            return Ok(());
        }
        if self.0.timer.borrow().is_none() {
            let queue = w::DispatcherQueue::GetForCurrentThread()?;
            let timer = queue.cast::<w::IDispatcherQueue>()?.CreateTimer()?.cast::<w::IDispatcherQueueTimer>()?;
            timer.SetIsRepeating(true)?;
            timer.SetInterval(windows_time::TimeSpan::from_micros(TICK_MICROS))?;
            let state = Rc::downgrade(&self.0);
            let tick = timer.Tick(move |_, _| {
                if let Some(state) = state.upgrade() {
                    step(&state);
                }
            })?;
            *self.0.timer.borrow_mut() = Some((timer, tick));
        }
        let timer = self.0.timer.borrow();
        let (timer, _) = timer.as_ref().expect("made above");
        timer.Start()?;
        self.0.running.set(true);
        Ok(())
    }
}

/// One tick's move, as far as the content goes.
fn step(state: &State) {
    let velocity = state.velocity.get();
    let Some(scroll) = state.scroll.borrow().clone() else { return };
    if velocity == 0.0 {
        return;
    }
    let offset = scroll.VerticalOffset().unwrap_or(0.0);
    let end = scroll.ScrollableHeight().unwrap_or(0.0);
    let to = (offset + velocity * TICK_MICROS as f64 / 1_000_000.0).clamp(0.0, end.max(0.0));
    if to != offset {
        _ = scroll.ChangeViewWithOptionalAnimation(None, Some(to), None, true);
    }
}
