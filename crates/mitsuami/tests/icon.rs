//! `Icon`: an icon from the platform's own set, by its name there, and
//! buttons that show one before their caption or alone. Names differ per
//! platform, so the tests pick them with `platform!`. Sizes are each
//! platform's own: tests only compare them.

use mitsuami::core::backend::{CaptureError, Image};
use mitsuami::core::{Prop, WidgetKind};
use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

fn trash() -> String {
    platform! {
        macos => "trash",
        gtk => "user-trash-symbolic",
        kde => "edit-delete",
        windows => "\u{E74D}",
        _ => "trash",
    }
    .to_owned()
}

fn folder() -> String {
    platform! {
        macos => "folder",
        gtk => "folder-symbolic",
        kde => "folder",
        windows => "\u{E8B7}",
        _ => "folder",
    }
    .to_owned()
}

fn has(app: &TestApp, query: Query, prop: Prop) -> bool {
    app.get(query).native_state().props.contains(&prop)
}

fn icon(label: &str) -> Query {
    by_role(Role::Image, label)
}

/// A caption long enough that its button is wider than Breeze's minimum
/// (80 points), which a short caption and its icon both fit within.
const LONG_CAPTION: &str = "Delete these files";

#[mitsuami_test::test]
async fn shows_the_named_icon(app: TestApp) {
    app.mount(|| Column::new().align(Align::Start).child(Icon::new(trash()).label("Delete")));

    app.expect(icon("Delete")).to_be_visible().await;
    assert_eq!(app.get(icon("Delete")).native_state().kind, WidgetKind::Icon);
    assert!(has(&app, icon("Delete"), Prop::Icon(trash())));
    assert!(!app.get(icon("Delete")).frame().size.is_empty());
}

/// Unlabelled, it's decorative: still an image, but with no name to read.
/// It never takes focus.
#[mitsuami_test::test]
async fn reads_as_an_image_named_by_its_label(app: TestApp) {
    app.mount(|| Column::new().children((Icon::new(trash()).label("Delete"), Icon::new(folder()), TextInput::new())));

    app.expect(icon("Delete")).to_exist().await;
    assert!(has(&app, icon("Delete"), Prop::Label("Delete".into())));
    let tree = app.ui().a11y_tree(app.window()).expect("a window");
    let names: Vec<_> = tree.walk().into_iter().filter(|n| n.role == Role::Image).map(|n| n.name.clone()).collect();
    assert_eq!(names, [Some("Delete".to_owned()), None]);
    assert_eq!(app.ui().focus_order(app.window()).len(), 1);
}

#[mitsuami_test::test]
async fn follows_a_new_name(app: TestApp) {
    let name = signal(trash());
    app.mount(move || Column::new().align(Align::Start).child(Icon::new(name).label("Icon")));
    app.expect(icon("Icon")).to_be_visible().await;

    name.set(folder());
    app.settle().await;
    assert!(has(&app, icon("Icon"), Prop::Icon(folder())));
    assert!(!app.get(icon("Icon")).frame().size.is_empty());
}

/// A size in points makes it larger or smaller, as each platform sizes
/// its icons (an SF Symbol's shape sets its frame, so only the order is
/// checked).
#[mitsuami_test::test]
async fn is_as_large_as_its_size(app: TestApp) {
    let size = signal(32.0_f32);
    app.mount(move || {
        Row::new().align(Align::Start).children((
            Icon::new(folder()).label("Small").icon_size(12.0),
            Icon::new(folder()).label("Default"),
            Icon::new(folder()).label("Large").icon_size(size),
        ))
    });
    app.expect(icon("Large")).to_be_visible().await;

    let height = |label: &str| app.get(icon(label)).frame().height();
    assert!(height("Small") < height("Default"), "{} < {}", height("Small"), height("Default"));
    assert!(height("Default") < height("Large"), "{} < {}", height("Default"), height("Large"));
    assert!(has(&app, icon("Large"), Prop::IconSize(32.0)));

    size.set(48.0);
    app.settle().await;
    assert!(height("Large") > 32.0 * 1.2, "{}", height("Large"));
    assert!(has(&app, icon("Large"), Prop::IconSize(48.0)));
}

