//! Stores: app-wide state and the actions on it, Pinia-style.
//!
//! A store is a plain struct of signals (and resources, actions) with
//! methods. [`use_store`] returns the app's one instance, created on first
//! use, and views share it:
//!
//! ```ignore
//! #[derive(Clone, Copy)]
//! struct Cart { items: Signal<Vec<Item>> }
//!
//! impl Store for Cart {
//!     fn create() -> Cart { Cart { items: signal(Vec::new()) } }
//! }
//!
//! let cart = use_store::<Cart>();
//! ```
//!
//! A store `provide`d in a scope takes precedence there, which is how tests
//! (and previews) swap in a store in a known state.
//!
//! An action that runs async work starts it with [`Store::spawn`], so it
//! runs in the store's scope. [`spawn_local`] would tie it to the view that
//! called the action, and closing that view (a dialog, say) would cancel it.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::rc::Rc;

use mitsuami_reactive::{Owner, inject, provide};

use crate::task::{TaskHandle, spawn_local};

/// App-wide state, created once per app by [`use_store`].
pub trait Store: Clone + 'static {
    /// Creates the store. Runs once, in a scope that lives as long as the
    /// app, so the store's effects, resources and tasks do too.
    fn create() -> Self;

    /// Runs `future` on the UI thread in the store's scope, for its
    /// actions' async work: it goes on when the view that called the
    /// action goes away, and stops with the store. The store is the one
    /// [`use_store`] finds from the caller's scope, `provide`d or the app's.
    ///
    /// ```ignore
    /// fn rename(&self, path: PathBuf, name: String) {
    ///     let store = *self;
    ///     self.spawn(async move {
    ///         let result = spawn_blocking(move || fs::rename(&path, &name)).await;
    ///         store.renamed(result);
    ///     });
    /// }
    /// ```
    ///
    /// # Panics
    /// Outside of an app, or for a store neither `provide`d above this
    /// scope nor made by [`use_store`].
    fn spawn(&self, future: impl Future<Output = ()> + 'static) -> TaskHandle {
        store_scope::<Self>().with(|| spawn_local(future))
    }
}

#[derive(Clone)]
struct Registry {
    stores: Rc<RefCell<HashMap<TypeId, Rc<dyn Any>>>>,
    /// Each store's scope, a child of the app scope. Known before the store
    /// is made, so `create` can spawn.
    scopes: Rc<RefCell<HashMap<TypeId, Owner>>>,
    /// The app scope.
    owner: Owner,
}

/// Makes [`use_store`] work in the current scope and below: the stores
/// live as long as it. `App` and `TestApp` call it in their app scope.
///
/// # Panics
/// When called outside of any scope.
pub fn provide_stores() {
    let owner = Owner::current().expect("mitsuami: provide_stores needs a scope (Owner::with)");
    provide(Registry { stores: Rc::default(), scopes: Rc::default(), owner });
}

/// The app's instance of store `S`, created on first use; or the one
/// `provide`d nearest to this scope, if any.
///
/// # Panics
/// Outside of an app (no [`provide_stores`] above this scope).
pub fn use_store<S: Store>() -> S {
    if let Some(store) = inject::<S>() {
        return store;
    }
    let registry = inject::<Registry>().unwrap_or_else(|| {
        panic!(
            "mitsuami: use_store::<{}>() outside of an app; mount the view with App or TestApp, \
             or call provide_stores() in an enclosing scope",
            std::any::type_name::<S>()
        )
    });
    let existing = registry.stores.borrow().get(&TypeId::of::<S>()).cloned();
    if let Some(store) = existing {
        return store.downcast_ref::<S>().expect("mitsuami: store registry holds the wrong type").clone();
    }
    // Not borrowed while creating: a store may use other stores.
    let scope = registry.owner.child();
    registry.scopes.borrow_mut().insert(TypeId::of::<S>(), scope);
    let store = scope.with(S::create);
    registry.stores.borrow_mut().insert(TypeId::of::<S>(), Rc::new(store.clone()));
    store
}

/// The scope of the store `use_store::<S>()` would return here: the one it
/// was `provide`d in, or the one the app made it in.
fn store_scope<S: Store>() -> Owner {
    if let Some(scope) = Owner::providing::<S>() {
        return scope;
    }
    let scope = inject::<Registry>().and_then(|registry| registry.scopes.borrow().get(&TypeId::of::<S>()).copied());
    scope.unwrap_or_else(|| {
        panic!(
            "mitsuami: {}::spawn found no store scope; spawn from an app's view or task, with a store \
             from use_store or one provided above it",
            std::any::type_name::<S>()
        )
    })
}
