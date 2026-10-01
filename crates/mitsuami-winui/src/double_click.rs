//! Double clicks (`Prop::DoubleClick`): XAML's `DoubleTapped` on the
//! node's element, which bubbles up from its children. One that started
//! on a control inside (a button, a field) is the control's.

use std::rc::Rc;

use mitsuami_core::{NodeId, UiEvent};
use windows_core::{EventRevoker, IInspectable, IUnknown, Interface};

use crate::backend::Events;
use crate::bindings as w;

type R<T> = windows_core::Result<T>;

pub(crate) struct DoubleClicker {
    report: Rc<dyn Fn()>,
    _revoker: EventRevoker,
}

impl DoubleClicker {
    pub(crate) fn new(id: NodeId, events: Events, element: &w::UIElement) -> R<DoubleClicker> {
        let report: Rc<dyn Fn()> = Rc::new(move || events.emit(id, UiEvent::DoubleClick));
        let fire = report.clone();
        let revoker = element.cast::<w::IUIElement>()?.DoubleTapped(move |sender, args| {
            let source = args.as_ref().and_then(|a| a.cast::<w::IRoutedEventArgs>().ok()?.OriginalSource().ok());
            if !on_control(&sender, source).unwrap_or(false) {
                fire();
            }
        })?;
        Ok(DoubleClicker { report, _revoker: revoker })
    }

    /// What its handler does, for `synthesize`: XAML can't be sent taps.
    pub(crate) fn report(&self) -> Rc<dyn Fn()> {
        self.report.clone()
    }
}

/// Whether `source` is in a control between it and `sender`, the element
/// that heard the double tap.
fn on_control(sender: &windows_core::Ref<IInspectable>, source: Option<IInspectable>) -> Option<bool> {
    let host = sender.as_ref()?.cast::<IUnknown>().ok()?;
    let mut current: w::DependencyObject = source?.cast().ok()?;
    loop {
        if current.cast::<IUnknown>().ok()? == host {
            return Some(false);
        }
        if current.cast::<w::IControl>().is_ok() {
            return Some(true);
        }
        current = w::VisualTreeHelper::GetParent(&current).ok()?;
    }
}
