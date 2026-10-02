use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::runtime::{ComputeFn, Kind, NodeKey, Slot, with_runtime};
use crate::signal::downcast;

/// A lazily evaluated, cached value derived from other reactive values.
///
/// It recomputes only when read after one of its dependencies changed, and
/// only notifies its own subscribers when the result is different (`!=`).
pub struct Computed<T: 'static> {
    key: NodeKey,
    ty: PhantomData<fn() -> T>,
}

impl<T: 'static> Clone for Computed<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for Computed<T> {}

impl<T: 'static> fmt::Debug for Computed<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Computed").field("key", &self.key).finish()
    }
}

/// Creates a [`Computed`] owned by the current [`Owner`](crate::Owner).
pub fn computed<T: PartialEq + 'static>(f: impl Fn() -> T + 'static) -> Computed<T> {
    let compute: ComputeFn = Rc::new(move |slot: &Slot| {
        let new = f();
        let mut slot = slot.borrow_mut();
        match slot.as_mut().and_then(|v| v.downcast_mut::<T>()) {
            Some(old) if *old == new => false,
            Some(old) => {
                *old = new;
                true
            }
            None => {
                *slot = Some(Box::new(new) as Box<dyn Any>);
                true
            }
        }
    });
    let key = with_runtime(|rt| rt.create(Kind::Computed(compute), None));
    Computed { key, ty: PhantomData }
}

impl<T: 'static> Computed<T> {
    /// Returns a clone of the value and subscribes the running observer.
    ///
    /// # Panics
    /// If the computed value's owner was disposed.
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        self.with(T::clone)
    }

    /// Borrows the value and subscribes the running observer.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let slot = with_runtime(|rt| {
            rt.update_if_necessary(self.key);
            rt.track(self.key);
            rt.slot(self.key, "computed")
        });
        let value = slot.borrow();
        f(downcast(&value))
    }

    /// Returns a clone of the value without subscribing.
    pub fn get_untracked(&self) -> T
    where
        T: Clone,
    {
        crate::untrack(|| self.get())
    }
}
