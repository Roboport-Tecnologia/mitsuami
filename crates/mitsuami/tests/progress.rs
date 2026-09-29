//! `Progress`: a progress bar, as the platform draws one. It shows how far
//! along a task is, or animates for work of unknown length, and reads as a
//! progress bar named by its label whose value is the percentage.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{A11yAction, ActionError, Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn has(app: &TestApp, query: Query, prop: Prop) -> bool {
    app.get(query).native_state().props.contains(&prop)
}

#[mitsuami_test::test]
async fn shows_how_far_along_it_is(app: TestApp) {
    app.mount(|| Progress::new("Upload").value(0.25));
    let upload = by_role(Role::ProgressBar, "Upload");

    assert_eq!(app.get(upload.clone()).native_state().kind, WidgetKind::Progress);
    assert!(has(&app, upload.clone(), Prop::Progress(Some(0.25))));
    app.expect(upload).to_have_value("25%").await;
}

#[mitsuami_test::test]
async fn is_indeterminate_without_a_value(app: TestApp) {
    app.mount(|| Progress::new("Loading"));
    let loading = by_role(Role::ProgressBar, "Loading");

    assert!(has(&app, loading.clone(), Prop::Progress(None)));
    assert_eq!(app.get(loading).value(), None);
}

#[mitsuami_test::test]
async fn follows_reactive_values(app: TestApp) {
    let done = signal(0.0);
    let unknown = signal(true);
    app.mount(move || Progress::new("Upload").value(done).indeterminate(unknown));
    let upload = by_role(Role::ProgressBar, "Upload");
    assert!(has(&app, upload.clone(), Prop::Progress(None)));

    unknown.set(false);
    done.set(0.5);
    app.expect(upload.clone()).to_have_value("50%").await;
    assert!(has(&app, upload.clone(), Prop::Progress(Some(0.5))));

    // Past the end is done.
    done.set(1.5);
    app.expect(upload.clone()).to_have_value("100%").await;
    assert!(has(&app, upload.clone(), Prop::Progress(Some(1.0))));

    unknown.set(true);
    app.settle().await;
    assert!(has(&app, upload, Prop::Progress(None)));
}

#[mitsuami_test::test]
async fn has_a_size_and_takes_no_actions(app: TestApp) {
    app.mount(|| Column::new().children((Progress::new("Upload").value(0.5), Progress::new("Loading"))));

    for name in ["Upload", "Loading"] {
        let bar = app.get_by_role(Role::ProgressBar, name);
        assert!(!bar.frame().size.is_empty(), "{name} has no size");
        assert_eq!(app.ui().perform(bar.id(), &A11yAction::Activate), Err(ActionError::Unsupported));
    }
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let done = signal(0.25);
    app.mount(move || {
        view! {
            <Column>
                <Progress label="Upload" value=done/>
                <Progress label="Loading" indeterminate=true/>
            </Column>
        }
    });

    app.expect(by_role(Role::ProgressBar, "Upload")).to_have_value("25%").await;
    done.set(0.5);
    app.expect(by_role(Role::ProgressBar, "Upload")).to_have_value("50%").await;
    assert!(has(&app, by_role(Role::ProgressBar, "Loading"), Prop::Progress(None)));
}

/// Logs whether the native bar is indeterminate, each time the tweak runs.
fn log_indeterminate(log: Rc<RefCell<Vec<bool>>>) -> Tweak<Progress> {
    platform! {
        macos => mitsuami::appkit::tweak(move |p: &mitsuami::appkit::objc2_app_kit::NSProgressIndicator| {
            log.borrow_mut().push(p.isIndeterminate())
        }),
        // GTK bars have no indeterminate mode: they pulse while nothing is
        // known, with no fraction.
        gtk => mitsuami::gtk::tweak(move |p: &mitsuami::gtk::gtk::ProgressBar| log.borrow_mut().push(p.fraction() == 0.0)),
        kde => mitsuami::kirigami::tweak(move |p: &mitsuami::kirigami::QmlObject| {
            log.borrow_mut().push(p.bool("indeterminate"))
        }),
        win32 => mitsuami::win32::tweak(move |p: &mitsuami::win32::ProgressBar| {
            use mitsuami::win32::Control;
            use mitsuami::win32::windows_sys::Win32::UI::Controls::PBS_MARQUEE;
            log.borrow_mut().push(p.style() & PBS_MARQUEE != 0)
        }),
        windows => mitsuami::winui::tweak(move |p: &mitsuami::winui::bindings::ProgressBar| {
            use mitsuami::winui::windows_core::Interface;
            log.borrow_mut().push(p.cast::<mitsuami::winui::bindings::IProgressBar>()?.IsIndeterminate()?);
            Ok(())
        }),
    }
}

/// Given before the value, the tweak still runs after it, and again when
/// it changes.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_bar_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let connecting = signal(true);
    let tweak = log_indeterminate(log.clone());
    app.mount(move || Progress::new("Upload").native(tweak).value(0.5).indeterminate(connecting));

    let props = app.get_by_role(Role::ProgressBar, "Upload").native_state().props;
    assert!(props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last(), Some(&true));
    connecting.set(false);
    app.settle().await;
    assert_eq!(log.borrow().last(), Some(&false));
}

/// A bar that was indeterminate shows its value once it has one, however
/// often it goes back and forth. (On macOS 26, a bar that had animated kept
/// drawing the animation: the captures show it.)
#[mitsuami_test::test]
async fn shows_its_value_after_being_indeterminate(app: TestApp) {
    let connecting = signal(true);
    let done = signal(0.5);
    app.mount(move || {
        Column::new().padding(16).width(240).child(Progress::new("Upload").value(done).indeterminate(connecting))
    });
    let upload = by_role(Role::ProgressBar, "Upload");

    connecting.set(false);
    app.settle().await;
    assert!(has(&app, upload.clone(), Prop::Progress(Some(0.5))));
    app.assert_visual_snapshot("half").await;

    connecting.set(true);
    app.settle().await;
    assert!(has(&app, upload.clone(), Prop::Progress(None)));
    connecting.set(false);
    done.set(0.8);
    app.settle().await;
    assert!(has(&app, upload, Prop::Progress(Some(0.8))));
    app.assert_visual_snapshot("most").await;
}

mitsuami_test::main!();
