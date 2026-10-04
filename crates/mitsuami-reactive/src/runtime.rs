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
use std::collections::HashSet;
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
    /// Each source, and where this node is in its `observers`, so a re-run
    /// unlinks from a signal thousands of nodes read without a search.
    sources: Vec<(NodeKey, usize)>,
    /// Each observer, and where this node is in its `sources`.
    observers: Vec<(NodeKey, usize)>,
    /// While a run has read many sources, the set of them, so each new read
    /// checks for a repeat in constant time. Dropped when the run ends.
    tracked: Option<HashSet<NodeKey>>,
    owner: Option<NodeKey>,
    owned: Vec<NodeKey>,
    /// Nodes in `owned` disposed on their own and not yet taken out: a
    /// list's rows go one by one, and taking each out was quadratic.
    disposed_owned: usize,
    cleanups: Vec<Box<dyn FnOnce()>>,
    contexts: Vec<(TypeId, Rc<dyn Any>)>,
    /// Creation order. Pending effects run in this order, so a parent effect
    /// runs (and possibly disposes its children) before its children do.
    seq: u64,
}

const MAX_FLUSH_PASSES: usize = 10_000;
/// Up to this many sources, a read looks for a repeat by scanning them.
const SCAN_SOURCES: usize = 16;

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

/// `None` once the thread's runtime is gone: in thread-local destructors,
/// where a scope dropped with its holder has nothing left to dispose.
pub(crate) fn try_with_runtime<R>(f: impl FnOnce(&Runtime) -> R) -> Option<R> {
    RUNTIME.try_with(f).ok()
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
            tracked: None,
            owner,
            owned: Vec::new(),
            disposed_owned: 0,
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
        let Some(at_source) = nodes.get(source).map(|s| s.observers.len()) else { return };
        let Some(obs) = nodes.get_mut(observer) else { return };
        if obs.tracked.is_none() && obs.sources.len() < SCAN_SOURCES {
            if obs.sources.iter().any(|(s, _)| *s == source) {
                return;
            }
        } else {
            let sources = &obs.sources;
            let tracked = obs.tracked.get_or_insert_with(|| sources.iter().map(|(s, _)| *s).collect());
            if !tracked.insert(source) {
                return;
            }
        }
        let at_observer = obs.sources.len();
        obs.sources.push((source, at_source));
        nodes[source].observers.push((observer, at_observer));
    }

    /// Brings a computed or effect up to date, re-running it only if one of
    /// its sources actually changed.
    ///
    /// A `Check` node brings its computed sources up to date first, in the
    /// order it read them, and stops at the first that changed. That walk
    /// keeps its own stack, so a long chain of computeds can't overflow the
    /// thread's.
    pub(crate) fn update_if_necessary(&self, key: NodeKey) {
        struct Frame {
            key: NodeKey,
            /// The sources still to check, taken when the walk reaches it.
            sources: Option<std::vec::IntoIter<NodeKey>>,
        }
        let mut stack = vec![Frame { key, sources: None }];
        while let Some(frame) = stack.last_mut() {
            let node = frame.key;
            let next = match self.state(node) {
                State::Check => {
                    let sources = frame.sources.get_or_insert_with(|| {
                        let nodes = self.nodes.borrow();
                        nodes
                            .get(node)
                            .map(|n| n.sources.iter().map(|(s, _)| *s).collect::<Vec<_>>())
                            .unwrap_or_default()
                            .into_iter()
                    });
                    sources.find(|&source| {
                        matches!(self.kind(source), Some(Kind::Computed(_))) && self.state(source) != State::Clean
                    })
                }
                State::Clean | State::Dirty => None,
            };
            match next {
                Some(source) => stack.push(Frame { key: source, sources: None }),
                None => {
                    if self.state(node) == State::Dirty {
                        self.run(node);
                    }
                    self.set_state(node, State::Clean);
                    stack.pop();
                }
            }
        }
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
                        let observers = self.observers(key);
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
        if let Some(node) = self.nodes.borrow_mut().get_mut(key) {
            node.state = State::Clean;
            node.tracked = None;
        }
    }

    // --------------------------------------------------------------- writes

    /// Marks everything downstream of a written signal and flushes effects
    /// unless a batch is open.
    pub(crate) fn notify(&self, signal: NodeKey) {
        for observer in self.observers(signal) {
            self.stale(observer, State::Dirty);
        }
        self.flush_if_idle();
    }

    /// Marks `key` stale and everything downstream `Check`, with a stack of
    /// its own so a long chain can't overflow the thread's.
    fn stale(&self, key: NodeKey, state: State) {
        let mut stack = vec![(key, state)];
        while let Some((key, state)) = stack.pop() {
            let mut nodes = self.nodes.borrow_mut();
            let Some(node) = nodes.get_mut(key) else { continue };
            if node.state >= state {
                continue;
            }
            if node.state == State::Clean && matches!(node.kind, Kind::Effect(_)) {
                self.pending.borrow_mut().push(key);
            }
            node.state = state;
            stack.extend(node.observers.iter().map(|&(o, _)| (o, State::Check)));
        }
    }

    fn observers(&self, key: NodeKey) -> Vec<NodeKey> {
        self.nodes.borrow().get(key).map(|n| n.observers.iter().map(|(o, _)| *o).collect()).unwrap_or_default()
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
            node.disposed_owned = 0;
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
        node.tracked = None;
        for (source, at) in sources {
            let Some(source) = nodes.get_mut(source) else { continue };
            debug_assert_eq!(source.observers[at].0, key);
            // Observers are in no order: the last takes this one's place.
            source.observers.swap_remove(at);
            if let Some(&(moved, moved_at)) = source.observers.get(at)
                && let Some(entry) = nodes.get_mut(moved).and_then(|m| m.sources.get_mut(moved_at))
            {
                entry.1 = at;
            }
        }
    }

    pub(crate) fn dispose(&self, key: NodeKey) {
        let owner = match self.nodes.borrow().get(key) {
            Some(node) => node.owner,
            None => return,
        };
        if let Some(owner) = owner {
            self.forget_owned(owner, key);
        }
        self.dispose_detached(key);
    }

    /// Takes `key` out of its owner's list: right away when it's the last,
    /// else once more than half the list is disposed, so a scope whose
    /// children come and go keeps a list no longer than twice theirs.
    fn forget_owned(&self, owner: NodeKey, key: NodeKey) {
        let mut nodes = self.nodes.borrow_mut();
        let Some(node) = nodes.get_mut(owner) else { return };
        if node.owned.last() == Some(&key) {
            node.owned.pop();
            return;
        }
        node.disposed_owned += 1;
        if node.disposed_owned * 2 <= node.owned.len() {
            return;
        }
        let mut owned = std::mem::take(&mut node.owned);
        owned.retain(|k| *k != key && nodes.contains_key(*k));
        let node = &mut nodes[owner];
        node.owned = owned;
        node.disposed_owned = 0;
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
            for (observer, at) in node.observers {
                let Some(obs) = nodes.get_mut(observer) else { continue };
                debug_assert_eq!(obs.sources[at].0, key);
                // Sources keep their order, which reads check them in: the
                // ones after move up, and their observers are told.
                obs.sources.remove(at);
                for i in at..obs.sources.len() {
                    let (source, source_at) = nodes[observer].sources[i];
                    if let Some(entry) = nodes.get_mut(source).and_then(|s| s.observers.get_mut(source_at)) {
                        entry.1 = i;
                    }
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
