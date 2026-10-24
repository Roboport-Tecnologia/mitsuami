//! `Popover`: content the platform shows by a container, over the window,
//! while the app says so, and which the user dismisses as the platform
//! lets them (Escape, a click outside), setting the app's state back.

use mitsuami::core::{Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// A button with a popover of branches.
fn page(open: Signal<bool>) -> impl View {
    Column::new().children((
        Row::new().test_id("anchor").children((
            Button::new("Contains in").on_click(move || open.set(true)),
            Popover::new(open).padding(8).gap(4).test_id("popover").children((
                Text::new("main"),
                Text::new("develop"),
                Button::new("Copy"),
            )),
        )),
        Text::new("Below"),
    ))
}

fn shown(app: &TestApp) -> Option<bool> {
    let props = app.get_by_test_id("popover").native_state().props;
    props.into_iter().find_map(|p| match p {
        Prop::PopoverShown(on) => Some(on),
        _ => None,
    })
}

/// Hidden until the app shows it: its content isn't visible, and it
/// takes no room by its parent.
#[mitsuami_test::test]
async fn shows_when_the_app_says(app: TestApp) {
    let open = signal(false);
    app.mount(move || page(open));
    app.expect(by_text("Below")).to_be_visible().await;
    app.expect(by_text("develop")).to_be_hidden().await;
    let anchor = app.get_by_test_id("anchor").frame();
    let button = app.get_by_role(Role::Button, "Contains in").frame();
    assert_eq!(anchor.height(), button.height(), "the popover takes no room in its parent");

    app.get_by_role(Role::Button, "Contains in").click().await;
    assert!(open.get());
    app.expect(by_text("develop")).to_be_visible().await;
    assert_eq!(shown(&app), Some(true));

    open.set(false);
    app.expect(by_text("develop")).to_be_hidden().await;
}

/// Its content is laid out on its own, at its natural size: a column of
/// the texts and the button, inside its padding.
#[mitsuami_test::test]
async fn lays_out_its_content_at_its_natural_size(app: TestApp) {
    let open = signal(true);
    app.mount(move || page(open));
    app.expect(by_text("develop")).to_be_visible().await;

    let popover = app.ui().frame(app.get_by_test_id("popover").id()).unwrap();
    let (main, develop) = (app.get_by_text("main").frame(), app.get_by_text("develop").frame());
    let copy = app.get_by_role(Role::Button, "Copy").frame();
    assert!(develop.y() > main.y() && copy.y() > develop.y(), "{main:?} {develop:?} {copy:?}");
    let widest = main.width().max(develop.width()).max(copy.width());
    assert_eq!(popover.width(), widest + 16.0);
    assert_eq!(app.ui().kind(app.get_by_test_id("popover").id()), Some(WidgetKind::Popover));
}

/// Escape dismisses it, as every platform's does, and the app hears it.
#[mitsuami_test::test]
async fn escape_dismisses_it(app: TestApp) {
    let open = signal(true);
    app.mount(move || page(open));
    app.expect(by_text("develop")).to_be_visible().await;

    app.get_by_role(Role::Button, "Copy").press(Key::Escape).await;
    app.expect(by_text("develop")).to_be_hidden().await;
    assert!(!open.get());
    assert_eq!(shown(&app), Some(false));
}

mitsuami_test::main!();
