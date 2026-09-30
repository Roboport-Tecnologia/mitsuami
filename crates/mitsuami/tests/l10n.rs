//! Localization: the app's messages in the user's language, plural forms,
//! numbers and dates the platform writes, right-to-left languages, and
//! mitsuami's own strings. The translations are in `tests/locales`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mitsuami::core::l10n::{CurrencyDisplay, DateTimeFormat, DateTimeStyle, NumberFormat, NumberStyle};
use mitsuami::core::{HorizontalAlign, LayoutDirection, Prop};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn app_locales() -> Locales {
    locales!("locales")
}

/// 2026-09-29 15:04:05 UTC.
fn tuesday() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_790_694_245)
}

/// Messages are in the first of the user's languages the app has, and
/// follow when that changes.
#[mitsuami_test::test]
async fn messages_are_in_the_users_language(app: TestApp) {
    app.set_locales(app_locales());
    app.mount(|| Column::new().children((Text::new(t!("greeting", name = "Ana")), Button::new(t!("save")))));
    app.expect(by_text("Hello, Ana!")).to_be_visible().await;
    app.expect(by_role(Role::Button, "Save")).to_be_visible().await;

    app.set_languages(&["pt-BR", "en-US"]).await;
    app.expect(by_text("Olá, Ana!")).to_be_visible().await;
    app.expect(by_role(Role::Button, "Salvar")).to_be_visible().await;
    assert_eq!(app.ui().language().to_string(), "pt-BR");
}

/// A language the app lacks falls back to a close one, else to the
/// fallback; a message missing from a language comes from the next.
#[mitsuami_test::test]
async fn languages_the_app_lacks_fall_back(app: TestApp) {
    app.set_locales(app_locales());
    app.mount(|| Column::new().children((Text::new(t!("save")), Text::new(t!("only-in-english")))));

    app.set_languages(&["de-DE"]).await;
    app.expect(by_text("Save")).to_be_visible().await;

    // Portugal's Portuguese reads Brazil's rather than English.
    app.set_languages(&["pt-PT"]).await;
    app.expect(by_text("Salvar")).to_be_visible().await;
    app.expect(by_text("Only in English")).to_be_visible().await;
}

/// Each language picks its own plural form: English has two, Arabic six.
#[mitsuami_test::test]
async fn plurals_follow_each_languages_rules(app: TestApp) {
    app.set_locales(app_locales());
    let count = signal(1);
    app.mount(move || Text::new(t!("files", count = count)).test_id("files"));
    let text = |app: &TestApp| app.get_by_test_id("files").text().unwrap();
    assert_eq!(text(&app), "1 file");
    count.set(5);
    app.settle().await;
    assert_eq!(text(&app), "5 files");

    app.set_languages(&["ar"]).await;
    for (n, expected) in
        [(0, "لا ملفات"), (1, "ملف واحد"), (2, "ملفان"), (3, "3 ملفات"), (11, "11 ملفًا"), (100, "100 ملف")]
    {
        count.set(n);
        app.settle().await;
        assert_eq!(text(&app), expected, "{n} in Arabic");
    }
}

/// A message's attribute is `id.attribute`.
#[mitsuami_test::test]
async fn attributes_are_messages_too(app: TestApp) {
    app.set_locales(app_locales());
    app.mount(|| TextInput::new().a11y_label(t!("search")).placeholder(t!("search.placeholder")).test_id("search"));
    let field = app.get_by_test_id("search");
    assert!(field.native_state().props.contains(&Prop::Placeholder("Search files".into())));
    app.set_languages(&["pt-BR"]).await;
    assert!(field.native_state().props.contains(&Prop::Placeholder("Pesquisar arquivos".into())));
}