/// A semantic colour is the platform's own, which follows the appearance;
/// `Rgba` is fixed. Both are what the native icon shows.
#[mitsuami_test::test]
async fn takes_the_colour_it_is_given(app: TestApp) {
    let color = signal(Color::Accent);
    app.mount(move || {
        Column::new().align(Align::Start).children((
            Icon::new(trash()).label("Tinted").color(color),
            Icon::new(trash()).label("Error").color(Color::Error),
            Icon::new(trash()).label("Plain"),
        ))
    });
    app.expect(icon("Tinted")).to_be_visible().await;

    assert!(has(&app, icon("Tinted"), Prop::TextColor(Color::Accent)));
    assert!(has(&app, icon("Error"), Prop::TextColor(Color::Error)));
    color.set(Color::SecondaryLabel);
    app.settle().await;
    assert!(has(&app, icon("Tinted"), Prop::TextColor(Color::SecondaryLabel)));
    color.set(Color::rgb(0x33, 0x66, 0x99));
    app.settle().await;
    assert!(has(&app, icon("Tinted"), Prop::TextColor(Color::rgb(0x33, 0x66, 0x99))));
}

/// The glyph is drawn in the colour: red pixels where a red icon is, none
/// where an icon has the platform's own colour.
#[mitsuami_test::test]
async fn draws_in_its_colour(app: TestApp) {
    // Kirigami recolours only an icon's parts in the text colour, and
    // Breeze draws its trash in the negative (red) one: a save icon is
    // all text colour.
    let name = platform! { kde => "document-save".to_owned(), _ => trash() };
    app.mount(move || {
        Row::new().align(Align::Start).padding(10).gap(10).children((
            Icon::new(name.clone()).label("Red").color(Color::rgb(255, 0, 0)).icon_size(32.0),
            Icon::new(name.clone()).label("Plain").icon_size(32.0),
        ))
    });
    app.expect(icon("Plain")).to_be_visible().await;

    let image = match app.ui().capture(app.window()).await {
        Err(CaptureError::Unsupported) => return assert!(app.is_headless(), "only headless may lack capture"),
        Err(e) => panic!("capture failed: {e:?}"),
        Ok(image) => image,
    };
    let red = red_in(&image, app.get(icon("Red")).frame());
    assert!(red > 20, "{red} red pixels in the red icon");
    assert_eq!(red_in(&image, app.get(icon("Plain")).frame()), 0);
}

/// Solid red pixels in a frame of a capture. Antialiased edges blend with
/// the background, so only solid ones count.
fn red_in(image: &Image, frame: Rect) -> usize {
    let scale = image.scale_factor;
    let (x0, y0) = ((frame.x() * scale) as usize, (frame.y() * scale) as usize);
    let (x1, y1) = (((frame.x() + frame.width()) * scale) as usize, ((frame.y() + frame.height()) * scale) as usize);
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (y * image.width as usize + x) * 4))
        .filter(|&i| image.rgba[i] > 200 && image.rgba[i + 1] < 80 && image.rgba[i + 2] < 80)
        .count()
}

/// A button's and a toggle button's icon take the colour they're given,
/// pressed or not, as the bookmarks of a Git client's picker do; their
/// captions keep the platform's.
#[mitsuami_test::test]
async fn button_icons_take_the_colour_they_are_given(app: TestApp) {
    let on = signal(false);
    app.mount(move || {
        Row::new().align(Align::Start).padding(10).gap(10).children((
            Button::new("Red").vector_icon(plus_icon()).icon_only(true).icon_color(Color::rgb(255, 0, 0)),
            ToggleButton::new("Red toggle")
                .vector_icon(plus_icon())
                .icon_only(true)
                .icon_color(Color::rgb(255, 0, 0))
                .bind(on),
            Button::new("Plain").vector_icon(plus_icon()).icon_only(true),
        ))
    });
    let (red, toggle, plain) =
        (by_role(Role::Button, "Red"), by_role(Role::ToggleButton, "Red toggle"), by_role(Role::Button, "Plain"));
    app.expect(plain.clone()).to_be_visible().await;
    assert!(has(&app, red.clone(), Prop::TextColor(Color::rgb(255, 0, 0))));
    assert!(has(&app, toggle.clone(), Prop::TextColor(Color::rgb(255, 0, 0))));

    for pressed in [false, true] {
        on.set(pressed);
        app.settle().await;
        let image = match app.ui().capture(app.window()).await {
            Err(CaptureError::Unsupported) => return assert!(app.is_headless(), "only headless may lack capture"),
            Err(e) => panic!("capture failed: {e:?}"),
            Ok(image) => image,
        };
        for query in [red.clone(), toggle.clone()] {
            let count = red_in(&image, app.get(query.clone()).frame());
            assert!(count > 10, "{count} red pixels in {query:?}, pressed: {pressed}");
        }
        assert_eq!(red_in(&image, app.get(plain.clone()).frame()), 0);
    }
}

