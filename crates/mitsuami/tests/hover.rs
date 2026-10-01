//! Hover: a container or group with `on_hover` hears the pointer come over
//! it, or over anything inside it, and leave, as the platform tracks it.
//! Tests move the pointer through each backend's own hover handling where
//! it can be reached (AppKit's tracking areas, GTK's motion controllers);
//! a real pointer is for trying by hand (`examples/hover.rs`).

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::Prop;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

type Log = Rc<RefCell<Vec<(&'static str, bool)>>>;

fn log(events: &Log, name: &'static str) -> impl Fn(bool) + 'static {
    let events = events.clone();
    move |over| events.borrow_mut().push((name, over))
}

fn reports_hover(app: &TestApp, query: Query) -> bool {
    app.get(query).native_state().props.contains(&Prop::Hover(true))
}

#[mitsuami_test::test]
async fn reports_the_pointer_coming_and_leaving(app: TestApp) {
    let events = Log::default();
    let on_hover = log(&events, "row");
    app.mount(move || Column::new().child(Row::new().test_id("row").on_hover(on_hover).child(Text::new("Play"))));

    app.get(by_test_id("row")).hover().await;
    assert_eq!(*events.borrow(), [("row", true)]);

    app.move_pointer_away().await;
    assert_eq!(*events.borrow(), [("row", true), ("row", false)]);
}

/// A pointer over a child is over its container too, as everywhere: a
/// row stays hovered while the pointer moves onto its buttons, and a box
/// inside that hovers hears its own.
#[mitsuami_test::test]
async fn a_child_keeps_its_container_hovered(app: TestApp) {
    let events = Log::default();
    let (row, actions) = (log(&events, "row"), log(&events, "actions"));
    app.mount(move || {
        Column::new().children((
            Row::new()
                .test_id("row")
                .on_hover(row)
                .children((Text::new("Windows 98"), Row::new().on_hover(actions).child(Button::new("Delete")))),
            Text::new("Elsewhere"),
        ))
    });

    app.get(by_text("Windows 98")).hover().await;
    assert_eq!(*events.borrow(), [("row", true)]);

    app.get(by_role(Role::Button, "Delete")).hover().await;
    app.get(by_text("Windows 98")).hover().await;
    assert_eq!(*events.borrow(), [("row", true), ("actions", true), ("actions", false)]);

    app.get(by_text("Elsewhere")).hover().await;
    assert_eq!(*events.borrow(), [("row", true), ("actions", true), ("actions", false), ("row", false)]);
}

#[derive(Clone, PartialEq, Debug)]
struct Machine {
    id: u32,
    name: String,
}

/// 2ksbox's machine list shows a row's Start button while it's hovered.
#[mitsuami_test::test]
async fn list_rows_report_it(app: TestApp) {
    let machines =
        signal(vec![Machine { id: 1, name: "Windows 98".into() }, Machine { id: 2, name: "Windows XP".into() }]);
    let hovered = signal(None::<u32>);
    app.mount(move || {
        List::new(
            machines,
            |m: &Machine| m.id,
            move |m| {
                let id = m.id;
                Row::new()
                    .height(24)
                    .on_hover(move |over| hovered.update(|h| *h = if over { Some(id) } else { h.filter(|h| *h != id) }))
                    .children((Text::new(m.name), Button::new("Start").hidden(move || hovered.get() != Some(id))))
            },
        )
        .height(100)
    });
    let start = by_role(Role::Button, "Start");

    assert_eq!(app.get(start.clone()).count(), 0);
    app.get(by_text("Windows XP")).hover().await;
    assert_eq!(hovered.get(), Some(2));
    app.expect(start.clone()).to_exist().await;

    // Onto the button it showed: still over the row, so it stays.
    app.get(start.clone()).hover().await;
    assert_eq!(hovered.get(), Some(2));
    app.expect(start.clone()).to_exist().await;

    app.get(by_text("Windows 98")).hover().await;
    assert_eq!(hovered.get(), Some(1));
    assert_eq!(app.get(start.clone()).count(), 1);

    app.move_pointer_away().await;
    assert_eq!(hovered.get(), None);
    app.expect(start).not_to_exist().await;
}

/// Each backend tracks it on containers, groups and list rows, and
/// only where the app asked.
#[mitsuami_test::test]
async fn containers_and_groups_report_it(app: TestApp) {
    app.mount(move || {
        Column::new().test_id("box").on_hover(|_| {}).children((
            Row::new().test_id("row").on_hover(|_| {}).child(Text::new("In a row")),
            Group::new().title("Display").child(Text::new("In a group")).on_hover(|_| {}),
            List::new(
                vec!["Windows 98"],
                |name: &&str| *name,
                |name| Container::new().height(24).test_id("list row").on_hover(|_| {}).child(Text::new(name)),
            )
            .height(60),
            Row::new().test_id("plain").child(Text::new("Not asked")),
        ))
    });

    for query in [by_test_id("box"), by_test_id("row"), by_role(Role::Group, "Display"), by_test_id("list row")] {
        assert!(reports_hover(&app, query.clone()), "{query}");
    }
    assert!(!reports_hover(&app, by_test_id("plain")));
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let over = signal(false);
    app.mount(move || view! { <Row @hover=move |o| over.set(o)><Text>"Play"</Text></Row> });

    app.get(by_text("Play")).hover().await;
    assert!(over.get());
}

mitsuami_test::main!();
