//! Text options: a line limit, the last line cut off with the platform's
//! ellipsis, colour, weight, italics, alignment, and raw platform settings. How text wraps and is measured is
//! in the conformance and layout suites.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{HorizontalAlign, Prop};
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
        win32 => mitsuami::win32::tweak(move |t: &mitsuami::win32::Static| {
            use mitsuami::win32::Control;
            use mitsuami::win32::windows_sys::Win32::UI::WindowsAndMessaging::{WM_GETTEXT, WM_GETTEXTLENGTH};
            let mut text = vec![0u16; t.send(WM_GETTEXTLENGTH, 0, 0) as usize + 1];
            let len = t.send(WM_GETTEXT, text.len(), text.as_mut_ptr() as isize) as usize;
            log.borrow_mut().push(String::from_utf16_lossy(&text[..len]))
        }),
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

fn has(app: &TestApp, id: &str, prop: Prop) -> bool {
    app.get_by_test_id(id).native_state().props.contains(&prop)
}

/// Each option reaches the native label, which the mirror check then reads
/// back after every settle.
#[mitsuami_test::test]
async fn colour_weight_italics_and_alignment_reach_the_label(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            Text::new("Failed").color(Color::Error).test_id("error"),
            Text::new("Fixed").color(Color::rgb(0x33, 0x66, 0x99)).test_id("rgb"),
            Text::new("Bold").weight(FontWeight::Bold).test_id("bold"),
            Text::new("Slanted").italic(true).test_id("italic"),
            Text::new("Centred").text_align(TextAlign::Center).test_id("centred"),
        ))
    });
    assert!(has(&app, "error", Prop::TextColor(Color::Error)));
    assert!(has(&app, "rgb", Prop::TextColor(Color::rgb(0x33, 0x66, 0x99))));
    assert!(has(&app, "bold", Prop::FontWeight(FontWeight::Bold)));
    assert!(has(&app, "italic", Prop::Italic(true)));
    assert!(has(&app, "centred", Prop::TextAlign(HorizontalAlign::Center)));
}

/// A heavier font is no narrower, on every platform; how much wider is the
/// platform's font's.
#[mitsuami_test::test]
async fn bold_text_is_at_least_as_wide(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).children((
            Text::new("Weighty words").test_id("regular"),
            Text::new("Weighty words").weight(FontWeight::Bold).test_id("bold"),
        ))
    });
    let regular = app.get_by_test_id("regular").frame().width();
    let bold = app.get_by_test_id("bold").frame().width();
    assert!(bold >= regular, "bold {bold} is narrower than regular {regular}");
}

/// Start and end are the text's direction's: left and right in
/// left-to-right text, the other way round in right-to-left.
#[mitsuami_test::test]
async fn alignment_follows_the_direction(app: TestApp) {
    let rtl = signal(false);
    app.mount(move || {
        Column::new().direction(move || if rtl.get() { TextDirection::Rtl } else { TextDirection::Ltr }).children((
            Text::new("Start").text_align(TextAlign::Start).test_id("start"),
            Text::new("End").text_align(TextAlign::End).test_id("end"),
        ))
    });
    assert!(has(&app, "start", Prop::TextAlign(HorizontalAlign::Left)));
    assert!(has(&app, "end", Prop::TextAlign(HorizontalAlign::Right)));

    rtl.set(true);
    app.settle().await;
    assert!(has(&app, "start", Prop::TextAlign(HorizontalAlign::Right)));
    assert!(has(&app, "end", Prop::TextAlign(HorizontalAlign::Left)));
}

/// The options follow their signals, and a new text style keeps the
/// weight and italics set over it.
#[mitsuami_test::test]
async fn the_options_follow_their_signals(app: TestApp) {
    let color = signal(Color::SecondaryLabel);
    let weight = signal(FontWeight::Semibold);
    let italic = signal(true);
    let style = signal(TextStyle::Body);
    let align = signal(TextAlign::Center);
    app.mount(move || {
        Column::new().child(
            Text::new("Changing")
                .color(color)
                .weight(weight)
                .italic(italic)
                .text_style(style)
                .text_align(align)
                .test_id("text"),
        )
    });
    assert!(has(&app, "text", Prop::FontWeight(FontWeight::Semibold)));

    style.set(TextStyle::Title);
    app.settle().await;
    assert!(has(&app, "text", Prop::FontWeight(FontWeight::Semibold)), "the style dropped the weight");
    assert!(has(&app, "text", Prop::Italic(true)), "the style dropped italics");

    color.set(Color::Accent);
    weight.set(FontWeight::Regular);
    italic.set(false);
    align.set(TextAlign::End);
    app.settle().await;
    assert!(has(&app, "text", Prop::TextColor(Color::Accent)));
    assert!(has(&app, "text", Prop::FontWeight(FontWeight::Regular)));
    assert!(has(&app, "text", Prop::Italic(false)));
    assert!(has(&app, "text", Prop::TextAlign(HorizontalAlign::Right)));
}

mitsuami_test::main!();
