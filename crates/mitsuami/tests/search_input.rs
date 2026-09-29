//! SearchInput: the platform's search field. It takes text as a text field
//! does, asks for a search when the platform does (as the user types, at
//! once or after a pause; on Return; when cleared), and reads as a search
//! field named by its label or placeholder, its text as its value.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use mitsuami::core::{A11yAction, ActionError, Command, Prop};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// A list of fruit, filtered by the searches the field asks for.
fn fruit(searches: Rc<RefCell<Vec<String>>>) -> impl View {
    let text = signal(String::new());
    let query = signal(String::new());
    let search = move |q: String| {
        searches.borrow_mut().push(q.clone());
        query.set(q);
    };
    Column::new().padding(16).gap(8).children((
        SearchInput::new().a11y_label("Search").placeholder("Fruit").bind(text).on_search(search),
        Text::new(move || format!("{} typed", text.get().chars().count())),
        Text::new(move || {
            let q = query.get();
            let found: Vec<&str> =
                ["apple", "apricot", "banana"].into_iter().filter(|f| f.contains(q.as_str())).collect();
            format!("Found {} for \"{q}\"", found.join(", "))
        }),
        Button::new("Done"),
    ))
}

fn mount(app: &TestApp) -> Rc<RefCell<Vec<String>>> {
    let searches = Rc::new(RefCell::new(Vec::new()));
    let s = searches.clone();
    app.mount(move || fruit(s));
    searches
}

/// Lets the platform run for a while, past any search it would ask for
/// after a pause: GTK waits 150 ms, AppKit 200 to 500 ms (longer for
/// shorter text). Assertions don't wait for the platform on their own.
async fn wait_past_the_search_delay(app: &TestApp) {
    app.ui().spawn_local(async {
        spawn_blocking(|| std::thread::sleep(Duration::from_millis(1000))).await;
    });
    app.wait_for_tasks().await;
}

#[mitsuami_test::test]
async fn binds_both_ways(app: TestApp) {
    let text = signal("first".to_string());
    app.mount(move || SearchInput::new().a11y_label("Search").bind(text));
    let field = app.get_by_label("Search");
    assert!(field.native_state().props.contains(&Prop::Value("first".into())));

    field.fill("second").await;
    assert_eq!(text.get_untracked(), "second");

    text.set("third".into());
    app.settle().await;
    assert!(field.native_state().props.contains(&Prop::Value("third".into())));
}

#[mitsuami_test::test]
async fn typing_updates_on_every_keystroke(app: TestApp) {
    mount(&app);
    let field = app.get_by_label("Search");

    field.type_text("apr").await;
    app.expect(by_text("3 typed")).to_exist().await;
    field.press(Key::Backspace).await;
    app.expect(by_text("2 typed")).to_exist().await;
    assert!(field.native_state().props.contains(&Prop::Value("ap".into())));
}

/// Every platform searches as the user types: Qt and WinUI at once, AppKit
/// and GTK once typing pauses. Whenever it comes, the last search is for
/// what the field shows.
#[mitsuami_test::test]
async fn searches_as_the_user_types(app: TestApp) {
    let searches = mount(&app);
    app.get_by_label("Search").type_text("apr").await;
    wait_past_the_search_delay(&app).await;
    app.expect(by_text("Found apricot for \"apr\"")).to_exist().await;
    assert_eq!(searches.borrow().last().map(String::as_str), Some("apr"));
}

/// Return searches again, even for text already searched for.
#[mitsuami_test::test]
async fn return_searches(app: TestApp) {
    let searches = mount(&app);
    let field = app.get_by_label("Search");
    field.type_text("ap").await;
    wait_past_the_search_delay(&app).await;
    app.expect(by_text("Found apple, apricot for \"ap\"")).to_exist().await;

    let before = searches.borrow().len();
    field.press(Key::Enter).await;
    app.expect(by_text("Found apple, apricot for \"ap\"")).to_exist().await;
    assert_eq!(searches.borrow()[before..], ["ap".to_string()]);
}

/// Emptying the field searches for nothing, as its clear button does.
#[mitsuami_test::test]
async fn clearing_searches_for_nothing(app: TestApp) {
    let searches = mount(&app);
    let field = app.get_by_label("Search");
    field.type_text("b").await;
    wait_past_the_search_delay(&app).await;
    app.expect(by_text("Found banana for \"b\"")).to_exist().await;

    field.press(Key::Backspace).await;
    wait_past_the_search_delay(&app).await;
    app.expect(by_text("Found apple, apricot, banana for \"\"")).to_exist().await;
    assert_eq!(searches.borrow().last().map(String::as_str), Some(""));
}

/// Assistive technology's edits are the user's: they search too.
#[mitsuami_test::test]
async fn a_set_value_searches(app: TestApp) {
    let searches = mount(&app);
    let field = app.get_by_label("Search");
    app.ui().perform(field.id(), &A11yAction::SetValue("ban".into())).unwrap();
    app.expect(by_text("Found banana for \"ban\"")).to_exist().await;
    assert_eq!(searches.borrow().last().map(String::as_str), Some("ban"));
}

