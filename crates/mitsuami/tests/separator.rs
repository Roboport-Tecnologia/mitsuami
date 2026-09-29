//! `Separator`: the platform's line between groups of content. It runs
//! across a column, or down a row when vertical, as thick as the platform
//! draws it; it reads as a separator, and takes no focus.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{A11yAction, ActionError, Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn separator(app: &TestApp) -> Locator<'_> {
    app.get(Query::Role(Role::Separator, None))
}

/// A line across the column, between what's above and below. Each
/// platform draws its own thickness: 1 pt or px on every one so far.
#[mitsuami_test::test]
async fn runs_across_a_column(app: TestApp) {
    app.mount(|| {
        Column::new().width(240).gap(8).children((Text::new("General"), Separator::new(), Text::new("Advanced")))
    });

    let line = separator(&app);
    assert_eq!(line.native_state().kind, WidgetKind::Separator);
    assert!(line.native_state().props.contains(&Prop::Orientation(Orientation::Horizontal)));
    let frame = line.frame();
    let (above, below) = (app.get_by_text("General").frame(), app.get_by_text("Advanced").frame());
    assert_eq!(frame.size.width, 240.0, "{frame}");
    assert!(frame.size.height > 0.0 && frame.size.height < above.size.height, "{frame}");
    assert!(frame.origin.y >= above.origin.y + above.size.height, "{frame} under {above}");
    assert!(frame.origin.y + frame.size.height <= below.origin.y, "{frame} over {below}");
}

/// Vertical, a line down the row, as tall as what's beside it.
#[mitsuami_test::test]
async fn runs_down_a_row(app: TestApp) {
    app.mount(|| Row::new().gap(8).children((Button::new("Back"), Separator::vertical(), Button::new("Forward"))));

    let line = separator(&app);
    assert!(line.native_state().props.contains(&Prop::Orientation(Orientation::Vertical)));
    let frame = line.frame();
    let back = app.get_by_role(Role::Button, "Back").frame();
    assert_eq!(frame.size.height, back.size.height, "{frame} beside {back}");
    assert!(frame.size.width > 0.0 && frame.size.width < back.size.width, "{frame}");
}

/// Turned with the box it's in, as a toolbar that moves from the top to
/// the side turns its separators.
#[mitsuami_test::test]
async fn turns_when_its_orientation_changes(app: TestApp) {
    let orientation = signal(Orientation::Horizontal);
    app.mount(move || {
        let direction = move || match orientation.get() {
            Orientation::Horizontal => FlexDirection::Column,
            Orientation::Vertical => FlexDirection::Row,
        };
        Container::new().flex_direction(direction).width(240).height(120).children((
            Text::new("One"),
            Separator::new().orientation(orientation),
            Text::new("Two"),
        ))
    });
    let across = separator(&app).frame();
    assert_eq!(across.size.width, 240.0, "{across}");
    assert!(across.size.height > 0.0 && across.size.height < 120.0, "{across}");

    orientation.set(Orientation::Vertical);
    app.settle().await;
    assert!(separator(&app).native_state().props.contains(&Prop::Orientation(Orientation::Vertical)));
    let down = separator(&app).frame();
    assert_eq!(down.size.height, 120.0, "{down}");
    assert_eq!(down.size.width, across.size.height, "{down}");
}

/// Assistive technology can't act on it, and Tab goes past it.
#[mitsuami_test::test]
async fn takes_no_focus_or_actions(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            TextInput::new().a11y_label("First"),
            Separator::new(),
            TextInput::new().a11y_label("Second"),
        ))
    });
    let line = separator(&app).node();
    assert_eq!(line.name, None);
    assert_eq!(app.ui().perform(line.id, &A11yAction::Activate), Err(ActionError::Unsupported));

    app.get_by_label("First").focus().await;
    app.get_by_label("First").press(Key::Tab).await;
    app.expect(by_label("Second")).to_be_focused().await;
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    app.mount(|| {
        view! {
            <Row>
                <Text>"One"</Text>
                <Separator orientation=Orientation::Vertical/>
                <Text>"Two"</Text>
            </Row>
        }
    });

    assert!(separator(&app).native_state().props.contains(&Prop::Orientation(Orientation::Vertical)));
}

/// Logs that the tweak ran on the native separator.
fn log_runs(log: Rc<RefCell<usize>>) -> Tweak<Separator> {
    platform! {
        macos => mitsuami::appkit::tweak(move |_: &mitsuami::appkit::objc2_app_kit::NSBox| *log.borrow_mut() += 1),
        gtk => mitsuami::gtk::tweak(move |_: &mitsuami::gtk::gtk::Separator| *log.borrow_mut() += 1),
        kde => mitsuami::kirigami::tweak(move |_: &mitsuami::kirigami::QmlObject| *log.borrow_mut() += 1),
        windows => mitsuami::winui::tweak(move |_: &mitsuami::winui::bindings::Border| {
            *log.borrow_mut() += 1;
            Ok(())
        }),
    }
}

#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_separator_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(0));
    let orientation = signal(Orientation::Horizontal);
    let tweak = log_runs(log.clone());
    app.mount(move || Separator::new().native(tweak).orientation(orientation));

    assert!(separator(&app).native_state().props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert_eq!(*log.borrow(), 0);
        return;
    }
    let runs = *log.borrow();
    assert!(runs > 0);
    orientation.set(Orientation::Vertical);
    app.settle().await;
    assert!(*log.borrow() > runs);
}

mitsuami_test::main!();
