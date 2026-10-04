//! Parking values for WinRT completion handlers.
//!
//! `IAsyncOperation::when` wants a `Send` closure, but replies and XAML
//! objects aren't `Send`. XAML completes its operations on the UI thread, so
//! the handler carries a ticket and the values wait here in a thread-local.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::thread::ThreadId;

use windows_core::Interface;

use crate::bindings as w;

thread_local! {
    static PARKED: RefCell<HashMap<u64, Box<dyn Any>>> = RefCell::new(HashMap::new());
    static NEXT: Cell<u64> = const { Cell::new(0) };
}

/// Parks `value` until [`take`] is called with the returned ticket.
pub(crate) fn park<T: 'static>(value: T) -> u64 {
    let ticket = NEXT.get();
    NEXT.set(ticket + 1);
    PARKED.with(|p| p.borrow_mut().insert(ticket, Box::new(value)));
    ticket
}

/// Takes back a parked value. `None` if it was taken already.
pub(crate) fn take<T: 'static>(ticket: u64) -> Option<T> {
    let value = PARKED.with(|p| p.borrow_mut().remove(&ticket))?;
    value.downcast().ok().map(|b| *b)
}

/// A parked value's ticket, for a `Send` closure to carry. Dropped unused
/// (a completion that never comes, a queue that won't run the closure), it
/// frees the value on the UI thread, later: a caller that takes the value
/// back right away when a handler couldn't be set still gets it.
pub(crate) struct Ticket {
    id: u64,
    queue: w::DispatcherQueue,
    thread: ThreadId,
    used: bool,
}

/// Parks `value` on this (the UI) thread, for a closure that may run on
/// another to bring back with [`Ticket::take`] or [`on_ui_take`].
pub(crate) fn park_until<T: 'static>(queue: &w::DispatcherQueue, value: T) -> Ticket {
    Ticket { id: park(value), queue: queue.clone(), thread: std::thread::current().id(), used: false }
}

impl Ticket {
    /// What [`take`] takes the value back with, if the closure never got it.
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    /// Takes back the value, on the UI thread.
    pub(crate) fn take<T: 'static>(mut self) -> Option<T> {
        self.used = true;
        take(self.id)
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        if !self.used {
            let id = self.id;
            enqueue(&self.queue, move || free(id));
        }
    }
}

/// Drops whatever is parked under `ticket`, out of the map's borrow: a
/// value's drop may park or take another.
fn free(ticket: u64) {
    let value = PARKED.with(|p| p.borrow_mut().remove(&ticket));
    drop(value);
}

/// Whether a value is still parked under `ticket`.
pub(crate) fn is_parked(ticket: u64) -> bool {
    PARKED.with(|p| p.borrow().contains_key(&ticket))
}

/// Runs `f` on the thread `queue` belongs to (the UI thread), soon. For
/// completions of non-XAML operations (file pickers, the clipboard), which
/// arrive on worker threads.
pub(crate) fn on_ui(queue: &w::DispatcherQueue, f: impl FnOnce() + Send + 'static) {
    enqueue(queue, f);
}

/// Runs `f` with the value parked under `ticket` on the UI thread, soon.
/// The value is freed if the queue won't take `f` (it's shut down): right
/// away when this is the UI thread, else when the ticket drops.
pub(crate) fn on_ui_take<T: 'static>(queue: &w::DispatcherQueue, ticket: Ticket, f: impl FnOnce(T) + Send + 'static) {
    let (id, thread) = (ticket.id, ticket.thread);
    let enqueued = enqueue(queue, move || {
        if let Some(value) = ticket.take::<T>() {
            f(value);
        }
    });
    if !enqueued && std::thread::current().id() == thread {
        free(id);
    }
}

/// Whether the queue took `f`.
fn enqueue(queue: &w::DispatcherQueue, f: impl FnOnce() + Send + 'static) -> bool {
    let f = std::sync::Mutex::new(Some(f));
    let handler = w::DispatcherQueueHandler::new(move || {
        if let Some(f) = f.lock().ok().and_then(|mut f| f.take()) {
            f();
        }
    });
    queue.cast::<w::IDispatcherQueue>().and_then(|q| q.TryEnqueue(&handler)).unwrap_or(false)
}