/// An icon goes before the caption and makes the button wider; alone,
/// the button is narrower than with its caption, and still reads as it.
#[mitsuami_test::test]
async fn a_button_shows_an_icon_before_its_caption_or_alone(app: TestApp) {
    let clicks = signal(0);
    app.mount(move || {
        Column::new().align(Align::Start).gap(8).children((
            Button::new(LONG_CAPTION).test_id("caption"),
            Button::new(LONG_CAPTION).icon(trash()).test_id("icon and caption"),
            Button::new("Discard").icon(trash()).icon_only(true).on_click(move || clicks.update(|c| *c += 1)),
        ))
    });
    app.expect(by_role(Role::Button, "Discard")).to_be_visible().await;

    // The same caption with and without the icon: captions of the same
    // length aren't as wide in GTK's and Qt's proportional fonts.
    let width = |query: Query| app.get(query).frame().width();
    let (caption, both) = (width(by_test_id("caption")), width(by_test_id("icon and caption")));
    let alone = width(by_role(Role::Button, "Discard"));
    assert!(both > caption, "{both} > {caption}");
    assert!(alone < caption, "{alone} < {caption}");
    assert!(has(&app, by_test_id("icon and caption"), Prop::Icon(trash())));
    assert!(has(&app, by_role(Role::Button, "Discard"), Prop::IconOnly(true)));

    app.get_by_role(Role::Button, "Discard").click().await;
    assert_eq!(clicks.get_untracked(), 1);
}

/// Without a bezel it's still as large as its icon (AppKit measured a
/// borderless trash button smaller than its symbol).
#[mitsuami_test::test]
async fn a_borderless_icon_button_holds_its_icon(app: TestApp) {
    app.mount(|| {
        Row::new().align(Align::Start).children((
            Icon::new(trash()).label("Trash"),
            Button::new("Delete").icon(trash()).icon_only(true).button_style(ButtonStyle::Borderless),
        ))
    });
    app.expect(by_role(Role::Button, "Delete")).to_be_visible().await;

    let icon = app.get(icon("Trash")).frame().size;
    let button = app.get(by_role(Role::Button, "Delete")).frame().size;
    assert!(button.width >= icon.width && button.height >= icon.height, "{button:?} holds {icon:?}");
}

#[mitsuami_test::test]
async fn a_button_follows_its_icon(app: TestApp) {
    let name = signal(String::new());
    let only = signal(false);
    app.mount(move || Column::new().align(Align::Start).child(Button::new(LONG_CAPTION).icon(name).icon_only(only)));
    app.expect(by_role(Role::Button, LONG_CAPTION)).to_be_visible().await;
    let width = || app.get(by_role(Role::Button, LONG_CAPTION)).frame().width();
    let plain = width();

    name.set(trash());
    app.settle().await;
    let with_icon = width();
    assert!(with_icon > plain, "{with_icon} > {plain}");

    only.set(true);
    app.settle().await;
    assert!(width() < plain, "{} < {plain}", width());
    assert!(has(&app, by_role(Role::Button, LONG_CAPTION), Prop::IconOnly(true)));

    only.set(false);
    app.settle().await;
    assert_eq!(width(), with_icon);

    name.set(String::new());
    app.settle().await;
    assert_eq!(width(), plain);
    assert!(has(&app, by_role(Role::Button, LONG_CAPTION), Prop::Label(LONG_CAPTION.into())));
}

/// A new caption keeps an icon shown alone so: AppKit puts a button's
/// image back beside a new title. `view!` sets the caption last.
#[mitsuami_test::test]
async fn a_new_caption_keeps_the_icon_alone(app: TestApp) {
    let caption = signal("Delete".to_string());
    let icon = trash();
    app.mount(move || view! { <Row><Button icon=icon.clone() icon_only=true>{caption}</Button></Row> });
    assert!(has(&app, by_role(Role::Button, "Delete"), Prop::IconOnly(true)));

    caption.set("Discard".into());
    app.settle().await;
    assert!(has(&app, by_role(Role::Button, "Discard"), Prop::IconOnly(true)));
}

// The app's own icons: path data, drawn as the platform draws its
// symbolic icons.

/// A square with a hole: solid enough to count pixels in, and even-odd.
fn frame_icon() -> VectorIcon {
    VectorIcon::with_view_box("F0 M2 2H22V22H2Z M8 8V16H16V8Z", Rect::new(0.0, 0.0, 24.0, 24.0)).unwrap()
}

