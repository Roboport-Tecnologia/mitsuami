//! Text options: a line limit, the last line cut off with the platform's
//! ellipsis, and raw platform settings. How text wraps and is measured is
//! in the conformance and layout suites.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::Prop;
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

const LONG: &str = "Native widgets only: the platform's own controls, laid out by one shared \
                    engine, with text that wraps across as many lines as it needs to at this width.";

fn height(app: &TestApp, id: &str) -> f32 {
    app.get_by_test_id(id).frame().height()
}

/// A limit makes wrapped text shorter, and one line is as high as a short
/// text; the whole text is still what's read out.
#[mitsuami_test::test]
async fn max_lines_cuts_off_wrapped_text(app: TestApp) {
    app.mount(|| {
        Column::new().width(160).align(Align::Stretch).children((
            Text::new(LONG).test_id("all"),
            Text::new(LONG).max_lines(2).test_id("two"),
            Text::new(LONG).max_lines(1).test_id("one"),
            Text::new("Short").test_id("short"),
        ))
    });
    let (all, two, one, short) = (height(&app, "all"), height(&app, "two"), height(&app, "one"), height(&app, "short"));
    assert!(all > two, "the limit didn't shorten it: {all} and {two}");
    assert!(two > one, "two lines aren't higher than one: {two} and {one}");
    assert_eq!(one, short, "one line isn't as high as a short text");
    assert!(app.get_by_test_id("one").native_state().props.contains(&Prop::MaxLines(Some(1))));
    assert_eq!(app.get_by_test_id("one").text().as_deref(), Some(LONG));
}

/// Text within the limit is as high as without one.
#[mitsuami_test::test]
async fn text_within_the_limit_keeps_its_height(app: TestApp) {
    app.mount(|| {
        Column::new()
            .align(Align::Start)
            .children((Text::new("Short").test_id("plain"), Text::new("Short").max_lines(3).test_id("limited")))
    });
    assert_eq!(app.get_by_test_id("limited").frame().size, app.get_by_test_id("plain").frame().size);
}

/// The limit follows its signal, and 0 lifts it.
#[mitsuami_test::test]
async fn max_lines_follows_its_signal(app: TestApp) {
    let lines = signal(1u32);
    app.mount(move || Column::new().width(160).child(Text::new(LONG).max_lines(lines).test_id("text")));
    let one = height(&app, "text");

    lines.set(0);
    app.settle().await;
    assert!(height(&app, "text") > one);
    assert!(app.get_by_test_id("text").native_state().props.contains(&Prop::MaxLines(None)));

    lines.set(1);
    app.settle().await;
    assert_eq!(height(&app, "text"), one);
}

/// Logs the text the native label shows, each time the tweak runs.
fn log_text(log: Rc<RefCell<Vec<String>>>) -> Tweak<Text> {
    platform! {
        macos => mitsuami::appkit::tweak(move |t: &mitsuami::appkit::objc2_app_kit::NSTextField| {
            log.borrow_mut().push(t.stringValue().to_string())
        }),
        gtk => mitsuami::gtk::tweak(move |l: &mitsuami::gtk::gtk::Label| log.borrow_mut().push(l.text().to_string())),
        kde => mitsuami::kirigami::tweak(move |l: &mitsuami::kirigami::QmlObject| log.borrow_mut().push(l.str("text"))),
        windows => mitsuami::winui::tweak(move |t: &mitsuami::winui::bindings::TextBlock| {
            use mitsuami::winui::windows_core::Interface;
            log.borrow_mut().push(t.cast::<mitsuami::winui::bindings::ITextBlock>()?.Text()?);
            Ok(())
        }),
    }
}

/// Given before the text, the tweak still runs after it, and again when it
/// changes.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_label_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let text = signal("first".to_string());
    let tweak = log_text(log.clone());
    app.mount(move || Text::new(text).native(tweak).test_id("text"));

    assert!(app.get_by_test_id("text").native_state().props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last().map(String::as_str), Some("first"));
    text.set("second".into());
    app.settle().await;
    assert_eq!(log.borrow().last().map(String::as_str), Some("second"));
}

mitsuami_test::main!();
