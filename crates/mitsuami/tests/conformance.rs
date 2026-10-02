//! Backend conformance, v1: the executable definition of the backend
//! contract. Every backend must pass it (`MITSUAMI_NATIVE=1 cargo test`);
//! headless runs it too, so the contract itself stays honest.
//!
//! Assertions are relative ("wider", "taller", "non-empty") wherever the
//! platform's metrics decide the numbers.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::backend::CaptureError;
use mitsuami::core::{A11yAction, ActionError, Command, Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn has(app: &TestApp, query: Query, prop: Prop) -> bool {
    app.get(query).native_state().props.contains(&prop)
}

// ------------------------------------------------------------- props

#[mitsuami_test::test]
async fn every_widget_kind_is_created_with_its_props(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            Text::new("Plain text"),
            Button::new("Press me"),
            TextInput::new().a11y_label("Field").value("initial").placeholder("Type here"),
            Checkbox::new("Tick me").checked(true),
            Switch::new("Toggle me").checked(true),
        ))
    });
    assert_eq!(app.get_by_text("Plain text").native_state().kind, WidgetKind::Text);
    assert!(has(&app, by_text("Plain text"), Prop::Text("Plain text".into())));
    assert!(has(&app, by_role(Role::Button, "Press me"), Prop::Label("Press me".into())));
    assert!(has(&app, by_label("Field"), Prop::Value("initial".into())));
    assert!(has(&app, by_label("Field"), Prop::Placeholder("Type here".into())));
    assert!(has(&app, by_role(Role::Checkbox, "Tick me"), Prop::Checked(true)));
    assert!(has(&app, by_role(Role::Switch, "Toggle me"), Prop::Checked(true)));
}

#[mitsuami_test::test]
async fn reactive_props_update_the_native_widgets(app: TestApp) {
    let on = signal(false);
    app.mount(move || {
        Column::new().children((
            Text::new(move || if on.get() { "on" } else { "off" }.to_string()).test_id("text"),
            Button::new(move || if on.get() { "Turn off" } else { "Turn on" }.to_string())
                .enabled(on)
                .test_id("button"),
            Checkbox::new("Mirror").checked(on).test_id("checkbox"),
            Switch::new("Mirror switch").checked(on).test_id("switch"),
            TextInput::new().value(move || on.get().to_string()).test_id("field"),
        ))
    });

    on.set(true);
    app.settle().await;

    assert!(has(&app, by_test_id("text"), Prop::Text("on".into())));
    assert!(has(&app, by_test_id("button"), Prop::Label("Turn off".into())));
    assert!(has(&app, by_test_id("button"), Prop::Enabled(true)));
    assert!(has(&app, by_test_id("checkbox"), Prop::Checked(true)));
    assert!(has(&app, by_test_id("switch"), Prop::Checked(true)));
    assert!(has(&app, by_test_id("field"), Prop::Value("true".into())));
}

// ------------------------------------------------------------ events

#[mitsuami_test::test]
async fn buttons_report_clicks(app: TestApp) {
    let clicks = signal(0);
    app.mount(move || Button::new("Click").on_click(move || clicks.update(|c| *c += 1)));
    app.get_by_role(Role::Button, "Click").click().await;
    app.get_by_role(Role::Button, "Click").press(Key::Enter).await;
    assert_eq!(clicks.get_untracked(), 2);
}

#[mitsuami_test::test]
async fn toggles_report_their_new_state_and_show_it(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (a, b) = (seen.clone(), seen.clone());
    app.mount(move || {
        Column::new().children((
            Checkbox::new("Check").on_change(move |v| a.borrow_mut().push(("checkbox", v))),
            Switch::new("Switch").on_change(move |v| b.borrow_mut().push(("switch", v))),
        ))
    });

    app.get_by_role(Role::Checkbox, "Check").click().await;
    app.get_by_role(Role::Switch, "Switch").click().await;
    app.get_by_role(Role::Switch, "Switch").click().await;

    assert_eq!(*seen.borrow(), [("checkbox", true), ("switch", true), ("switch", false)]);
    assert!(has(&app, by_role(Role::Checkbox, "Check"), Prop::Checked(true)));
    assert!(has(&app, by_role(Role::Switch, "Switch"), Prop::Checked(false)));
}

