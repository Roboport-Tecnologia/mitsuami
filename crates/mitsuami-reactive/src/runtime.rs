//! The reactive graph.
//!
//! Push-pull propagation with three node states (the "Reactively" algorithm):
//! writing a signal marks its direct observers `Dirty` and everything further
//! downstream `Check`. Reading a node pulls: a `Check` node first brings its
//! sources up to date and only re-runs if one of them actually changed. This
//! keeps updates glitch-free (no effect observes a half-updated graph) and
//! skips work when a computed recomputes to an equal value.
//!
//! The same arena also stores the ownership tree. Every node is owned by the
//! node that was running when it was created. Re-running or disposing a node
//! first disposes everything it owns and runs its cleanups.

use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use slotmap::{SlotMap, new_key_type};

new_key_type! {
    pub(crate) struct NodeKey;
}

pub(crate) type Slot = Rc<RefCell<Option<Box<dyn Any>>>>;
/// Recomputes a computed into its slot. Returns whether the value changed.
pub(crate) type ComputeFn = Rc<dyn Fn(&Slot) -> bool>;
pub(crate) type EffectFn = Rc<RefCell<dyn FnMut()>>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum State {
    Clean,
    Check,
    Dirty,
}

#[derive(Clone)]
pub(crate) enum Kind {
    Signal,
    Computed(ComputeFn),
    Effect(EffectFn),
    Owner,
}

struct Node {
    kind: Kind,
    slot: Slot,
    state: State,
    sources: Vec<NodeKey>,
    observers: Vec<NodeKey>,
    owner: Option<NodeKey>,
    owned: Vec<NodeKey>,
    cleanups: Vec<Box<dyn FnOnce()>>,
    contexts: Vec<(TypeId, Rc<dyn Any>)>,
    /// Creation order. Pending effects run in this order, so a parent effect
    /// runs (and possibly disposes its children) before its children do.
    seq: u64,
}

const MAX_FLUSH_PASSES: usize = 10_000;

#[derive(Default)]
pub(crate) struct Runtime {
    nodes: RefCell<SlotMap<NodeKey, Node>>,
    observer: Cell<Option<NodeKey>>,
    owner: Cell<Option<NodeKey>>,
    batch_depth: Cell<u32>,
    flushing: Cell<bool>,
    pending: RefCell<Vec<NodeKey>>,
    seq: Cell<u64>,
}

thread_local! {
    static RUNTIME: Runtime = Runtime::default();
}

pub(crate) fn with_runtime<R>(f: impl FnOnce(&Runtime) -> R) -> R {
    RUNTIME.with(f)
}

/// Restores a `Cell` to its previous value on drop, so panics inside user
/// code don't leave the runtime pointing at the wrong observer or owner.
struct Restore<'a, T: Copy>(&'a Cell<T>, T);

impl<T: Copy> Drop for Restore<'_, T> {
    fn drop(&mut self) {
        self.0.set(self.1);
    }
}

fn replace<T: Copy>(cell: &Cell<T>, value: T) -> Restore<'_, T> {
    Restore(cell, cell.replace(value))
}

impl Runtime {
    pub(crate) fn create(&self, kind: Kind, initial: Option<Box<dyn Any>>) -> NodeKey {
        let state = match kind {
            Kind::Computed(_) | Kind::Effect(_) => State::Dirty,
            Kind::Signal | Kind::Owner => State::Clean,
        };
        let seq = self.seq.get();
        self.seq.set(seq + 1);
        let owner = self.owner.get();
        let mut nodes = self.nodes.borrow_mut();
        let key = nodes.insert(Node {
            kind,
            slot: Rc::new(RefCell::new(initial)),
            state,
            sources: Vec::new(),
            observers: Vec::new(),
            owner,
            owned: Vec::new(),
            cleanups: Vec::new(),
            contexts: Vec::new(),
            seq,
        });
        if let Some(owner) = owner.and_then(|o| nodes.get_mut(o)) {
            owner.owned.push(key);
        }
        key
    }

    /// Creates an owner-only node with an explicit parent (or none).
    pub(crate) fn create_owner(&self, parent: Option<NodeKey>) -> NodeKey {
        let _owner = replace(&self.owner, parent);
        self.create(Kind::Owner, None)
    }

    pub(crate) fn exists(&self, key: NodeKey) -> bool {
        self.nodes.borrow().contains_key(key)
    }

    pub(crate) fn current_owner(&self) -> Option<NodeKey> {
        self.owner.get()
    }