/// Numbers and dates are written by the platform, as the user's region
/// writes them. Tests fix the region as US English, and dates in UTC; the
/// headless backend and AppKit's formatters agree there, and every
/// backend writes what `format_number` and `format_date_time` do.
#[mitsuami_test::test]
async fn numbers_and_dates_are_the_platforms(app: TestApp) {
    app.set_locales(app_locales());
    app.mount(|| {
        Column::new().children((
            Text::new(t!("size", bytes = 1_234_567)).test_id("size"),
            Text::new(t!("done", fraction = 0.25)).test_id("done"),
            Text::new(t!("price", amount = 9.5)).test_id("price"),
            Text::new(t!("year", year = 2026)).test_id("year"),
            Text::new(t!("modified", date = tuesday())).test_id("modified"),
            Text::new(t!("modified-at", date = tuesday())).test_id("modified-at"),
        ))
    });
    let text = |id: &str| app.get_by_test_id(id).text().unwrap();
    let number = |value: f64, format: NumberFormat| l10n::format_number(value, &format);
    assert_eq!(text("size"), format!("{} bytes", number(1_234_567.0, NumberFormat::default())));
    assert_eq!(
        text("done"),
        format!("{} done", number(0.25, NumberFormat { style: NumberStyle::Percent, ..Default::default() }))
    );
    assert_eq!(text("year"), "2026", "no grouping when the message says so");
    let long = DateTimeFormat { date: Some(DateTimeStyle::Long), time: None };
    assert_eq!(text("modified"), format!("Modified {}", l10n::format_date_time(tuesday(), &long)));
    if matches!(app.backend_name(), "headless" | "appkit") {
        assert_eq!(text("size"), "1,234,567 bytes");
        assert_eq!(text("done"), "25% done");
        assert_eq!(text("price"), "$9.50");
        assert_eq!(text("modified"), "Modified September 29, 2026");
        assert_eq!(text("modified-at"), "9/29/26, 3:04\u{202f}PM");
    }
}

/// What the headless backend writes, and platforms that give only their
/// symbols build on, rounds and pads as `Intl.NumberFormat("en-US")`
/// does (the expected values are Node's).
#[mitsuami_test::test]
async fn mitsuamis_own_numbers_are_intls(_app: TestApp) {
    let number = |value: f64, format: NumberFormat| l10n::basic_number(value, &format);
    let currency = |code: &str| NumberStyle::Currency { code: code.into(), display: CurrencyDisplay::Symbol };
    assert_eq!(number(1_234_567.891, NumberFormat::default()), "1,234,567.891");
    assert_eq!(number(999.9996, NumberFormat::default()), "1,000");
    assert_eq!(number(0.1, NumberFormat { minimum_significant_digits: Some(1), ..Default::default() }), "0.1");
    assert_eq!(number(0.1, NumberFormat { maximum_fraction_digits: Some(20), ..Default::default() }), "0.1");
    assert_eq!(number(1234.5, NumberFormat { maximum_significant_digits: Some(2), ..Default::default() }), "1,200");
    // Ties go away from zero, not to even.
    assert_eq!(number(0.125, NumberFormat { style: NumberStyle::Percent, ..Default::default() }), "13%");
    assert_eq!(number(0.125, NumberFormat { style: currency("USD"), ..Default::default() }), "$0.13");
    // A currency's own digits, and a maximum below them.
    assert_eq!(number(1000.0, NumberFormat { style: currency("JPY"), ..Default::default() }), "¥1,000");
    assert_eq!(
        number(1.4, NumberFormat { style: currency("USD"), maximum_fraction_digits: Some(0), ..Default::default() }),
        "$1"
    );
}

/// Arguments that are signals are tracked: the text follows them.
#[mitsuami_test::test]
async fn messages_follow_their_arguments(app: TestApp) {
    app.set_locales(app_locales());
    let name = signal("Ana".to_string());
    app.mount(move || Text::new(t!("greeting", name = name)));
    app.expect(by_text("Hello, Ana!")).to_be_visible().await;
    name.set("Bruno".into());
    app.expect(by_text("Hello, Bruno!")).to_be_visible().await;
}

/// Fluent isolates a message's values with invisible marks, so a name
/// written right to left doesn't reorder the sentence around it. Queries
/// and `text()` see the text as people do.
#[mitsuami_test::test]
async fn values_are_isolated_from_the_sentence(app: TestApp) {
    app.set_locales(app_locales());
    app.mount(|| Text::new(t!("greeting", name = "Ana")).test_id("greeting"));
    let a11y = app.a11y_tree();
    let raw = a11y.children.iter().find_map(|n| n.name.clone()).unwrap();
    assert_eq!(raw, "Hello, \u{2068}Ana\u{2069}!");
    assert_eq!(app.get_by_test_id("greeting").text().as_deref(), Some("Hello, Ana!"));
    app.expect(by_text("Hello, Ana!")).to_be_visible().await;
}

