//! Single instance (`App::single_instance`): a later launch reaches the
//! running app's `on_reopen` handlers, through the `single_instance`
//! example. The backend hands the launch over with `Ui::reopen`, as
//! `TestApp::reopen` does here; bringing the window forward is the
//! backend's, before the handlers run, so it isn't checked headless.

#[allow(dead_code)]
#[path = "../examples/single_instance.rs"]
mod example;

use std::path::PathBuf;

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn launch(args: &[&str]) -> Reopen {
    Reopen::new(args.iter().map(|arg| arg.to_string()).collect(), Some(PathBuf::from("/home/me")), Vec::new())
}

#[mitsuami_test::test]
async fn a_handler_gets_each_launch(app: TestApp) {
    let launches = signal(Vec::<String>::new());
    app.mount(move || {
        on_reopen(move |reopen| launches.update(|list| list.push(example::describe(reopen))));
        example::page(launches)
    });
    app.expect(by_test_id("launches")).to_have_text("Not launched again yet.").await;
    app.reopen(launch(&["hello", "world"])).await;
    app.reopen(Reopen::new(Vec::new(), None, vec![PathBuf::from("/tmp/notes.txt")])).await;
    app.expect(by_test_id("launches"))
        .to_have_text("hello world, in /home/me\nNo arguments, opening /tmp/notes.txt")
        .await;
}

#[mitsuami_test::test]
async fn handlers_go_with_their_scope(app: TestApp) {
    let heard = signal(0);
    app.mount(move || {
        on_reopen(move |_| heard.update(|n| *n += 1));
        Text::new("")
    });
    app.reopen(launch(&[])).await;
    assert_eq!(heard.get(), 1);
    app.unmount();
    app.reopen(launch(&[])).await;
    assert_eq!(heard.get(), 1, "its scope ended");
}

#[mitsuami_test::test]
async fn without_a_handler_nothing_runs(app: TestApp) {
    app.mount(|| Text::new("Still here"));
    app.reopen(launch(&["anything"])).await;
    app.expect(by_text("Still here")).to_be_visible().await;
}

#[mitsuami_test::test]
async fn a_windowless_app_opens_its_window_from_the_handler(app: TestApp) {
    app.ui().set_runs_without_windows(true);
    let owner = Owner::new_root();
    owner.with(|| {
        // `on_reopen` finds the Ui in its scope, as in the app scope.
        provide(app.ui().clone());
        _ = example::window(true).build(app.ui());
    });
    app.settle().await;
    assert!(app.ui().windows().is_empty(), "started hidden");

    app.reopen(launch(&["again"])).await;
    assert_eq!(app.ui().windows().len(), 1);
    app.expect(by_test_id("launches")).to_have_text("again, in /home/me").await;
}

#[mitsuami_test::test]
async fn a_launch_can_quit_it(app: TestApp) {
    app.ui().set_runs_without_windows(true);
    let owner = Owner::new_root();
    owner.with(|| {
        // `on_reopen` finds the Ui in its scope, as in the app scope.
        provide(app.ui().clone());
        _ = example::window(true).build(app.ui());
    });
    app.settle().await;
    assert!(!app.ui().should_end());
    app.reopen(launch(&["stop"])).await;
    assert!(app.ui().should_end());
    assert!(app.ui().windows().is_empty(), "stop doesn't open the window");
}

mitsuami_test::main!();
