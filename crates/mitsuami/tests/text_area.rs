//! TextArea: the platform's text over many lines. Return starts a new line
//! and never submits; Tab is the platform's (a tab, or the next control).
//! It's as tall as its lines of the platform's text, scrolls past them,
//! and reads as a text area named by its label or placeholder whose value
//! is its text.

use std::cell::RefCell;
use std::rc::Rc;

use mitsuami::core::{A11yAction, ActionError, Command, Prop, SyntheticInput};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn has(app: &TestApp, label: &str, prop: Prop) -> bool {
    app.get_by_label(label).native_state().props.contains(&prop)
}

/// A note and what it says: its lines and its characters.
fn note() -> impl View {
    let text = signal(String::new());
    Column::new().padding(16).gap(8).children((
        TextArea::new().a11y_label("Note").bind(text),
        Text::new(move || {
            let text = text.get();
            format!("{} lines, {} characters", text.lines().count(), text.chars().count())
        }),
        TextInput::new().a11y_label("Next"),
    ))
}

#[mitsuami_test::test]
async fn binds_both_ways(app: TestApp) {
    let text = signal("first\nline".to_string());
    app.mount(move || TextArea::new().a11y_label("Note").bind(text));
    assert!(has(&app, "Note", Prop::Value("first\nline".into())));

    app.get_by_label("Note").fill("second\nand third").await;
    assert_eq!(text.get_untracked(), "second\nand third");

    text.set("fourth".into());
    app.settle().await;
    assert!(has(&app, "Note", Prop::Value("fourth".into())));
}

/// Return starts a new line, on every platform.
#[mitsuami_test::test]
async fn typing_updates_on_every_keystroke(app: TestApp) {
    app.mount(note);
    let area = app.get_by_label("Note");

    area.type_text("Hi").await;
    app.expect(by_text("1 lines, 2 characters")).to_exist().await;
    area.press(Key::Enter).await;
    area.type_text("there").await;
    app.expect(by_text("2 lines, 8 characters")).to_exist().await;
    area.press(Key::Backspace).await;
    assert!(has(&app, "Note", Prop::Value("Hi\nther".into())));
    app.expect(by_label("Note")).to_be_focused().await;
}

#[mitsuami_test::test]
async fn native_edits_are_not_echoed_back_to_the_widget(app: TestApp) {
    app.mount(note);
    let area = app.get_by_label("Note").id();
    app.take_command_log();

    app.get_by_label("Note").type_text("a").await;
    app.get_by_label("Note").press(Key::Enter).await;

    let echoes: Vec<Command> = app
        .take_command_log()
        .into_iter()
        .filter(|c| matches!(c, Command::SetProp { id, .. } if *id == area))
        .collect();
    assert_eq!(echoes, vec![], "the native text area already shows what the user typed");
}

/// Tab inserts a tab where the platform's text areas take it (AppKit, GTK,
/// Qt); WinUI's text box takes no tabs, and moves focus on, as a field does.
#[mitsuami_test::test]
async fn tab_is_the_platforms(app: TestApp) {
    app.mount(note);
    let area = app.get_by_label("Note");
    area.type_text("a").await;
    area.press(Key::Tab).await;
    if app.backend_name() == "winui" {
        app.expect(by_label("Next")).to_be_focused().await;
        assert!(has(&app, "Note", Prop::Value("a".into())));
    } else {
        app.expect(by_label("Note")).to_be_focused().await;
        assert!(has(&app, "Note", Prop::Value("a\t".into())));
    }
}

/// A text area, named by its label or its placeholder, whose value is its
/// text.
#[mitsuami_test::test]
async fn reads_as_a_text_area(app: TestApp) {
    app.mount(|| {
        Column::new()
            .children((TextArea::new().a11y_label("Note").value("Two\nlines"), TextArea::new().placeholder("Comments")))
    });
    let area = app.get_by_role(Role::TextArea, "Note");
    assert_eq!(area.value().as_deref(), Some("Two\nlines"));
    app.expect(by_role(Role::TextArea, "Comments")).to_exist().await;
    app.assert_a11y_snapshot("areas");
}

#[mitsuami_test::test]
async fn a_read_only_area_takes_no_edits(app: TestApp) {
    let edits = Rc::new(RefCell::new(Vec::new()));
    let log = edits.clone();
    app.mount(move || {
        TextArea::new()
            .a11y_label("Licence")
            .value("Permission is granted\nto anyone")
            .read_only(true)
            .on_input(move |text| log.borrow_mut().push(text))
    });

    let area = app.get_by_label("Licence");
    app.expect(by_label("Licence")).to_be_read_only().await;
    for key in [Key::Char('x'), Key::Backspace, Key::Enter] {
        assert_eq!(app.ui().synthesize(area.id(), &SyntheticInput::Key(key)), Err(ActionError::ReadOnly));
    }
    assert_eq!(app.ui().perform(area.id(), &A11yAction::SetValue("y".into())), Err(ActionError::ReadOnly));
    app.settle().await;
    assert!(has(&app, "Licence", Prop::Value("Permission is granted\nto anyone".into())));
    assert!(edits.borrow().is_empty());
}

