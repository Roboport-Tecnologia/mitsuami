//! Async on the UI thread: tasks, timers on the test clock, background work
//! and cancellation with the component that started it.

use std::time::Duration;

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

const SECOND: Duration = Duration::from_secs(1);

#[mitsuami_test::test]
async fn tasks_update_the_ui(app: TestApp) {
    app.mount(|| {
        let status = signal("loading".to_string());
        spawn_local(async move { status.set("ready".into()) });
        Text::new(status)
    });
    app.expect(by_text("ready")).to_exist().await;
}

fn countdown(from: u32) -> impl View {
    let left = signal(from);
    spawn_local(async move {
        while left.get_untracked() > 0 {
            sleep(SECOND).await;
            left.update(|n| *n -= 1);
        }
    });
    Text::new(move || match left.get() {
        0 => "Liftoff".to_string(),
        n => format!("{n}…"),
    })
}

#[mitsuami_test::test]
async fn timers_follow_the_test_clock(app: TestApp) {
    app.mount(|| countdown(3));
    app.expect(by_text("3…")).to_exist().await;

    app.advance(Duration::from_millis(999)).await;
    app.expect(by_text("3…")).to_exist().await;

    app.advance(Duration::from_millis(1)).await;
    app.expect(by_text("2…")).to_exist().await;

    app.advance(SECOND).await;
    app.advance(SECOND).await;
    app.expect(by_text("Liftoff")).to_exist().await;
    assert_eq!(app.ui().pending_tasks(), 0);
}

#[mitsuami_test::test]
async fn background_results_come_back_to_the_ui_thread(app: TestApp) {
    app.mount(|| {
        let answer = signal(None::<u64>);
        spawn_local(async move {
            let ui_thread = std::thread::current().id();
            let (value, worker) = spawn_blocking(|| {
                std::thread::sleep(Duration::from_millis(20));
                ((1..=10u64).product::<u64>(), std::thread::current().id())
            })
            .await;
            assert_ne!(worker, ui_thread, "the work ran on another thread");
            assert_eq!(std::thread::current().id(), ui_thread, "the task resumed on the UI thread");
            answer.set(Some(value));
        });
        Text::new(move || answer.get().map_or("computing".into(), |v| format!("10! = {v}")))
    });
    // Assertions keep settling while tasks are pending.
    app.expect(by_text("10! = 3628800")).to_exist().await;
}

#[mitsuami_test::test]
async fn tasks_stop_with_the_component_that_started_them(app: TestApp) {
    let shown = signal(true);
    let ticks = signal(0);
    app.mount(move || {
        Show::new(shown, move || {
            spawn_local(async move {
                loop {
                    sleep(SECOND).await;
                    ticks.update(|n| *n += 1);
                }
            });
            Text::new("ticking")
        })
    });
    app.advance(SECOND).await;
    assert_eq!(ticks.get_untracked(), 1);

    shown.set(false);
    app.settle().await;
    app.advance(SECOND).await;
    app.advance(SECOND).await;

    assert_eq!(ticks.get_untracked(), 1, "the task was cancelled");
    assert_eq!(app.ui().pending_tasks(), 0);
}

#[mitsuami_test::test]
async fn task_handles_cancel(app: TestApp) {
    let done = signal(false);
    // Spawned on the Ui directly: no component scope, `sleep` still works.
    let handle = app.ui().spawn_local(async move {
        sleep(SECOND).await;
        done.set(true);
    });
    app.settle().await;
    handle.cancel();
    app.advance(SECOND).await;
    assert!(!done.get_untracked());
    assert_eq!(app.ui().pending_tasks(), 0);
}

#[mitsuami_test::test]
async fn tasks_run_in_their_component_scope(app: TestApp) {
    #[derive(Clone)]
    struct Greeting(&'static str);
    app.provide(Greeting("hello from context"));
    app.mount(|| {
        let text = signal(String::new());
        spawn_local(async move {
            sleep(SECOND).await;
            // Still inside the component: context is visible.
            text.set(inject::<Greeting>().map(|g| g.0.to_string()).unwrap_or_default());
        });
        Text::new(text).test_id("text")
    });
    app.advance(SECOND).await;
    app.expect(by_text("hello from context")).to_exist().await;
}

#[mitsuami_test::test]
async fn timers_fire_in_deadline_order(app: TestApp) {
    let log = signal(Vec::<&'static str>::new());
    app.mount(move || {
        for (name, ms) in [("slow", 300), ("fast", 100), ("medium", 200)] {
            spawn_local(async move {
                sleep(Duration::from_millis(ms)).await;
                log.update(|l| l.push(name));
            });
        }
        Text::new("timers")
    });
    for _ in 0..3 {
        app.advance(Duration::from_millis(100)).await;
    }
    assert_eq!(log.get_untracked(), ["fast", "medium", "slow"]);
}

#[mitsuami_test::test]
async fn a_task_that_keeps_yielding_lets_the_ui_run(app: TestApp) {
    // Wakes itself and returns once, as `yield_now` does.
    struct Yield(bool);
    impl std::future::Future for Yield {
        type Output = ();
        fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context) -> std::task::Poll<()> {
            if std::mem::replace(&mut self.0, true) {
                return std::task::Poll::Ready(());
            }
            cx.waker().wake_by_ref();
            std::task::Poll::Pending
        }
    }
    let clicks = signal(0);
    let handle = app.ui().spawn_local(async move {
        loop {
            Yield(false).await;
        }
    });
    app.mount(move || {
        Button::new(move || format!("Clicked {}", clicks.get())).on_click(move || clicks.update(|n| *n += 1))
    });
    app.get(by_role(Role::Button, "Clicked 0")).click().await;
    app.expect(by_text("Clicked 1")).to_exist().await;
    handle.cancel();
}

mitsuami_test::main!();
