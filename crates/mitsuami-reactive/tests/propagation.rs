//! How writes propagate through computeds to effects.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami_reactive::*;

fn log() -> (Rc<RefCell<Vec<String>>>, impl Fn(String) + Clone) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let push = {
        let log = log.clone();
        move |s: String| log.borrow_mut().push(s)
    };
    (log, push)
}

#[test]
fn effect_runs_immediately_and_on_every_write() {
    let count = signal(0);
    let (log, push) = log();
    effect(move || push(format!("count={}", count.get())));

    count.set(1);
    count.update(|c| *c += 1);

    assert_eq!(*log.borrow(), ["count=0", "count=1", "count=2"]);
}

#[test]
fn computed_is_lazy_and_cached() {
    let runs = Rc::new(RefCell::new(0));
    let a = signal(2);
    let doubled = computed({
        let runs = runs.clone();
        move || {
            *runs.borrow_mut() += 1;
            a.get() * 2
        }
    });
    assert_eq!(*runs.borrow(), 0, "not evaluated until read");

    assert_eq!(doubled.get(), 4);
    assert_eq!(doubled.get(), 4);
    assert_eq!(*runs.borrow(), 1, "cached between reads");

    a.set(5);
    assert_eq!(*runs.borrow(), 1, "still lazy after a write");
    assert_eq!(doubled.get(), 10);
    assert_eq!(*runs.borrow(), 2);
}

#[test]
fn diamond_dependencies_are_glitch_free() {
    // a -> b, a -> c, (b, c) -> effect. The effect must run once per write
    // and never observe b and c out of sync.
    let a = signal(1);
    let b = computed(move || a.get() + 1);
    let c = computed(move || a.get() * 10);
    let (log, push) = log();
    effect(move || push(format!("{}+{}", b.get(), c.get())));

    a.set(2);
    a.set(3);

    assert_eq!(*log.borrow(), ["2+10", "3+20", "4+30"]);
}

#[test]
fn equal_computed_results_stop_propagation() {
    let n = signal(1);
    let parity = computed(move || n.get() % 2 == 0);
    let (log, push) = log();
    effect(move || push(format!("even={}", parity.get())));

    n.set(3); // still odd: effect must not run
    n.set(4);
    n.set(6); // still even

    assert_eq!(*log.borrow(), ["even=false", "even=true"]);
}

#[test]
fn batch_runs_effects_once_after_all_writes() {
    let first = signal("Ada".to_string());
    let last = signal("Lovelace".to_string());
    let (log, push) = log();
    effect(move || push(format!("{} {}", first.get(), last.get())));

    batch(|| {
        first.set("Grace".into());
        last.set("Hopper".into());
    });

    assert_eq!(*log.borrow(), ["Ada Lovelace", "Grace Hopper"]);
}

#[test]
fn nested_batches_flush_at_the_outermost_end() {
    let a = signal(0);
    let (log, push) = log();
    effect(move || push(a.get().to_string()));

    batch(|| {
        a.set(1);
        batch(|| a.set(2));
        assert_eq!(log.borrow().len(), 1, "no flush inside the outer batch");
        a.set(3);
    });

    assert_eq!(*log.borrow(), ["0", "3"]);
}

#[test]
fn dependencies_are_dynamic() {
    let use_a = signal(true);
    let a = signal("a1");
    let b = signal("b1");
    let (log, push) = log();
    effect(move || push(if use_a.get() { a.get() } else { b.get() }.to_string()));

    b.set("b2"); // not a dependency yet
    use_a.set(false);
    a.set("a2"); // no longer a dependency
    b.set("b3");

    assert_eq!(*log.borrow(), ["a1", "b2", "b3"]);
}

#[test]
fn untrack_reads_without_subscribing() {
    let tracked = signal(0);
    let ignored = signal(0);
    let (log, push) = log();
    effect(move || push(format!("{}/{}", tracked.get(), untrack(|| ignored.get()))));

    ignored.set(1);
    tracked.set(1);

    assert_eq!(*log.borrow(), ["0/0", "1/1"]);
}

#[test]
fn effects_may_write_other_signals() {
    let celsius = signal(0.0_f64);
    let fahrenheit = signal(32.0_f64);
    effect(move || fahrenheit.set(celsius.get() * 9.0 / 5.0 + 32.0));

    celsius.set(100.0);

    assert_eq!(fahrenheit.get_untracked(), 212.0);
}

#[test]
fn watch_reports_new_and_old_values_but_not_the_initial_one() {
    let n = signal(1);
    let (log, push) = log();
    watch(move || n.get(), move |new, old| push(format!("{old}->{new}")));

    n.set(2);
    n.set(2); // equal: not reported
    n.set(5);

    assert_eq!(*log.borrow(), ["1->2", "2->5"]);
}

#[test]
fn an_effect_does_not_retrigger_itself() {
    // Same as Vue's default (`allowRecurse: false`): a write to a dependency
    // made by the running effect itself does not schedule another run.
    let n = signal(0);
    let runs = std::rc::Rc::new(std::cell::Cell::new(0));
    let runs2 = runs.clone();
    effect(move || {
        runs2.set(runs2.get() + 1);
        n.set(n.get() + 1);
    });
    n.set(10);
    assert_eq!(runs.get(), 2);
    assert_eq!(n.get_untracked(), 11);
}

#[test]
#[should_panic(expected = "did not settle")]
fn effects_triggering_each_other_forever_are_detected() {
    let a = signal(0);
    let b = signal(0);
    effect(move || b.set(a.get() + 1));
    effect(move || a.set(b.get() + 1));
    a.set(10);
}

#[test]
fn values_accept_literals_signals_and_closures() {
    let name = signal("world".to_string());
    let literal: Value<String> = "fixed".into_value();
    let from_signal: Value<String> = name.into_value();
    let derived: Value<String> = (move || format!("hello {}", name.get())).into_value();

    assert_eq!(literal.get(), "fixed");
    name.set("mitsuami".into());
    assert_eq!(from_signal.get(), "mitsuami");
    assert_eq!(derived.get(), "hello mitsuami");
}

#[test]
fn a_long_chain_of_computeds_updates_without_overflowing() {
    // Each link is read as it's made, so only the update walks the chain.
    let source = signal(0u64);
    let mut last = computed(move || source.get());
    for _ in 0..100_000 {
        let previous = last;
        last = computed(move || previous.get() + 1);
        last.get();
    }
    let seen = Rc::new(RefCell::new(Vec::new()));
    let push = seen.clone();
    effect(move || push.borrow_mut().push(last.get()));

    source.set(1);

    assert_eq!(*seen.borrow(), [100_000, 100_001]);
}

#[test]
fn a_computed_reading_many_signals_reads_each_once() {
    let signals: Vec<_> = (0..1_000).map(signal).collect();
    let runs = Rc::new(RefCell::new(0));
    let sum = computed({
        let signals = signals.clone();
        let runs = runs.clone();
        move || {
            *runs.borrow_mut() += 1;
            // Every signal twice: the second read must not subscribe again.
            signals.iter().chain(&signals).map(|s| s.get()).sum::<i32>()
        }
    });
    assert_eq!(sum.get(), 2 * (0..1_000).sum::<i32>());

    signals[500].set(0);
    signals[999].set(0);

    assert_eq!(sum.get(), 2 * ((0..1_000).sum::<i32>() - 500 - 999));
    assert_eq!(*runs.borrow(), 2);
}