#[mitsuami_test::test]
async fn text_fields_report_edits_keystrokes_and_submit(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (a, b) = (seen.clone(), seen.clone());
    app.mount(move || {
        TextInput::new()
            .a11y_label("Field")
            .on_input(move |t| a.borrow_mut().push(format!("input {t}")))
            .on_submit(move || b.borrow_mut().push("submit".into()))
    });
    let field = app.get_by_label("Field");

    field.fill("ab").await;
    field.type_text("c").await;
    field.press(Key::Backspace).await;
    field.press(Key::Enter).await;

    assert_eq!(*seen.borrow(), ["input ab", "input abc", "input ab", "submit"]);
    assert!(has(&app, by_label("Field"), Prop::Value("ab".into())));
}

#[mitsuami_test::test]
async fn only_return_submits_a_text_field(app: TestApp) {
    let submits = signal(0);
    app.mount(move || {
        Column::new().children((
            TextInput::new().a11y_label("Field").on_submit(move || submits.update(|n| *n += 1)),
            Button::new("Next"),
        ))
    });
    let field = app.get_by_label("Field");

    field.type_text("hello").await;
    field.press(Key::Tab).await;
    assert_eq!(submits.get_untracked(), 0, "leaving the field with Tab is not a submit");

    field.type_text("!").await;
    field.press(Key::Enter).await;
    assert_eq!(submits.get_untracked(), 1);
}

#[mitsuami_test::test]
async fn tab_moves_focus_to_the_next_text_field(app: TestApp) {
    app.mount(|| Column::new().children((TextInput::new().a11y_label("First"), TextInput::new().a11y_label("Second"))));
    app.get_by_label("First").type_text("a").await;
    app.expect(by_label("First")).to_be_focused().await;

    app.get_by_label("First").press(Key::Tab).await;

    app.expect(by_label("Second")).to_be_focused().await;
    app.expect(by_label("First")).not_to_be_focused().await;
}

/// Focuses `from`, presses Tab, and returns whether `to` got focus.
async fn tab_from(app: &TestApp, from: &str, to: &str) {
    app.get_by_label(from).focus().await;
    app.expect(by_label(from)).to_be_focused().await;
    app.get_by_label(from).press(Key::Tab).await;
    app.expect(by_label(to)).to_be_focused().await;
}

// Each Tab test uses three controls arranged so that reading order and
// on-screen (left-to-right, top-to-bottom) order disagree about what comes
// next; with only two, wrap-around would make any order pass.

#[mitsuami_test::test]
async fn tab_order_follows_reading_order_in_right_to_left_layouts(app: TestApp) {
    // Laid out right to left: on screen C | B | A.
    app.mount(|| {
        Row::new().direction(TextDirection::Rtl).align(Align::Start).children((
            TextInput::new().a11y_label("A").width(120),
            TextInput::new().a11y_label("B").width(120),
            TextInput::new().a11y_label("C").width(120),
        ))
    });
    assert!(app.get_by_label("A").frame().x() > app.get_by_label("C").frame().x());
    // By position, A (rightmost) would wrap to C (leftmost).
    tab_from(&app, "A", "B").await;
    tab_from(&app, "B", "C").await;
}

#[mitsuami_test::test]
async fn tab_order_follows_the_tree_not_the_geometry(app: TestApp) {
    // On screen, top to bottom: Second, First, Third.
    app.mount(|| {
        Container::new().size(300, 300).children((
            TextInput::new().a11y_label("First").absolute().top(100).start(0).width(120),
            TextInput::new().a11y_label("Second").absolute().top(0).start(0).width(120),
            TextInput::new().a11y_label("Third").absolute().top(200).start(0).width(120),
        ))
    });
    assert!(app.get_by_label("Second").frame().y() < app.get_by_label("First").frame().y());
    // By position, First would go to Third.
    tab_from(&app, "First", "Second").await;
    tab_from(&app, "Second", "Third").await;
}

#[mitsuami_test::test]
async fn tab_index_puts_controls_first(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            TextInput::new().a11y_label("A"),
            TextInput::new().a11y_label("B").tab_index(1),
            TextInput::new().a11y_label("C"),
        ))
    });
    // Order: B, A, C, then around again. By position, B would go to C.
    tab_from(&app, "B", "A").await;
    tab_from(&app, "A", "C").await;
    tab_from(&app, "C", "B").await;
}

#[mitsuami_test::test]
async fn tab_order_updates_when_controls_come_and_go(app: TestApp) {
    let middle = signal(false);
    app.mount(move || {
        Column::new().children((
            TextInput::new().a11y_label("Top"),
            Show::new(middle, || TextInput::new().a11y_label("Middle")),
            TextInput::new().a11y_label("Bottom"),
        ))
    });
    tab_from(&app, "Top", "Bottom").await;

    middle.set(true);
    app.settle().await;
    tab_from(&app, "Top", "Middle").await;
    tab_from(&app, "Middle", "Bottom").await;

    middle.set(false);
    app.settle().await;
    tab_from(&app, "Top", "Bottom").await;
}

