//! An app that runs without windows (`App::runs_without_windows`): it
//! doesn't end when no window is open, only when it quits. The backends
//! end their run loops when `Ui::should_end` says so, after each turn.

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

#[mitsuami_test::test]
async fn an_app_ends_when_no_window_is_open(app: TestApp) {
    // The test window comes on first use: none yet.
    assert!(app.ui().windows().is_empty());
    assert!(app.ui().should_end());
}

#[mitsuami_test::test]
async fn one_that_runs_without_windows_waits_until_it_quits(app: TestApp) {
    app.ui().set_runs_without_windows(true);
    assert!(app.ui().windows().is_empty());
    assert!(!app.ui().should_end(), "it keeps running with no window");
    app.ui().quit();
    assert!(app.ui().should_end());
}

#[mitsuami_test::test]
async fn quitting_ends_it_with_windows_open(app: TestApp) {
    app.mount(|| view! { <Text>"Still open"</Text> });
    assert!(!app.ui().should_end());
    app.ui().quit();
    assert!(app.ui().should_end(), "quit doesn't wait for windows to close");
}

#[mitsuami_test::test]
async fn a_window_can_open_and_close_while_it_waits(app: TestApp) {
    app.ui().set_runs_without_windows(true);
    let open = signal(false);
    let owner = Owner::new_root();
    owner.with(|| {
        Window::new("Later").open(open).content(|| view! { <Text>"Shown on demand"</Text> }).build(app.ui());
    });
    app.settle().await;
    assert!(app.ui().windows().is_empty(), "not open yet");
    assert!(!app.ui().should_end());

    open.set(true);
    app.settle().await;
    assert_eq!(app.ui().windows().len(), 1);
    app.expect(by_text("Shown on demand")).to_be_visible().await;

    open.set(false);
    app.settle().await;
    assert!(app.ui().windows().is_empty());
    assert!(!app.ui().should_end(), "closing its last window doesn't end it");
}

mitsuami_test::main!();