fn plus_icon() -> VectorIcon {
    VectorIcon::new("M10 0H14V10H24V14H14V24H10V14H0V10H10Z").unwrap()
}

#[mitsuami_test::test]
async fn shows_the_apps_own_icon(app: TestApp) {
    app.mount(|| Column::new().align(Align::Start).child(Icon::new("").vector(frame_icon()).label("Frame")));

    app.expect(icon("Frame")).to_be_visible().await;
    assert!(has(&app, icon("Frame"), Prop::VectorIcon(Some(frame_icon()))));
    assert!(!app.get(icon("Frame")).frame().size.is_empty());
}

/// It takes the size a named icon takes, and is as large as one by
/// default.
#[mitsuami_test::test]
async fn the_apps_own_icon_is_as_large_as_its_size(app: TestApp) {
    app.mount(|| {
        Row::new().align(Align::Start).gap(8).children((
            Icon::new(trash()).label("Named"),
            Icon::new("").vector(plus_icon()).label("Own"),
            Icon::new("").vector(plus_icon()).label("Large").icon_size(48.0),
        ))
    });
    app.expect(icon("Large")).to_be_visible().await;

    let size = |label| app.get(icon(label)).frame().size;
    let (named, own, large) = (size("Named"), size("Own"), size("Large"));
    assert!(large.width > own.width && large.height > own.height, "{large:?} > {own:?}");
    // Symbols' frames follow their shapes on AppKit: alike, not equal.
    assert!((own.height - named.height).abs() <= 4.0, "{own:?} is about {named:?}");
}

#[mitsuami_test::test]
async fn the_apps_own_icon_draws_in_its_colour(app: TestApp) {
    app.mount(move || {
        Row::new().align(Align::Start).padding(10).gap(10).children((
            Icon::new("").vector(frame_icon()).label("Red").color(Color::rgb(255, 0, 0)).icon_size(32.0),
            Icon::new("").vector(frame_icon()).label("Plain").icon_size(32.0),
        ))
    });
    app.expect(icon("Plain")).to_be_visible().await;

    let image = match app.ui().capture(app.window()).await {
        Err(CaptureError::Unsupported) => return assert!(app.is_headless(), "only headless may lack capture"),
        Err(e) => panic!("capture failed: {e:?}"),
        Ok(image) => image,
    };
    let red_in = |frame: Rect| {
        let scale = image.scale_factor;
        let (x0, y0) = ((frame.x() * scale) as usize, (frame.y() * scale) as usize);
        let (x1, y1) =
            (((frame.x() + frame.width()) * scale) as usize, ((frame.y() + frame.height()) * scale) as usize);
        (y0..y1)
            .flat_map(|y| (x0..x1).map(move |x| (y * image.width as usize + x) * 4))
            .filter(|&i| image.rgba[i] > 200 && image.rgba[i + 1] < 80 && image.rgba[i + 2] < 80)
            .count()
    };
    let red = red_in(app.get(icon("Red")).frame());
    assert!(red > 100, "{red} red pixels in the red icon");
    assert_eq!(red_in(app.get(icon("Plain")).frame()), 0);
}

#[mitsuami_test::test]
async fn a_button_shows_the_apps_own_icon(app: TestApp) {
    let clicks = signal(0);
    app.mount(move || {
        Column::new().align(Align::Start).gap(8).children((
            Button::new(LONG_CAPTION).test_id("caption"),
            Button::new(LONG_CAPTION).vector_icon(plus_icon()).test_id("icon and caption"),
            Button::new("Add")
                .vector_icon(plus_icon())
                .icon_only(true)
                .button_style(ButtonStyle::Borderless)
                .on_click(move || clicks.update(|c| *c += 1)),
        ))
    });
    app.expect(by_role(Role::Button, "Add")).to_be_visible().await;

    let width = |query: Query| app.get(query).frame().width();
    let (caption, both) = (width(by_test_id("caption")), width(by_test_id("icon and caption")));
    assert!(both > caption, "{both} > {caption}");
    assert!(width(by_role(Role::Button, "Add")) < caption);
    assert!(has(&app, by_test_id("icon and caption"), Prop::VectorIcon(Some(plus_icon()))));

    app.get_by_role(Role::Button, "Add").click().await;
    assert_eq!(clicks.get_untracked(), 1);
}

mitsuami_test::main!();