/// A message missing from every language shows its id, and fails the
/// test unless the test takes the error.
#[mitsuami_test::test]
async fn missing_messages_show_their_id(app: TestApp) {
    app.set_locales(app_locales());
    app.mount(|| Text::new(t!("no-such-message")));
    app.expect(by_text("no-such-message")).to_be_visible().await;
    let errors = app.take_l10n_errors();
    assert!(errors.iter().any(|e| e.contains("no-such-message")), "{errors:?}");
}

/// A right-to-left language lays the app out right to left: rows run from
/// the right, text starts on the right, and every native widget, and the
/// platform's own chrome, are told.
#[mitsuami_test::test]
async fn right_to_left_languages_mirror_the_app(app: TestApp) {
    app.set_locales(app_locales());
    app.mount(|| {
        Column::new().align(Align::Stretch).children((
            Row::new()
                .gap(Spacing::Md)
                .children((Button::new(t!("save")).test_id("first"), Checkbox::new(t!("remember")).test_id("second"))),
            Text::new(t!("name")).test_id("text"),
        ))
    });
    let x = |id: &str| app.get_by_test_id(id).frame().origin.x;
    assert!(x("first") < x("second"));
    assert!(
        !app.get_by_test_id("first")
            .native_state()
            .props
            .iter()
            .any(|p| matches!(p, Prop::LayoutDirection(d) if *d == LayoutDirection::RightToLeft))
    );

    app.set_languages(&["ar"]).await;
    assert!(x("first") > x("second"), "the row runs from the right");
    for id in ["first", "second", "text"] {
        assert!(
            app.get_by_test_id(id).native_state().props.contains(&Prop::LayoutDirection(LayoutDirection::RightToLeft)),
            "{id} is right to left"
        );
    }
    assert!(app.get_by_test_id("text").native_state().props.contains(&Prop::TextAlign(HorizontalAlign::Right)));
    if app.is_headless() {
        assert_eq!(app.headless().app_locale(), Some(("ar".into(), true)));
    }

    app.set_languages(&["en-US"]).await;
    assert!(x("first") < x("second"), "and back");
    assert!(
        app.get_by_test_id("first").native_state().props.contains(&Prop::LayoutDirection(LayoutDirection::LeftToRight))
    );
}

/// The app can choose its language itself, as an in-app setting does.
#[mitsuami_test::test]
async fn the_app_can_choose_its_language(app: TestApp) {
    app.set_locales(app_locales());
    app.mount(|| {
        Column::new().children((
            Text::new(move || l10n::language().to_string()).test_id("language"),
            Button::new("Português").on_click(|| l10n::set_language(Some("pt-BR"))),
            Button::new("System").on_click(|| l10n::set_language(None)),
        ))
    });
    app.expect(by_test_id("language")).to_have_text("en-US").await;
    app.get_by_role(Role::Button, "Português").click().await;
    app.expect(by_test_id("language")).to_have_text("pt-BR").await;
    assert_eq!(l10n::available_languages().len(), 3);
    app.get_by_role(Role::Button, "System").click().await;
    app.expect(by_test_id("language")).to_have_text("en-US").await;
}

/// Window titles can be messages, and follow the language.
#[mitsuami_test::test]
async fn window_titles_are_messages(app: TestApp) {
    app.set_locales(app_locales());
    app.mount(|| Window::new(t!("window-title")).open(true).content(|| Text::new(t!("save"))));
    assert!(app.window_titled("Documents").is_some());
    app.set_languages(&["pt-BR"]).await;
    assert!(app.window_titled("Documentos").is_some());
}

/// mitsuami's own strings are in the app's language, and an app can give
/// them: here the button of an alert the app gave none.
#[mitsuami_test::test]
async fn mitsumamis_own_strings_follow_the_app(app: TestApp) {
    app.set_locales(app_locales());
    app.mount(|| {
        Button::new("Ask").on_click(|| {
            spawn_local(async {
                alert(Alert::new("Saved")).await;
            });
        })
    });
    app.get_by_role(Role::Button, "Ask").click().await;
    let pending = app.services().take_alert().expect("an alert is showing");
    assert_eq!(pending.request.effective_buttons(), ["OK"]);
    pending.respond(0);

    app.set_languages(&["pt-BR"]).await;
    app.get_by_role(Role::Button, "Ask").click().await;
    let pending = app.services().take_alert().expect("an alert is showing");
    assert_eq!(pending.request.effective_buttons(), ["Certo"]);
    pending.respond(0);
}

mitsuami_test::main!();