#[mitsuami_test::test]
async fn hidden_controls_leave_the_tab_order(app: TestApp) {
    let hide_b = signal(true);
    app.mount(move || {
        Column::new().children((
            TextInput::new().a11y_label("A"),
            TextInput::new().a11y_label("B").hidden(hide_b),
            TextInput::new().a11y_label("C"),
        ))
    });
    tab_from(&app, "A", "C").await;

    hide_b.set(false);
    app.settle().await;
    tab_from(&app, "A", "B").await;
}

#[mitsuami_test::test]
async fn focus_changes_are_reported(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let field = |name: &'static str, log: &Rc<RefCell<Vec<String>>>| {
        let (f, b) = (log.clone(), log.clone());
        TextInput::new()
            .a11y_label(name)
            .on_focus(move || f.borrow_mut().push(format!("focus {name}")))
            .on_blur(move || b.borrow_mut().push(format!("blur {name}")))
    };
    let (a, b) = (field("A", &log), field("B", &log));
    app.mount(move || Column::new().children((a, b)));

    app.get_by_label("A").focus().await;
    app.get_by_label("A").press(Key::Tab).await;

    assert_eq!(*log.borrow(), ["focus A", "blur A", "focus B"]);
    assert_eq!(app.ui().focused(app.window()), Some(app.get_by_label("B").id()));
}

#[mitsuami_test::test]
async fn typing_into_a_field_focuses_it(app: TestApp) {
    let focused = signal(false);
    app.mount(move || TextInput::new().a11y_label("Field").on_focus(move || focused.set(true)));
    app.get_by_label("Field").type_text("x").await;
    assert!(focused.get_untracked());
    assert_eq!(app.ui().focused(app.window()), Some(app.get_by_label("Field").id()));
}

#[mitsuami_test::test]
async fn disabled_controls_refuse_actions(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            Button::new("Off").enabled(false),
            Checkbox::new("Off box").enabled(false),
            TextInput::new().a11y_label("Off field").enabled(false),
        ))
    });
    for query in [by_role(Role::Button, "Off"), by_role(Role::Checkbox, "Off box"), by_label("Off field")] {
        let id = app.get(query.clone()).id();
        let action =
            if matches!(query, Query::Label(_)) { A11yAction::SetValue("x".into()) } else { A11yAction::Activate };
        assert_eq!(app.ui().perform(id, &action), Err(ActionError::Disabled), "{query}");
    }
}

// ------------------------------------------------------- measurement

#[mitsuami_test::test]
async fn leaves_have_an_intrinsic_size(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).children((
            Text::new("Text"),
            Button::new("Button"),
            TextInput::new().a11y_label("Field"),
            Checkbox::new("Checkbox"),
            Switch::new("Switch"),
        ))
    });
    for query in [
        by_text("Text"),
        by_role(Role::Button, "Button"),
        by_label("Field"),
        by_role(Role::Checkbox, "Checkbox"),
        by_role(Role::Switch, "Switch"),
    ] {
        assert!(!app.get(query.clone()).frame().size.is_empty(), "{query} has no size");
    }
}

#[mitsuami_test::test]
async fn measurement_follows_content(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).children((
            Text::new("short"),
            Text::new("a considerably longer piece of text"),
            Button::new("OK"),
            Button::new("A much longer button caption"),
            Text::new("Big").text_style(TextStyle::LargeTitle),
            Text::new("Small").text_style(TextStyle::Caption),
        ))
    });
    let width = |q| app.get(q).frame().width();
    let height = |q| app.get(q).frame().height();
    assert!(width(by_text("a considerably longer piece of text")) > width(by_text("short")));
    assert!(width(by_role(Role::Button, "A much longer button caption")) > width(by_role(Role::Button, "OK")));
    assert!(height(by_text("Big")) > height(by_text("Small")));
}

#[mitsuami_test::test]
async fn text_changes_are_remeasured(app: TestApp) {
    let text = signal("short".to_string());
    app.mount(move || Column::new().align(Align::Start).child(Text::new(text).test_id("text")));
    let before = app.get_by_test_id("text").frame().width();

    text.set("a much, much longer text than before".into());
    app.settle().await;

    assert!(app.get_by_test_id("text").frame().width() > before);
}

// --------------------------------------------------------- structure