/// Read-only and enabled change independently, and the app can still set
/// the text of either.
#[mitsuami_test::test]
async fn read_only_and_enabled_follow_their_signals(app: TestApp) {
    let (read_only, enabled) = (signal(true), signal(false));
    let text = signal("one".to_string());
    app.mount(move || TextArea::new().a11y_label("Note").value(text).read_only(read_only).enabled(enabled));
    app.expect(by_label("Note")).to_be_disabled().await;

    enabled.set(true);
    app.settle().await;
    app.expect(by_label("Note")).to_be_read_only().await;
    text.set("two".into());
    app.settle().await;
    assert!(has(&app, "Note", Prop::Value("two".into())));

    read_only.set(false);
    app.settle().await;
    app.get_by_label("Note").type_text("!").await;
    // Typing goes in at the caret. XAML focuses the first control as the
    // window activates, and text set while it has focus puts the caret at
    // the start.
    let typed = if app.backend_name() == "winui" { "!two" } else { "two!" };
    assert!(has(&app, "Note", Prop::Value(typed.into())));
}

#[mitsuami_test::test]
async fn a_disabled_area_takes_no_input(app: TestApp) {
    app.mount(|| TextArea::new().a11y_label("Note").enabled(false));
    let area = app.get_by_label("Note");
    app.expect(by_label("Note")).to_be_disabled().await;
    assert_eq!(app.ui().perform(area.id(), &A11yAction::SetValue("x".into())), Err(ActionError::Disabled));
    assert_eq!(app.ui().synthesize(area.id(), &SyntheticInput::Key(Key::Char('x'))), Err(ActionError::Disabled));
}

/// Selects all the text the first time it runs, then logs how many
/// characters are selected each time it runs again.
fn select_then_log(log: Rc<RefCell<Vec<usize>>>) -> Tweak<TextArea> {
    let first = Rc::new(std::cell::Cell::new(true));
    platform! {
        macos => mitsuami::appkit::tweak(move |t: &mitsuami::appkit::objc2_app_kit::NSTextView| {
            if first.replace(false) {
                t.setSelectedRange(mitsuami::appkit::objc2_foundation::NSRange::new(0, t.string().length()));
            } else {
                log.borrow_mut().push(t.selectedRange().length);
            }
        }),
        gtk => mitsuami::gtk::tweak(move |v: &mitsuami::gtk::gtk::TextView| {
            use mitsuami::gtk::gtk::prelude::*;
            let buffer = v.buffer();
            if first.replace(false) {
                buffer.select_range(&buffer.start_iter(), &buffer.end_iter());
            } else {
                let length = buffer.selection_bounds().map_or(0, |(start, end)| (end.offset() - start.offset()) as usize);
                log.borrow_mut().push(length);
            }
        }),
        kde => mitsuami::kirigami::tweak(move |a: &mitsuami::kirigami::QmlObject| {
            if first.replace(false) {
                a.invoke("selectAll");
            } else {
                log.borrow_mut().push(a.str("selectedText").chars().count());
            }
        }),
        windows => mitsuami::winui::tweak(move |t: &mitsuami::winui::bindings::TextBox| {
            use mitsuami::winui::windows_core::Interface;
            let field = t.cast::<mitsuami::winui::bindings::ITextBox>()?;
            if first.replace(false) {
                field.SetSelectionStart(0)?;
                field.SetSelectionLength(field.Text()?.encode_utf16().count() as i32)?;
            } else {
                log.borrow_mut().push(field.SelectionLength()? as usize);
            }
            Ok(())
        }),
    }
}

/// Disabled, it shows no selection: disabling it drops the one it had, as
/// disabling a field ends its editing. Its text stays.
#[mitsuami_test::test]
async fn disabling_clears_the_selection(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let enabled = signal(true);
    let tweak = select_then_log(log.clone());
    app.mount(move || TextArea::new().a11y_label("Note").value("Selected text").enabled(enabled).native(tweak));
    if app.is_headless() {
        return;
    }
    enabled.set(false);
    app.settle().await;
    app.expect(by_label("Note")).to_be_disabled().await;
    assert_eq!(log.borrow().last(), Some(&0), "a disabled text area still shows its selection");
    assert!(has(&app, "Note", Prop::Value("Selected text".into())));
}

/// Taller than a text field, and taller with more lines, in each
/// platform's line height.
#[mitsuami_test::test]
async fn is_as_tall_as_its_lines(app: TestApp) {
    app.mount(|| {
        Column::new().align(Align::Start).children((
            TextInput::new().a11y_label("Field"),
            TextArea::new().a11y_label("One").lines(1),
            TextArea::new().a11y_label("Three"),
            TextArea::new().a11y_label("Six").lines(6),
        ))
    });
    let height = |label: &str| app.get_by_label(label).frame().size.height;
    let (field, one, three, six) = (height("Field"), height("One"), height("Three"), height("Six"));
    assert!(one > 0.0 && one < three && three < six, "{one} {three} {six}");
    assert!(three > field, "three lines ({three}) are no taller than a text field ({field})");
    // Each line adds the same height.
    let line = (six - three) / 3.0;
    assert!(((three - one) / 2.0 - line).abs() <= 1.0, "{one} {three} {six}");
}