/// Text the app sets is shown, but isn't searched for: GTK's delayed
/// search and Qt's automatic one would, so their backends guard it.
#[mitsuami_test::test]
async fn text_the_app_sets_is_not_searched_for(app: TestApp) {
    let searches = Rc::new(RefCell::new(Vec::new()));
    let text = signal(String::new());
    let s = searches.clone();
    app.mount(move || SearchInput::new().a11y_label("Search").value(text).on_search(move |q| s.borrow_mut().push(q)));
    text.set("apple".into());
    app.settle().await;
    wait_past_the_search_delay(&app).await;
    assert!(app.get_by_label("Search").native_state().props.contains(&Prop::Value("apple".into())));
    assert!(searches.borrow().is_empty(), "searched for {:?}", searches.borrow());

    // Emptied by the app, it doesn't search either (GTK searches for an
    // empty field at once).
    text.set(String::new());
    app.settle().await;
    wait_past_the_search_delay(&app).await;
    assert!(searches.borrow().is_empty(), "searched for {:?}", searches.borrow());
}

#[mitsuami_test::test]
async fn native_edits_are_not_echoed_back_to_the_widget(app: TestApp) {
    mount(&app);
    let input = app.get_by_label("Search").id();
    app.take_command_log();

    app.get_by_label("Search").type_text("ap").await;
    wait_past_the_search_delay(&app).await;

    let echoes: Vec<Command> = app
        .take_command_log()
        .into_iter()
        .filter(|c| matches!(c, Command::SetProp { id, .. } if *id == input))
        .collect();
    assert_eq!(echoes, vec![], "the native search field already shows what the user typed");
}

/// Tab moves on from it to the next field, as from a text field. (Not to
/// a button: AppKit's take focus only with Full Keyboard Access.)
#[mitsuami_test::test]
async fn tab_moves_focus_on(app: TestApp) {
    app.mount(|| {
        Column::new().children((SearchInput::new().a11y_label("Search"), TextInput::new().a11y_label("Note")))
    });
    let field = app.get_by_label("Search");
    field.type_text("a").await;
    app.expect(by_label("Search")).to_be_focused().await;
    field.press(Key::Tab).await;
    app.expect(by_label("Note")).to_be_focused().await;
    app.expect(by_label("Search")).not_to_be_focused().await;
}

/// A search field, named by its label or its placeholder, whose value is
/// its text.
#[mitsuami_test::test]
async fn reads_as_a_search_field(app: TestApp) {
    app.mount(|| {
        Column::new().children((
            SearchInput::new().a11y_label("Search").value("apple"),
            SearchInput::new().placeholder("Find in page"),
        ))
    });
    let field = app.get_by_role(Role::SearchField, "Search");
    assert_eq!(field.value().as_deref(), Some("apple"));
    app.expect(by_role(Role::SearchField, "Find in page")).to_exist().await;
    app.assert_a11y_snapshot("fields");
}

#[mitsuami_test::test]
async fn a_disabled_field_takes_no_input(app: TestApp) {
    app.mount(|| SearchInput::new().a11y_label("Search").enabled(false));
    let field = app.get_by_label("Search");
    app.expect(by_label("Search")).to_be_disabled().await;
    assert_eq!(app.ui().perform(field.id(), &A11yAction::SetValue("a".into())), Err(ActionError::Disabled));
}

/// About a text field's height, with room for its icon and clear button.
#[mitsuami_test::test]
async fn has_a_size_like_a_text_field(app: TestApp) {
    app.mount(|| {
        Column::new()
            .align(Align::Start)
            .children((TextInput::new().a11y_label("Text"), SearchInput::new().a11y_label("Search")))
    });
    let text = app.get_by_label("Text").frame().size;
    let search = app.get_by_label("Search").frame().size;
    assert!(search.width > 0.0);
    assert!((search.height - text.height).abs() <= text.height * 0.25, "{search:?} isn't about as tall as {text:?}");
}

#[mitsuami_test::test]
async fn works_in_view_macros(app: TestApp) {
    let text = signal(String::new());
    let searched = signal(String::new());
    app.mount(move || view! { <SearchInput a11y_label="Search" bind=text @search=move |q| searched.set(q)/> });
    app.get_by_label("Search").fill("ap").await;
    assert_eq!(text.get_untracked(), "ap");
    app.get_by_label("Search").press(Key::Enter).await;
    assert_eq!(searched.get_untracked(), "ap");
}

/// Logs the text the native field holds, each time the tweak runs.
fn log_text(log: Rc<RefCell<Vec<String>>>) -> Tweak<SearchInput> {
    platform! {
        macos => mitsuami::appkit::tweak(move |f: &mitsuami::appkit::objc2_app_kit::NSSearchField| {
            log.borrow_mut().push(f.stringValue().to_string())
        }),
        gtk => mitsuami::gtk::tweak(move |e: &mitsuami::gtk::gtk::SearchEntry| {
            use mitsuami::gtk::gtk::prelude::*;
            log.borrow_mut().push(e.text().to_string())
        }),
        kde => mitsuami::kirigami::tweak(move |f: &mitsuami::kirigami::QmlObject| log.borrow_mut().push(f.str("text"))),
        windows => mitsuami::winui::tweak(move |f: &mitsuami::winui::bindings::AutoSuggestBox| {
            use mitsuami::winui::windows_core::Interface;
            log.borrow_mut().push(f.cast::<mitsuami::winui::bindings::IAutoSuggestBox>()?.Text()?.to_string());
            Ok(())
        }),
    }
}

/// Given before the text, the tweak still runs after it, and again when it
/// changes.
#[mitsuami_test::test]
async fn a_tweak_runs_on_the_native_field_after_its_props(app: TestApp) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let value = signal("first".to_string());
    let tweak = log_text(log.clone());
    app.mount(move || SearchInput::new().a11y_label("Search").native(tweak).value(value));

    let props = app.get_by_label("Search").native_state().props;
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

mitsuami_test::main!();