#[mitsuami_test::test]
async fn native_children_follow_core_order(app: TestApp) {
    let items = signal(vec!["a", "b", "c"]);
    app.mount(move || Column::new().test_id("list").child(For::new(items, |s: &&str| *s, Text::new)));
    let order = |app: &TestApp| -> Vec<String> {
        let list = app.get_by_test_id("list").native_state();
        list.children
            .iter()
            .map(|id| {
                app.ui()
                    .native_state(*id)
                    .unwrap()
                    .props
                    .iter()
                    .find_map(|p| match p {
                        Prop::Text(t) => Some(t.clone()),
                        _ => None,
                    })
                    .unwrap()
            })
            .collect()
    };
    assert_eq!(order(&app), ["a", "b", "c"]);

    items.set(vec!["c", "a", "b"]);
    app.settle().await;
    assert_eq!(order(&app), ["c", "a", "b"]);

    items.set(vec!["b"]);
    app.settle().await;
    assert_eq!(order(&app), ["b"]);
}

#[mitsuami_test::test]
async fn destroyed_subtrees_release_their_native_widgets(app: TestApp) {
    let open = signal(true);
    app.mount(move || {
        Column::new().child(Show::new(open, || {
            Column::new()
                .children((Text::new("one"), Row::new().children((Button::new("two"), Checkbox::new("three")))))
        }))
    });
    let with_panel = app.native_node_count();

    open.set(false);
    app.settle().await;
    assert_eq!(app.native_node_count(), with_panel - 5);

    let log = app.take_command_log();
    let destroyed = log.iter().filter(|c| matches!(c, Command::Destroy { .. })).count();
    assert_eq!(destroyed, 5);
}

// -------------------------------------------------------------- scrolling

fn rows(count: usize) -> Vec<Container> {
    (0..count).map(|i| Container::new().height(20).test_id(format!("row{i}"))).collect()
}

fn scroll_content(app: &TestApp, scroller: &str) -> Rect {
    let id = app.get_by_test_id(scroller).id();
    let content = app.ui().native_children(id)[0];
    app.ui().frame(content).unwrap()
}

#[mitsuami_test::test]
async fn scroll_view_content_overflows_its_viewport(app: TestApp) {
    app.mount(|| ScrollView::new().height(100).test_id("scroller").children(rows(20)));

    assert_eq!(app.get_by_test_id("scroller").frame().height(), 100.0);
    assert_eq!(scroll_content(&app, "scroller").height(), 400.0);
    assert!(app.get_by_test_id("row0").is_visible());
    assert!(!app.get_by_test_id("row10").is_visible(), "clipped by the scroll view");
    assert!(app.a11y_tree().walk().iter().any(|n| n.role == Role::ScrollArea));
}

#[mitsuami_test::test]
async fn scroll_into_view_reveals_content(app: TestApp) {
    let seen = signal(None);
    app.mount(move || {
        ScrollView::new().height(100).test_id("scroller").on_scroll(move |p| seen.set(Some(p))).children(rows(20))
    });

    app.get_by_test_id("row15").scroll_into_view().await;

    // Row 15 spans 300..320; the smallest scroll that shows it is 220.
    let scroller = app.get_by_test_id("scroller").id();
    assert_eq!(app.ui().scroll_offset(scroller), Some(Point::new(0.0, 220.0)));
    assert_eq!(seen.get_untracked(), Some(Point::new(0.0, 220.0)));
    assert!(app.get_by_test_id("row15").is_visible());
    assert!(!app.get_by_test_id("row0").is_visible());
}

#[mitsuami_test::test]
async fn scrolling_moves_content_and_clamps(app: TestApp) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    app.mount(move || {
        Column::new().padding(10).child(
            ScrollView::new()
                .height(100)
                .test_id("scroller")
                .on_scroll(move |p| log.borrow_mut().push(p.y))
                .children(rows(20)),
        )
    });
    let scroller = app.get_by_test_id("scroller");

    scroller.scroll_by(0.0, 50.0).await;
    // Window coordinates follow the scroll: the scroll view starts at y=10.
    assert_eq!(app.get_by_test_id("row0").frame().y(), 10.0 - 50.0);

    scroller.scroll_by(0.0, 10_000.0).await;
    scroller.scroll_by(0.0, -10_000.0).await;

    assert_eq!(*seen.borrow(), [50.0, 300.0, 0.0], "clamped to 0..=content-viewport");
}

