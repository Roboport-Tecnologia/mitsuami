//! Following the pointer while a list's own rows are dragged. XAML raises
//! `DragOver` as the pointer moves, but not for where it stops: a drag
//! that came to rest in a folder's middle had last been heard of in its
//! bottom quarter, so the mark showed a line where the drop went onto the
//! folder, or a refusal from a row it had crossed (the maintainer,
//! 2026-10-27; seen in a log with injected moves). Each `DragOver` says
//! where the pointer is on screen and in the list, and once a frame the
//! cursor's screen position is taken back to the list, for the mark and
//! the edge scrolling to follow it, moved or not: the edge scrolling moves
//! the rows under a pointer at rest, and a mark left as it was showed a
//! line through a row.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use windows_core::{EventRevoker, Interface};

use crate::bindings as w;

type R<T> = windows_core::Result<T>;

/// Once a frame.
const TICK_MICROS: i64 = 16_000;

/// What follows the pointer: given where it is in the list.
pub(crate) type Follow = Rc<dyn Fn(w::Point)>;

#[derive(Default)]
struct State {
    /// The pointer on screen (in pixels) and in the list (in epx) at the
    /// last `DragOver`, and the window's scale between them.
    anchor: Cell<Option<(w::POINT, w::Point, f64)>>,
    follow: RefCell<Option<Follow>>,
    timer: RefCell<Option<(w::IDispatcherQueueTimer, EventRevoker)>>,
}

/// A list's pointer tracking during its own drags, shared by its handlers.
#[derive(Clone, Default)]
pub(crate) struct Tracker(Rc<State>);

impl Tracker {
    /// A drag of its own rows started: `follow` hears where the pointer
    /// is, once it's been over the list.
    pub(crate) fn start(&self, follow: Follow) {
        *self.0.follow.borrow_mut() = Some(follow);
        self.0.anchor.set(None);
        _ = self.run();
    }

    /// A `DragOver` at `at` in `list`: where the screen's pixels are in it.
    pub(crate) fn anchor(&self, list: &w::UIElement, at: w::Point) {
        let mut screen = w::POINT::default();
        if !unsafe { w::GetCursorPos(&mut screen) }.as_bool() {
            return;
        }
        let scale = list.XamlRoot().and_then(|r| r.RasterizationScale()).unwrap_or(1.0);
        self.0.anchor.set(Some((screen, at, scale)));
    }

    /// The drag dropped or ended.
    pub(crate) fn stop(&self) {
        if let Some((timer, _)) = self.0.timer.borrow().as_ref() {
            _ = timer.Stop();
        }
        *self.0.follow.borrow_mut() = None;
        self.0.anchor.set(None);
    }

    fn run(&self) -> R<()> {
        if self.0.timer.borrow().is_none() {
            let queue = w::DispatcherQueue::GetForCurrentThread()?;
            let timer = queue.cast::<w::IDispatcherQueue>()?.CreateTimer()?.cast::<w::IDispatcherQueueTimer>()?;
            timer.SetIsRepeating(true)?;
            timer.SetInterval(windows_time::TimeSpan::from_micros(TICK_MICROS))?;
            let state = Rc::downgrade(&self.0);
            let tick = timer.Tick(move |_, _| {
                if let Some(state) = state.upgrade() {
                    tick(&state);
                }
            })?;
            *self.0.timer.borrow_mut() = Some((timer, tick));
        }
        let timer = self.0.timer.borrow();
        let (timer, _) = timer.as_ref().expect("made above");
        timer.Start()
    }
}

/// The pointer, taken to the list.
fn tick(state: &State) {
    let Some((screen, at, scale)) = state.anchor.get() else { return };
    let mut now = w::POINT::default();
    if !unsafe { w::GetCursorPos(&mut now) }.as_bool() {
        return;
    }
    let Some(follow) = state.follow.borrow().clone() else { return };
    let scale = if scale > 0.0 { scale } else { 1.0 };
    follow(w::Point {
        x: at.x + ((now.x - screen.x) as f64 / scale) as f32,
        y: at.y + ((now.y - screen.y) as f64 / scale) as f32,
    });
}