    fn state(&self, key: NodeKey) -> State {
        self.nodes.borrow().get(key).map_or(State::Clean, |n| n.state)
    }

    fn set_state(&self, key: NodeKey, state: State) {
        if let Some(node) = self.nodes.borrow_mut().get_mut(key) {
            node.state = state;
        }
    }

    fn kind(&self, key: NodeKey) -> Option<Kind> {
        self.nodes.borrow().get(key).map(|n| n.kind.clone())
    }

    /// Returns the value slot of a live node, panicking with a useful message
    /// if it has been disposed.
    pub(crate) fn slot(&self, key: NodeKey, what: &str) -> Slot {
        match self.nodes.borrow().get(key) {
            Some(node) => node.slot.clone(),
            None => panic!("mitsuami-reactive: {what} used after its owner was disposed"),
        }
    }

    // ---------------------------------------------------------------- reads

    /// Records `source` as a dependency of whatever is currently running.
    pub(crate) fn track(&self, source: NodeKey) {
        let Some(observer) = self.observer.get() else { return };
        let mut nodes = self.nodes.borrow_mut();
        if !nodes.contains_key(source) {
            return;
        }
        let Some(obs) = nodes.get_mut(observer) else { return };
        if obs.sources.contains(&source) {
            return;
        }
        obs.sources.push(source);
        nodes[source].observers.push(observer);
    }

    /// Brings a computed or effect up to date, re-running it only if one of
    /// its sources actually changed.
    pub(crate) fn update_if_necessary(&self, key: NodeKey) {
        if self.state(key) == State::Check {
            let sources = match self.nodes.borrow().get(key) {
                Some(node) => node.sources.clone(),
                None => return,
            };
            for source in sources {
                if matches!(self.kind(source), Some(Kind::Computed(_))) {
                    self.update_if_necessary(source);
                }
                if self.state(key) == State::Dirty {
                    break;
                }
            }
        }
        if self.state(key) == State::Dirty {
            self.run(key);
        }
        self.set_state(key, State::Clean);
    }

    fn run(&self, key: NodeKey) {
        self.clean(key);
        self.unlink_sources(key);
        let Some(kind) = self.kind(key) else { return };
        let slot = self.slot(key, "node");
        {
            let _observer = replace(&self.observer, Some(key));
            let _owner = replace(&self.owner, Some(key));
            match kind {
                Kind::Computed(compute) => {
                    if compute(&slot) {
                        let observers = self.nodes.borrow()[key].observers.clone();
                        for observer in observers {
                            self.set_state(observer, State::Dirty);
                        }
                    }
                }
                Kind::Effect(effect) => {
                    let mut effect = effect.try_borrow_mut().expect("mitsuami-reactive: an effect re-entered itself");
                    effect();
                }
                Kind::Signal | Kind::Owner => {}
            }
        }
        self.set_state(key, State::Clean);
    }

    // --------------------------------------------------------------- writes

    /// Marks everything downstream of a written signal and flushes effects
    /// unless a batch is open.
    pub(crate) fn notify(&self, signal: NodeKey) {
        let observers = match self.nodes.borrow().get(signal) {
            Some(node) => node.observers.clone(),
            None => return,
        };
        for observer in observers {
            self.stale(observer, State::Dirty);
        }
        self.flush_if_idle();
    }

    fn stale(&self, key: NodeKey, state: State) {
        let (was_clean, is_effect, observers) = {
            let mut nodes = self.nodes.borrow_mut();
            let Some(node) = nodes.get_mut(key) else { return };
            if node.state >= state {
                return;
            }
            let was_clean = node.state == State::Clean;
            node.state = state;
            (was_clean, matches!(node.kind, Kind::Effect(_)), node.observers.clone())
        };
        if was_clean && is_effect {
            self.pending.borrow_mut().push(key);
        }
        for observer in observers {
            self.stale(observer, State::Check);
        }
    }

    pub(crate) fn batch<R>(&self, f: impl FnOnce() -> R) -> R {
        let depth = self.batch_depth.get();
        let result = {
            let _depth = replace(&self.batch_depth, depth + 1);
            f()
        };
        self.flush_if_idle();
        result
    }

    pub(crate) fn untrack<R>(&self, f: impl FnOnce() -> R) -> R {
        let _observer = replace(&self.observer, None);
        f()
    }

    fn flush_if_idle(&self) {
        if self.batch_depth.get() == 0 && !self.flushing.get() {
            self.flush();
        }
    }

