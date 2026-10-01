//! Hover (`Prop::Hover`): XAML's `PointerEntered` and `PointerExited` on
//! the node's element. Both bubble, so its children's come up too: the
//! pointer entering any of them is over the element, and an exit counts
//! only once the pointer is outside the element's bounds, so moving
//! between its children doesn't leave it.

use std::cell::Cell;
use std::rc::Rc;

use mitsuami_core::{NodeId, UiEvent};
use windows_core::{EventRevoker, Interface};

use crate::backend::Events;
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

pub(crate) struct HoverTracker {
    state: Rc<HoverState>,
    _revokers: Vec<EventRevoker>,
}

struct HoverState {
    id: NodeId,
    events: Events,
    over: Cell<bool>,
}

impl HoverState {
    fn set(&self, over: bool) {
        if self.over.replace(over) != over {
            self.events.emit(self.id, UiEvent::Hover(over));
        }
    }
}

impl HoverTracker {
    pub(crate) fn new(id: NodeId, events: Events, element: &w::UIElement) -> R<HoverTracker> {
        let state = Rc::new(HoverState { id, events, over: Cell::new(false) });
        let ui: w::IUIElement = element.cast()?;
        let weak = Rc::downgrade(&state);
        let entered = ui.PointerEntered(move |_, _| {
            if let Some(state) = weak.upgrade() {
                state.set(true);
            }
        })?;
        let weak = Rc::downgrade(&state);
        let exited = ui.PointerExited(move |sender, args| {
            let Some(state) = weak.upgrade() else { return };
            if !inside(&sender, &args).unwrap_or(false) {
                state.set(false);
            }
        })?;
        Ok(HoverTracker { state, _revokers: vec![entered, exited] })
    }

    /// What its handlers do, for `synthesize`: XAML can't be sent pointer
    /// moves.
    pub(crate) fn input(&self) -> HoverInput {
        HoverInput(self.state.clone())
    }

    /// Ends a hover it reported, as it stops tracking.
    pub(crate) fn remove(self) {
        self.state.set(false);
    }
}

pub(crate) struct HoverInput(Rc<HoverState>);

impl HoverInput {
    pub(crate) fn set(&self, over: bool) {
        self.0.set(over);
    }
}

/// Whether the pointer is still within the element that heard the exit.
fn inside(
    sender: &windows_core::Ref<windows_core::IInspectable>,
    args: &windows_core::Ref<w::PointerRoutedEventArgs>,
) -> Option<bool> {
    let element: w::UIElement = sender.as_ref()?.cast().ok()?;
    let frame: w::IFrameworkElement = element.cast().ok()?;
    let args: w::IPointerRoutedEventArgs = args.as_ref()?.cast().ok()?;
    let at = args.GetCurrentPoint(&element).ok()?.cast::<w::IPointerPoint>().ok()?.Position().ok()?;
    let (x, y) = (at.x as f64, at.y as f64);
    Some(x >= 0.0 && y >= 0.0 && x < frame.ActualWidth().ok()? && y < frame.ActualHeight().ok()?)
}