#[mitsuami_test::test]
async fn horizontal_scroll_views_scroll_sideways_only(app: TestApp) {
    app.mount(|| {
        ScrollView::horizontal()
            .width(100)
            .height(40)
            .test_id("scroller")
            .children((0..10).map(|i| Container::new().width(30).test_id(format!("col{i}"))).collect::<Vec<_>>())
    });
    // As high as the viewport: the whole view where the scroll bar
    // overlays the content, less its row where it doesn't (Breeze's).
    let scroller = app.get_by_test_id("scroller").id();
    let viewport = app.ui().viewport(scroller).unwrap();
    assert!(viewport.height() <= 40.0);
    assert_eq!(scroll_content(&app, "scroller").size, Size::new(300.0, viewport.height()));

    app.get_by_test_id("scroller").scroll_by(1000.0, 1000.0).await;

    assert_eq!(app.ui().scroll_offset(scroller), Some(Point::new(300.0 - viewport.width(), 0.0)));
    assert!(app.get_by_test_id("col9").is_visible());
}

#[mitsuami_test::test]
async fn scroll_views_fill_the_space_they_are_given(app: TestApp) {
    app.mount(|| {
        // As in CSS, a scroll container's natural size is its content, so
        // siblings that must keep their size opt out of shrinking.
        Column::new().height(300).children((
            Container::new().height(50).shrink(0.0),
            ScrollView::new().grow(1.0).test_id("scroller").children(rows(50)),
        ))
    });
    assert_eq!(app.get_by_test_id("scroller").frame(), Rect::new(0.0, 50.0, 800.0, 250.0));
    assert_eq!(scroll_content(&app, "scroller").height(), 1000.0);
}

#[mitsuami_test::test]
async fn scrolled_content_renders_at_its_offset(app: TestApp) {
    app.mount(|| {
        Column::new().padding(20).child(
            ScrollView::new()
                .height(120)
                .width(240)
                .test_id("scroller")
                .children((0..30).map(|i| Text::new(format!("Line {i}"))).collect::<Vec<_>>()),
        )
    });
    app.get_by_text("Line 12").scroll_into_view().await;
    app.expect(by_text("Line 12")).to_be_visible().await;
    app.expect(by_text("Line 0")).to_be_hidden().await;
    app.assert_visual_snapshot("scrolled").await;
}

// ------------------------------------------------------------ windows

#[mitsuami_test::test]
async fn window_resizes_relayout_the_content(app: TestApp) {
    app.mount(|| Column::new().child(Container::new().height(10).test_id("fill")));
    assert_eq!(app.get_by_test_id("fill").frame().width(), 800.0);

    app.resize(500.0, 400.0).await;

    assert_eq!(app.ui().window_size(app.window()), Some(Size::new(500.0, 400.0)));
    assert_eq!(app.get_by_test_id("fill").frame().width(), 500.0);
}

#[mitsuami_test::test]
async fn windows_can_fit_their_height_to_the_content(app: TestApp) {
    let ui = app.ui().clone();
    let window = ui.create_window("fitted", WindowSize::FitHeight(300.0));
    let root = Owner::new_root()
        .with(|| Column::new().padding(10).children((Container::new().height(40), Text::new("fits"))).build(&ui));
    ui.append_child(window, root);
    app.settle().await;

    // The text's height is the platform's, so only the fixed parts are known.
    let size = ui.window_size(window).unwrap();
    assert_eq!(size.width, 300.0);
    assert!(size.height > 60.0, "the text adds to the fixed 60: {size:?}");
    // Up to a pixel of slack: platforms size windows in physical pixels.
    let slack = size.height - ui.frame(root).unwrap().height();
    assert!((0.0..1.0).contains(&slack), "the content fills the fitted window: {slack} left");
    match ui.capture(window).await {
        Err(CaptureError::Unsupported) => assert!(app.is_headless(), "only headless may lack capture"),
        Err(e) => panic!("capture failed: {e:?}"),
        Ok(image) => {
            let scale = image.scale_factor;
            assert_eq!((image.width, image.height), ((300.0 * scale) as u32, (size.height * scale) as u32));
        }
    }
    ui.destroy(window);
    app.settle().await;
}

#[mitsuami_test::test]
async fn windows_can_be_captured_at_backing_scale(app: TestApp) {
    app.mount(|| Text::new("pixels"));
    match app.ui().capture(app.window()).await {
        Err(CaptureError::Unsupported) => assert!(app.is_headless(), "only headless may lack capture"),
        Err(e) => panic!("capture failed: {e:?}"),
        Ok(image) => {
            let scale = image.scale_factor;
            assert!(scale >= 1.0);
            assert_eq!((image.width, image.height), ((800.0 * scale) as u32, (600.0 * scale) as u32));
            assert_eq!(image.rgba.len(), (image.width * image.height * 4) as usize);
            assert!(image.rgba.chunks_exact(4).any(|p| p[0] < 128), "the text should leave dark pixels");
        }
    }
}

mitsuami_test::main!();
