//! Control flow: conditional and list rendering.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::Rc;

use mitsuami_reactive::{IntoValue, Owner, Value, computed, effect, on_cleanup, untrack};

use crate::ui::Ui;
use crate::view::{AnyView, Children, View};
use crate::widget::{NodeId, WidgetKind};

type Branch = Rc<dyn Fn() -> Vec<AnyView>>;

fn branch<C: Children>(children: impl Fn() -> C + 'static) -> Branch {
    Rc::new(move || {
        let mut views = Vec::new();
        children().into_views(&mut views);
        views
    })
}

/// Renders `then` while `when` is true, otherwise the optional fallback.
/// Vue's `v-if` / `v-else`. Either is any children: a view, or a tuple of
/// them, which take their places in the parent as if written there.
/// Switching branches disposes the old branch's nodes and reactive state.
pub struct Show {
    when: Value<bool>,
    then: Branch,
    fallback: Option<Branch>,
}

impl Show {
    pub fn new<C: Children>(when: impl IntoValue<bool>, then: impl Fn() -> C + 'static) -> Show {
        Show { when: when.into_value(), then: branch(then), fallback: None }
    }

    pub fn fallback<C: Children>(mut self, fallback: impl Fn() -> C + 'static) -> Show {
        self.fallback = Some(branch(fallback));
        self
    }
}

impl View for Show {
    fn build(self, ui: &Ui) -> NodeId {
        let fragment = ui.create(WidgetKind::Fragment, Vec::new());
        let Show { when, then, fallback } = self;
        let ui = ui.clone();
        let render = move |condition: bool| {
            let branch = if condition { Some(&then) } else { fallback.as_ref() };
            if let Some(branch) = branch {
                for view in branch() {
                    let child = view.build(&ui);
                    ui.append_child(fragment, child);
                    let ui = ui.clone();
                    on_cleanup(move || ui.destroy(child));
                }
            }
        };
        match when {
            Value::Static(condition) => render(condition),
            Value::Dynamic(f) => {
                let condition = computed(move || f());
                effect(move || {
                    let c = condition.get();
                    untrack(|| render(c));
                });
            }
        }
        fragment
    }
}

type KeyFn<T, K> = Rc<dyn Fn(&T) -> K>;
type RenderFn<T> = Rc<dyn Fn(T) -> AnyView>;

/// Keyed list rendering. Vue's `v-for` with `:key`.
///
/// Items are matched by key between updates: existing rows keep their nodes
/// and state (and are moved if needed), new keys render new rows, and rows
/// whose key disappeared are disposed. A row is rendered once per key; put
/// signals inside items for per-row updates.
pub struct For<T: 'static, K: 'static> {
    each: Value<Vec<T>>,
    key: KeyFn<T, K>,
    render: RenderFn<T>,
}

impl<T: Clone + 'static, K: Eq + Hash + 'static> For<T, K> {
    pub fn new<V: View>(
        each: impl IntoValue<Vec<T>>,
        key: impl Fn(&T) -> K + 'static,
        render: impl Fn(T) -> V + 'static,
    ) -> For<T, K> {
        For { each: each.into_value(), key: Rc::new(key), render: Rc::new(move |item| AnyView::new(render(item))) }
    }
}

impl<T: Clone + 'static, K: Eq + Hash + 'static> View for For<T, K> {
    fn build(self, ui: &Ui) -> NodeId {
        let fragment = ui.create(WidgetKind::Fragment, Vec::new());
        // Row scopes hang off a dedicated scope in the *building* owner, so
        // they survive re-runs of the list effect.
        let rows_scope = Owner::current().map(|o| o.child()).unwrap_or_else(Owner::new_root);
        let rows: Rc<RefCell<Vec<(K, NodeId, Owner)>>> = Rc::default();
        let For { each, key, render } = self;
        let ui = ui.clone();
        effect(move || {
            let items = each.get();
            untrack(|| {
                let mut old: HashMap<K, (NodeId, Owner)> =
                    rows.borrow_mut().drain(..).map(|(k, id, owner)| (k, (id, owner))).collect();
                let mut next = Vec::with_capacity(items.len());
                for item in items {
                    let k = key(&item);
                    let (id, owner) = match old.remove(&k) {
                        Some(existing) => existing,
                        None => {
                            let owner = rows_scope.child();
                            let id = owner.with(|| render(item).build(&ui));
                            (id, owner)
                        }
                    };
                    next.push((k, id, owner));
                }
                for (_, (id, owner)) in old {
                    owner.dispose();
                    ui.destroy(id);
                }
                ui.set_children(fragment, next.iter().map(|(_, id, _)| *id).collect());
                *rows.borrow_mut() = next;
            });
        });
        fragment
    }
}

// `view!` tags. `<Show when=…>` and `<For each=… key=… let:item>` need
// their required attributes before their children; each step is a type of
// its own, so a missing one is a compile error.

impl Show {
    #[doc(hidden)]
    pub fn __tag() -> ShowWithoutWhen {
        ShowWithoutWhen { fallback: None }
    }
}

#[doc(hidden)]
pub struct ShowWithoutWhen {
    fallback: Option<Branch>,
}

impl ShowWithoutWhen {
    pub fn when(self, when: impl IntoValue<bool>) -> ShowWhen {
        ShowWhen { when: when.into_value(), fallback: self.fallback }
    }

    pub fn fallback<C: Children>(mut self, fallback: impl Fn() -> C + 'static) -> ShowWithoutWhen {
        self.fallback = Some(branch(fallback));
        self
    }
}

#[doc(hidden)]
pub struct ShowWhen {
    when: Value<bool>,
    fallback: Option<Branch>,
}

impl ShowWhen {
    pub fn fallback<C: Children>(mut self, fallback: impl Fn() -> C + 'static) -> ShowWhen {
        self.fallback = Some(branch(fallback));
        self
    }

    pub fn __children<C: Children>(self, then: impl Fn() -> C + 'static) -> Show {
        Show { when: self.when, then: branch(then), fallback: self.fallback }
    }
}

impl For<(), ()> {
    #[doc(hidden)]
    pub fn __tag() -> ForWithoutEach {
        ForWithoutEach
    }
}

#[doc(hidden)]
pub struct ForWithoutEach;

impl ForWithoutEach {
    pub fn each<T: Clone + 'static>(self, each: impl IntoValue<Vec<T>>) -> ForWithoutKey<T> {
        ForWithoutKey { each: each.into_value() }
    }
}

#[doc(hidden)]
pub struct ForWithoutKey<T: 'static> {
    each: Value<Vec<T>>,
}

impl<T: Clone + 'static> ForWithoutKey<T> {
    pub fn key<K: Eq + Hash + 'static>(self, key: impl Fn(&T) -> K + 'static) -> ForKeyed<T, K> {
        ForKeyed { each: self.each, key: Rc::new(key) }
    }
}

#[doc(hidden)]
pub struct ForKeyed<T: 'static, K: 'static> {
    each: Value<Vec<T>>,
    key: KeyFn<T, K>,
}

impl<T: Clone + 'static, K: Eq + Hash + 'static> ForKeyed<T, K> {
    pub fn __children<V: View>(self, render: impl Fn(T) -> V + 'static) -> For<T, K> {
        For { each: self.each, key: self.key, render: Rc::new(move |item| AnyView::new(render(item))) }
    }
}
