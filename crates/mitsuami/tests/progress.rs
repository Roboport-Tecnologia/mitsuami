//! `Progress`: a progress bar, as the platform draws one. It shows how far
//! along a task is, or animates for work of unknown length, and reads as a
//! progress bar named by its label whose value is the percentage.

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

mitsuami_test::main!();