/// Its text doesn't count: with text or without, and as its lines
/// change, it's as tall as its lines.
#[mitsuami_test::test]
async fn is_as_tall_as_its_lines_whatever_its_text(app: TestApp) {
    let lines = signal(1);
    let text = "One\nTwo\nThree\nFour".to_string();
    app.mount(move || {
        Column::new().align(Align::Start).children((
            TextArea::new().a11y_label("Empty").lines(lines),
            TextArea::new().a11y_label("Full").lines(lines).value(text.clone()),
        ))
    });
    let height = |label: &str| app.get_by_label(label).frame().size.height;
    let mut last = 0.0;
    for n in [1, 3, 6] {
        lines.set(n);
        app.settle().await;
        assert_eq!(height("Full"), height("Empty"), "{n} lines");
        assert!(height("Empty") > last, "{n} lines: {} after {last}", height("Empty"));
        last = height("Empty");
    }
}

/// Text past its lines scrolls inside it: it doesn't grow.
#[mitsuami_test::test]
async fn scrolls_past_its_lines(app: TestApp) {
    let text = signal(String::new());
    app.mount(move || Column::new().align(Align::Start).children(TextArea::new().a11y_label("Note").value(text)));
    let empty = app.get_by_label("Note").frame().size;
    text.set((1..=40).map(|n| format!("Line {n}")).collect::<Vec<_>>().join("\n"));
    app.settle().await;
    assert_eq!(app.get_by_label("Note").frame().size, empty);
}

/// The layout can make it larger than its lines.
#[mitsuami_test::test]
async fn takes_the_size_the_layout_gives(app: TestApp) {
    app.mount(|| Column::new().children(TextArea::new().a11y_label("Note").height(160)));
    assert_eq!(app.get_by_label("Note").frame().size.height, 160.0);
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let text = signal(String::new());
    app.mount(move || view! { <TextArea a11y_label="Note" lines=5 bind=text/> });
    app.get_by_label("Note").fill("a\nb").await;
    assert_eq!(text.get_untracked(), "a\nb");
    assert!(has(&app, "Note", Prop::Lines(5)));
}

/// Logs the text the native text view holds, each time the tweak runs.
fn log_text(log: Rc<RefCell<Vec<String>>>) -> Tweak<TextArea> {
    platform! {
        macos => mitsuami::appkit::tweak(move |t: &mitsuami::appkit::objc2_app_kit::NSTextView| {
            log.borrow_mut().push(t.string().to_string())
        }),
        gtk => mitsuami::gtk::tweak(move |v: &mitsuami::gtk::gtk::TextView| {
            use mitsuami::gtk::gtk::prelude::*;
            let buffer = v.buffer();
            log.borrow_mut().push(buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string())
        }),
        kde => mitsuami::kirigami::tweak(move |a: &mitsuami::kirigami::QmlObject| log.borrow_mut().push(a.str("text"))),
        windows => mitsuami::winui::tweak(move |t: &mitsuami::winui::bindings::TextBox| {
            use mitsuami::winui::windows_core::Interface;
            log.borrow_mut().push(t.cast::<mitsuami::winui::bindings::ITextBox>()?.Text()?);
            Ok(())
        }),
    }
}

/// Given before the text, the tweak still runs after it, and again when it
/// changes.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_text_view_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let value = signal("first".to_string());
    let tweak = log_text(log.clone());
    app.mount(move || TextArea::new().a11y_label("Note").native(tweak).value(value));

    let props = app.get_by_label("Note").native_state().props;
    assert!(props.iter().any(|p| matches!(p, Prop::Tweak(_))));
    if app.is_headless() {
        assert!(log.borrow().is_empty());
        return;
    }
    assert_eq!(log.borrow().last().map(String::as_str), Some("first"));
    value.set("second".into());
    app.settle().await;
    assert_eq!(log.borrow().last().map(String::as_str), Some("second"));
}

/// Its lines wrap unless the app says otherwise; without, a long line
/// scrolls sideways, and the area keeps its size either way.
#[mitsuami_test::test]
async fn lines_wrap_unless_the_app_says_otherwise(app: TestApp) {
    let wrap = signal(false);
    let long = "A line much longer than a text area is wide. ".repeat(8);
    app.mount(move || {
        Column::new()
            .align(Align::Start)
            .children(TextArea::new().a11y_label("Log").value(long.clone()).line_wrap(wrap))
    });
    assert!(has(&app, "Log", Prop::LineWrap(false)));
    let size = app.get_by_label("Log").frame().size;

    wrap.set(true);
    app.settle().await;
    assert!(has(&app, "Log", Prop::LineWrap(true)));
    assert_eq!(app.get_by_label("Log").frame().size, size);
    app.get_by_label("Log").fill("typed").await;
    app.expect(by_label("Log")).to_have_value("typed").await;
}

mitsuami_test::main!();
