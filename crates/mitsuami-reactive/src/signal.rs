use std::any::Any;
use std::fmt;
use std::marker::PhantomData;

use crate::runtime::{Kind, NodeKey, with_runtime};

/// A reactive value that can be read and written. Vue's `ref`.
///
/// Reading inside a [`computed`](crate::computed) or [`effect`](crate::effect)
/// subscribes it to the signal. Every write notifies subscribers, even if the
/// new value equals the old one; computeds stop propagation of equal values.
pub struct Signal<T: 'static> {
    pub(crate) key: NodeKey,
    ty: PhantomData<fn() -> T>,
}

impl<T: 'static> Clone for Signal<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for Signal<T> {}

impl<T: 'static> fmt::Debug for Signal<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Signal").field("key", &self.key).finish()
    }
}

impl<T: 'static> PartialEq for Signal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<T: 'static> Eq for Signal<T> {}

/// Creates a [`Signal`] owned by the current [`Owner`](crate::Owner).
pub fn signal<T: 'static>(value: T) -> Signal<T> {
    let key = with_runtime(|rt| rt.create(Kind::Signal, Some(Box::new(value) as Box<dyn Any>)));
    Signal { key, ty: PhantomData }
}

impl<T: 'static> Signal<T> {
    /// Returns a clone of the value and subscribes the running observer.
    ///
    /// # Panics
    /// If the signal's owner was disposed.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }

    /// Borrows the value and subscribes the running observer.
    ///
    /// # Panics
    /// If `f` writes to this same signal, or if the signal's owner was
    /// disposed.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        with_runtime(|rt| rt.track(self.key));
        self.with_untracked(f)
    }

    /// Returns a clone of the value without subscribing.
    pub fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        self.with_untracked(T::clone)
    }

    /// Borrows the value without subscribing.
    pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let slot = with_runtime(|rt| rt.slot(self.key, "signal"));
        let value = slot.borrow();
        f(downcast(&value))
    }

    /// Replaces the value and notifies subscribers.
    ///
    /// # Panics
    /// If the signal's owner was disposed.
    pub fn set(&self, value: T) {
        self.update(|v| *v = value);
    }

    /// Mutates the value in place and notifies subscribers.
    ///
    /// # Panics
    /// If the signal's owner was disposed.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        let slot = with_runtime(|rt| rt.slot(self.key, "signal"));
        {
            let mut value = slot.borrow_mut();
            let value = value
                .as_mut()
                .and_then(|v| v.downcast_mut::<T>())
                .expect("mitsuami-reactive: signal slot holds the wrong type");
            f(value);
        }
        with_runtime(|rt| rt.notify(self.key));
    }

    /// Whether the signal's owner is still alive.
    pub fn is_alive(&self) -> bool {
        with_runtime(|rt| rt.exists(self.key))
    }
}

pub(crate) fn downcast<T: 'static>(value: &Option<Box<dyn Any>>) -> &T {
    value.as_ref().and_then(|v| v.downcast_ref::<T>()).expect("mitsuami-reactive: slot holds the wrong type")
}
