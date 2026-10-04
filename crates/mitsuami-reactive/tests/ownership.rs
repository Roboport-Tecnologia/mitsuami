//! Scopes, disposal, cleanups and context.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami_reactive::*;

#[test]
fn disposing_an_owner_stops_its_effects_and_runs_cleanups() {
    let n = signal(0);
    let log = Rc::new(RefCell::new(Vec::new()));
    let scope = Owner::new_root();
    scope.with(|| {
        let log2 = log.clone();
        effect(move || log2.borrow_mut().push(format!("run {}", n.get())));
        let log3 = log.clone();
        on_cleanup(move || log3.borrow_mut().push("cleanup".into()));
    });

    n.set(1);
    scope.dispose();
    n.set(2);

    assert_eq!(*log.borrow(), ["run 0", "run 1", "cleanup"]);
    assert!(!scope.is_alive());
}

#[test]
fn effect_children_are_disposed_before_each_rerun() {
    let outer = signal(0);
    let inner = signal(0);
    let log = Rc::new(RefCell::new(Vec::new()));
    let log2 = log.clone();
    effect(move || {
        let o = outer.get();
        let log3 = log2.clone();
        effect(move || log3.borrow_mut().push(format!("inner of {o}: {}", inner.get())));
        let log4 = log2.clone();
        on_cleanup(move || log4.borrow_mut().push(format!("cleanup {o}")));
    });

    inner.set(1);
    outer.set(1); // old inner effect must die, not run again
    inner.set(2);

    assert_eq!(*log.borrow(), ["inner of 0: 0", "inner of 0: 1", "cleanup 0", "inner of 1: 1", "inner of 1: 2",]);
}

#[test]
fn signals_created_in_a_disposed_scope_are_dead() {
    let scope = Owner::new_root();
    let s = scope.with(|| signal(1));
    assert!(s.is_alive());
    scope.dispose();
    assert!(!s.is_alive());
    let result = std::panic::catch_unwind(|| s.get_untracked());
    assert!(result.is_err());
}

#[test]
fn child_scopes_are_disposed_with_their_parent_but_can_go_first() {
    let root = Owner::new_root();
    let log = Rc::new(RefCell::new(Vec::new()));
    let (a, b) = root.with(|| {
        let a = Owner::new_child();
        let b = Owner::new_child();
        for (scope, name) in [(a, "a"), (b, "b")] {
            let log = log.clone();
            scope.with(|| on_cleanup(move || log.borrow_mut().push(name)));
        }
        (a, b)
    });

    a.dispose();
    assert_eq!(*log.borrow(), ["a"]);
    root.dispose();
    assert_eq!(*log.borrow(), ["a", "b"]);
    assert!(!b.is_alive());
}

#[test]
fn inject_finds_the_nearest_provider() {
    #[derive(Clone, PartialEq, Debug)]
    struct Theme(&'static str);

    let root = Owner::new_root();
    root.with(|| {
        provide(Theme("light"));
        assert_eq!(inject::<Theme>(), Some(Theme("light")));

        Owner::new_child().with(|| {
            assert_eq!(inject::<Theme>(), Some(Theme("light")), "inherited");
            provide(Theme("dark"));
            assert_eq!(inject::<Theme>(), Some(Theme("dark")), "shadowed");
        });

        assert_eq!(inject::<Theme>(), Some(Theme("light")), "sibling unaffected");
        assert_eq!(inject::<u32>(), None);
    });
    root.dispose();
}

#[test]
fn a_disposed_effect_stops_running() {
    let n = signal(0);
    let runs = Rc::new(RefCell::new(0));
    let runs2 = runs.clone();
    let e = effect(move || {
        n.get();
        *runs2.borrow_mut() += 1;
    });
    n.set(1);
    e.dispose();
    n.set(2);
    assert_eq!(*runs.borrow(), 2);
}

#[test]
fn an_effect_disposed_by_an_earlier_effect_in_the_same_flush_does_not_run() {
    // Parent effect re-runs first (creation order), disposing its child,
    // which also depends on the written signal.
    let n = signal(0);
    let log = Rc::new(RefCell::new(Vec::new()));
    let log2 = log.clone();
    effect(move || {
        let seen = n.get();
        let log3 = log2.clone();
        effect(move || log3.borrow_mut().push(format!("child created at {seen} sees {}", n.get())));
    });

    n.set(1);

    assert_eq!(*log.borrow(), ["child created at 0 sees 0", "child created at 1 sees 1"]);
}

#[test]
fn effects_keep_their_other_sources_when_one_is_disposed() {
    // Many effects share signals, and some of the signals go while the
    // effects stay: the others still reach every effect.
    let scopes: Vec<Owner> = (0..4).map(|_| Owner::new_root()).collect();
    let signals: Vec<Signal<u32>> = scopes.iter().map(|scope| scope.with(|| signal(0))).collect();
    let runs = Rc::new(RefCell::new(0));
    for _ in 0..20 {
        let (signals, runs) = (signals.clone(), runs.clone());
        effect(move || {
            for s in signals.iter().filter(|s| s.is_alive()) {
                s.get();
            }
            *runs.borrow_mut() += 1;
        });
    }
    scopes[1].dispose();
    signals[3].set(1);
    scopes[0].dispose();
    signals[2].set(1);
    signals[3].set(2);
    assert_eq!(*runs.borrow(), 20 * 4);
}

#[test]
fn children_disposed_one_by_one_leave_their_scope() {
    let scope = Owner::new_root();
    let children: Vec<Owner> = (0..100).map(|_| scope.child()).collect();
    let log = Rc::new(RefCell::new(Vec::new()));
    for (i, child) in children.iter().enumerate() {
        let log = log.clone();
        child.with(|| on_cleanup(move || log.borrow_mut().push(i)));
    }
    // Every other one, then the scope: each cleanup runs once.
    for child in children.iter().step_by(2) {
        child.dispose();
    }
    scope.dispose();
    let mut ran = log.borrow().clone();
    assert_eq!(ran.len(), 100);
    ran.sort();
    ran.dedup();
    assert_eq!(ran.len(), 100);
    assert!(children.iter().all(|c| !c.is_alive()));
}
