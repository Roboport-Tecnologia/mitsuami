//! `Spinner`: the platform's spinner for work of unknown length. It spins
//! while running, shows nothing while stopped but keeps its place, reads as
//! a progress bar without a value named by its label, and takes no focus.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn props(app: &TestApp) -> Vec<Prop> {
    app.get_by_role(Role::ProgressBar, "Loading").native_state().props
}

#[mitsuami_test::test]
async fn spins_from_the_start(app: TestApp) {
    app.mount(|| Spinner::new("Loading"));

    let spinner = app.get_by_role(Role::ProgressBar, "Loading");
    assert_eq!(spinner.native_state().kind, WidgetKind::Spinner);
    assert!(props(&app).contains(&Prop::Running(true)));
    assert_eq!(spinner.node().value, None);
}

/// Stopped, it keeps its place, so nothing around it moves.
#[mitsuami_test::test]
async fn stops_and_starts_in_place(app: TestApp) {
    let loading = signal(true);
    app.mount(move || {
        Row::new().gap(8).align(Align::Center).children((Spinner::new("Loading").running(loading), Text::new("Photos")))
    });
    let spinner = app.get_by_role(Role::ProgressBar, "Loading").frame();
    let text = app.get_by_text("Photos").frame();
    assert!(!spinner.size.is_empty(), "{spinner}");

    loading.set(false);
    app.settle().await;
    assert!(props(&app).contains(&Prop::Running(false)));
    assert_eq!(app.get_by_role(Role::ProgressBar, "Loading").frame(), spinner);
    assert_eq!(app.get_by_text("Photos").frame(), text);

    loading.set(true);
    app.settle().await;
    assert!(props(&app).contains(&Prop::Running(true)));
}

/// Tab goes past it, as past the platform's own spinners.
#[mitsuami_test::test]
async fn takes_no_focus(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            TextInput::new().a11y_label("First"),
            Spinner::new("Loading"),
            TextInput::new().a11y_label("Second"),
        ))
    });

    app.get_by_label("First").focus().await;
    app.get_by_label("First").press(Key::Tab).await;
    app.expect(by_label("Second")).to_be_focused().await;
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let loading = signal(false);
    app.mount(move || view! { <Spinner a11y_label="Loading" running=loading/> });

    assert!(props(&app).contains(&Prop::Running(false)));
}

/// Logs whether the native spinner spins, each time the tweak runs.
fn log_running(log: Rc<RefCell<Vec<bool>>>) -> Tweak<Spinner> {
    platform! {
        // AppKit can't say whether it's animating: log that it ran.
        macos => mitsuami::appkit::tweak(move |_: &mitsuami::appkit::objc2_app_kit::NSProgressIndicator| {
            log.borrow_mut().push(true)
        }),
        gtk => mitsuami::gtk::tweak(move |s: &mitsuami::gtk::gtk::Spinner| log.borrow_mut().push(s.is_spinning())),
        kde => mitsuami::kirigami::tweak(move |s: &mitsuami::kirigami::QmlObject| log.borrow_mut().push(s.bool("running"))),
        windows => mitsuami::winui::tweak(move |s: &mitsuami::winui::bindings::ProgressRing| {
            use mitsuami::winui::windows_core::Interface;
            log.borrow_mut().push(s.cast::<mitsuami::winui::bindings::IProgressRing>()?.IsActive()?);
            Ok(())
        }),
    }
}

#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_spinner_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let loading = signal(true);
    let tweak = log_running(log.clone());
    app.mount(move || Spinner::new("Loading").native(tweak).running(loading));

    assert!(props(&app).iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&true));
    let runs = log.borrow().len();
    loading.set(false);
    app.settle().await;
    assert!(log.borrow().len() > runs);
}

mitsuami_test::main!();