    fn flush(&self) {
        let _flushing = replace(&self.flushing, true);
        for _ in 0..MAX_FLUSH_PASSES {
            let mut pending = std::mem::take(&mut *self.pending.borrow_mut());
            if pending.is_empty() {
                return;
            }
            {
                let nodes = self.nodes.borrow();
                pending.retain(|k| nodes.contains_key(*k));
                pending.sort_by_key(|k| nodes[*k].seq);
            }
            for effect in pending {
                if self.exists(effect) {
                    self.update_if_necessary(effect);
                }
            }
        }
        self.pending.borrow_mut().clear();
        panic!(
            "mitsuami-reactive: effects did not settle after {MAX_FLUSH_PASSES} passes; \
             an effect is probably writing a signal it depends on"
        );
    }

    // ------------------------------------------------------------ ownership

    pub(crate) fn with_owner<R>(&self, owner: Option<NodeKey>, f: impl FnOnce() -> R) -> R {
        let _owner = replace(&self.owner, owner);
        f()
    }

    pub(crate) fn on_cleanup(&self, f: Box<dyn FnOnce()>) {
        let Some(owner) = self.owner.get() else { return };
        if let Some(node) = self.nodes.borrow_mut().get_mut(owner) {
            node.cleanups.push(f);
        }
    }

    /// Disposes everything `key` owns and runs its cleanups, keeping `key`.
    fn clean(&self, key: NodeKey) {
        let (owned, cleanups) = {
            let mut nodes = self.nodes.borrow_mut();
            let Some(node) = nodes.get_mut(key) else { return };
            node.contexts.clear();
            (std::mem::take(&mut node.owned), std::mem::take(&mut node.cleanups))
        };
        for child in owned.into_iter().rev() {
            self.dispose_detached(child);
        }
        for cleanup in cleanups.into_iter().rev() {
            self.untrack(cleanup);
        }
    }

    fn unlink_sources(&self, key: NodeKey) {
        let mut nodes = self.nodes.borrow_mut();
        let Some(node) = nodes.get_mut(key) else { return };
        let sources = std::mem::take(&mut node.sources);
        for source in sources {
            if let Some(source) = nodes.get_mut(source) {
                source.observers.retain(|o| *o != key);
            }
        }
    }

    pub(crate) fn dispose(&self, key: NodeKey) {
        let owner = match self.nodes.borrow().get(key) {
            Some(node) => node.owner,
            None => return,
        };
        if let Some(owner) = owner
            && let Some(node) = self.nodes.borrow_mut().get_mut(owner)
        {
            node.owned.retain(|k| *k != key);
        }
        self.dispose_detached(key);
    }

    /// Disposes a node that has already been removed from its owner's list.
    fn dispose_detached(&self, key: NodeKey) {
        if !self.exists(key) {
            return;
        }
        self.clean(key);
        self.unlink_sources(key);
        let removed = self.nodes.borrow_mut().remove(key);
        if let Some(node) = removed {
            let mut nodes = self.nodes.borrow_mut();
            for observer in node.observers {
                if let Some(observer) = nodes.get_mut(observer) {
                    observer.sources.retain(|s| *s != key);
                }
            }
        }
    }

    // -------------------------------------------------------------- context

    pub(crate) fn provide(&self, value: Rc<dyn Any>, type_id: TypeId) {
        let Some(owner) = self.owner.get() else {
            panic!("mitsuami-reactive: provide() called outside of any owner");
        };
        let mut nodes = self.nodes.borrow_mut();
        let contexts = &mut nodes[owner].contexts;
        contexts.retain(|(t, _)| *t != type_id);
        contexts.push((type_id, value));
    }

    pub(crate) fn inject(&self, type_id: TypeId) -> Option<Rc<dyn Any>> {
        let key = self.provider(type_id)?;
        let nodes = self.nodes.borrow();
        nodes[key].contexts.iter().find(|(t, _)| *t == type_id).map(|(_, value)| value.clone())
    }

    /// The nearest owner, the current one or above, that provides `type_id`.
    pub(crate) fn provider(&self, type_id: TypeId) -> Option<NodeKey> {
        let nodes = self.nodes.borrow();
        let mut current = self.owner.get();
        while let Some(key) = current {
            let node = nodes.get(key)?;
            if node.contexts.iter().any(|(t, _)| *t == type_id) {
                return Some(key);
            }
            current = node.owner;
        }
        None
    }
}
