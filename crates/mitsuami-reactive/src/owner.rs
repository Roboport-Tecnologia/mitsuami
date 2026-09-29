use crate::runtime::{NodeKey, with_runtime};

/// A node in the ownership tree: a scope that disposes everything created
/// inside it. Vue's `effectScope`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    key: NodeKey,
}

impl Owner {
    /// Creates a scope with no parent. It lives until [`Owner::dispose`].
    pub fn new_root() -> Owner {
        Owner { key: with_runtime(|rt| rt.create_owner(None)) }
    }

    /// Creates a scope owned by the current owner.
    pub fn new_child() -> Owner {
        Owner { key: with_runtime(|rt| rt.create_owner(rt.current_owner())) }
    }

    /// Creates a scope owned by `self`.
    pub fn child(&self) -> Owner {
        Owner { key: with_runtime(|rt| rt.create_owner(Some(self.key))) }
    }

    /// The owner that reactive nodes created right now would belong to.
    pub fn current() -> Option<Owner> {
        with_runtime(|rt| rt.current_owner()).map(|key| Owner { key })
    }

    /// The nearest scope, the current one or above, that `provide`s a `T`:
    /// the one [`inject`](crate::inject) would find it in.
    pub fn providing<T: 'static>() -> Option<Owner> {
        with_runtime(|rt| rt.provider(std::any::TypeId::of::<T>())).map(|key| Owner { key })
    }

    /// Runs `f` with this scope as the current owner.
    pub fn with<R>(&self, f: impl FnOnce() -> R) -> R {
        with_runtime(|rt| rt.with_owner(Some(self.key), f))
    }

    /// Disposes the scope, everything it owns, and runs its cleanups.
    pub fn dispose(self) {
        with_runtime(|rt| rt.dispose(self.key));
    }

    pub fn is_alive(&self) -> bool {
        with_runtime(|rt| rt.exists(self.key))
    }
}

/// Registers `f` to run when the current owner is disposed or re-run.
/// Does nothing when there is no current owner.
pub fn on_cleanup(f: impl FnOnce() + 'static) {
    with_runtime(|rt| rt.on_cleanup(Box::new(f)));
}
