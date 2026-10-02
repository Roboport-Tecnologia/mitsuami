//! Fine-grained, single-threaded reactivity for mitsuami.
//!
//! The API follows Vue's Composition API, adapted to Rust:
//!
//! | Vue | mitsuami |
//! |---|---|
//! | `ref(x)` | [`signal`] (`ref` is a Rust keyword) |
//! | `computed(fn)` | [`computed`] |
//! | `watchEffect(fn)` | [`effect`] |
//! | `watch(src, cb)` | [`watch`] |
//! | `provide` / `inject` | [`provide`] / [`inject`] |
//! | `onUnmounted` / `onScopeDispose` | [`on_cleanup`] |
//!
//! All handles are `Copy` indexes into a thread-local arena. They belong to
//! the [`Owner`] that was current when they were created; reading or
//! writing one after that owner is disposed panics.
//!
//! Effects run synchronously: immediately when created, then whenever a
//! dependency changes. Group several writes with [`batch`] so dependents run
//! once, after all of them.

mod computed;
mod context;
mod effect;
mod owner;
mod runtime;
mod signal;
mod value;

pub use computed::{Computed, computed};
pub use context::{inject, provide};
pub use effect::{Effect, effect, watch};
pub use owner::{Owner, on_cleanup};
pub use signal::{Signal, signal};
pub use value::{IntoValue, Value};

/// Runs `f` with all signal writes batched: effects run once, when the
/// outermost batch ends.
pub fn batch<R>(f: impl FnOnce() -> R) -> R {
    runtime::with_runtime(|rt| rt.batch(f))
}

/// Runs `f` without recording any of its reads as dependencies.
pub fn untrack<R>(f: impl FnOnce() -> R) -> R {
    runtime::with_runtime(|rt| rt.untrack(f))
}
