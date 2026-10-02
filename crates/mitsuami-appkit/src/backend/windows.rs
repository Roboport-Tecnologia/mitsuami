//! Windows: resizing them as the user does, showing them, and their modal loops.

use block2::RcBlock;
use mitsuami_core::{Modality, NodeId, Size};
use objc2::rc::Retained;
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSWindow, NSWindowStyleMask};
use objc2_core_foundation::{CFRunLoop, kCFRunLoopCommonModes};
use objc2_foundation::{NSPoint, NSSize};

use super::{AppKitHandle, Widget};

impl AppKitHandle {
    /// Resizes a window's content like the user would, between its minimum
    /// and maximum, as a drag goes (`setContentSize:` alone would go past
    /// them); the window delegate reports it back as a `WindowResized` event.
    pub fn resize_window(&self, window: NodeId, size: Size) {
        let state = self.state.borrow();
        let Some(Widget::Window { window, _delegate, .. }) = state.nodes.get(&window).map(|n| &n.widget) else {
            return;
        };
        // The user can't resize a window without the resizable style.
        if !window.styleMask().contains(NSWindowStyleMask::Resizable) {
            return;
        }
        let (window, extra) = (window.clone(), _delegate.extra(window));
        drop(state);
        let (min, max) = (window.contentMinSize(), window.contentMaxSize());
        window.setContentSize(NSSize::new(
            (size.width as f64 + extra.width).clamp(min.width, max.width),
            (size.height as f64 + extra.height).clamp(min.height, max.height),
        ));
    }

    /// Orders front windows whose first layout has been applied: a sheet
    /// on the window it belongs to, a window in the app's modal loop, or
    /// an ordinary window.
    pub fn show_pending_windows(&self) {
        let pending = std::mem::take(&mut self.state.borrow_mut().pending_show);
        for id in pending {
            let Some(window) = self.ns_window(id) else { continue };
            let modal = self.state.borrow().nodes.get(&id).and_then(|n| n.modal);
            let owner = modal.and_then(|(owner, _)| owner).and_then(|owner| self.ns_window(owner));
            match (modal.map(|(_, modality)| modality), owner) {
                (Some(Modality::Window), Some(owner)) => owner.beginSheet_completionHandler(&window, None),
                (Some(_), owner) => {
                    match owner {
                        Some(owner) => centre_on(&window, &owner),
                        None => window.center(),
                    }
                    self.run_modal(id, window.clone());
                }
                (None, _) => {
                    window.center();
                    window.makeKeyAndOrderFront(None);
                }
            }
            // Full screen asked for before it was shown.
            if let Some(Widget::Window { _delegate, .. }) = self.state.borrow().nodes.get(&id).map(|n| &n.widget) {
                _delegate.apply_full_screen(&window);
            }
        }
    }

    /// Runs the app's modal loop for the window, once the current run-loop
    /// turn is over: it's a nested loop, which mustn't start inside a
    /// tick. The UI keeps ticking in it (the app's observer runs in the
    /// modal panel mode too). It ends when the window is destroyed. Queued
    /// in the common modes: a dialog opened from an app-modal window is
    /// asked for inside that window's loop, in the modal panel mode, where
    /// a default-mode block waits until the outer loop ends.
    fn run_modal(&self, id: NodeId, window: Retained<NSWindow>) {
        let state = self.state.clone();
        let block = RcBlock::new(move || {
            // Destroyed before it got to run.
            if !state.borrow().nodes.contains_key(&id) {
                return;
            }
            let mtm = state.borrow().mtm;
            NSApplication::sharedApplication(mtm).runModalForWindow(&window);
        });
        if let Some(run_loop) = CFRunLoop::main() {
            unsafe { run_loop.perform_block(kCFRunLoopCommonModes.map(|m| &**m), Some(&block)) };
            run_loop.wake_up();
        }
    }

    /// Escape hatch: the native window of a window node.
    pub fn ns_window(&self, id: NodeId) -> Option<Retained<NSWindow>> {
        match &self.state.borrow().nodes.get(&id)?.widget {
            Widget::Window { window, .. } => Some(window.clone()),
            _ => None,
        }
    }

    /// The window node showing this native window.
    pub(crate) fn window_node(&self, ns_window: &NSWindow) -> Option<NodeId> {
        self.state.borrow().nodes.iter().find_map(|(id, node)| match &node.widget {
            Widget::Window { window, .. } if std::ptr::eq(&**window, ns_window) => Some(*id),
            _ => None,
        })
    }
}

/// Centres a window on another, as dialogs open over their window.
fn centre_on(window: &NSWindow, owner: &NSWindow) {
    let (own, frame) = (owner.frame(), window.frame());
    window.setFrameOrigin(NSPoint::new(
        (own.origin.x + (own.size.width - frame.size.width) / 2.0).round(),
        (own.origin.y + (own.size.height - frame.size.height) / 2.0).round(),
    ));
}

/// An empty event, which wakes the app's event loop.
pub(super) fn post_empty_event(app: &NSApplication) {
    let event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
        NSEventType::ApplicationDefined,
        NSPoint::new(0.0, 0.0),
        NSEventModifierFlags::empty(),
        0.0,
        0,
        None,
        0,
        0,
        0,
    );
    if let Some(event) = event {
        app.postEvent_atStart(&event, false);
    }
}

/// In full screen, or moving into or out of it.
pub(super) fn in_full_screen(window: &NSWindow) -> bool {
    window.styleMask().contains(NSWindowStyleMask::FullScreen)
}
