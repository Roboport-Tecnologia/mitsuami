//! The built-in widgets as stories: captured on each platform at each size,
//! in light and dark, and compared with their baselines. Heights fit the
//! content, since control heights differ per platform.

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// Every role, bordered and borderless, and disabled. Default buttons are
/// the accent colour on AppKit, GTK and WinUI, and highlighted on Qt;
/// cancel buttons look normal; only GTK draws destructive buttons.
#[mitsuami_test::story(sizes = [(420, fit)])]
fn buttons() -> impl View {
    let roles = [
        ("Normal", ButtonRole::Normal),
        ("Default", ButtonRole::Default),
        ("Cancel", ButtonRole::Cancel),
        ("Destructive", ButtonRole::Destructive),
    ];
    let row = |style: ButtonStyle, enabled: bool| {
        Row::new().gap(8).children(Vec::from(
            roles.map(|(name, role)| Button::new(name).role(role).button_style(style).enabled(enabled)),
        ))
    };
    Column::new().padding(16).gap(8).align(Align::Start).children((
        row(ButtonStyle::Bordered, true),
        row(ButtonStyle::Borderless, true),
        row(ButtonStyle::Bordered, false),
    ))
}

/// A raw platform setting through `.native()`: a large control on AppKit,
/// round ends on GTK and WinUI, a checkable button, checked, on Qt.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn button_tweaked() -> impl View {
    let tweak: Tweak<Button> = platform! {
        macos => mitsuami::appkit::tweak(|b: &mitsuami::appkit::objc2_app_kit::NSButton| {
            b.setControlSize(mitsuami::appkit::objc2_app_kit::NSControlSize::Large)
        }),
        gtk => mitsuami::gtk::tweak(|b: &mitsuami::gtk::gtk::Button| {
            use mitsuami::gtk::gtk::prelude::*;
            b.add_css_class("circular")
        }),
        kde => mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| {
            b.set_bool("checkable", true);
            b.set_bool("checked", true);
        }),
        windows => mitsuami::winui::tweak(|b: &mitsuami::winui::bindings::Button| {
            use mitsuami::winui::bindings::{CornerRadius, IControl};
            use mitsuami::winui::windows_core::Interface;
            let round = CornerRadius { top_left: 16.0, top_right: 16.0, bottom_right: 16.0, bottom_left: 16.0 };
            b.cast::<IControl>()?.SetCornerRadius(round)
        }),
    };
    Row::new()
        .padding(16)
        .gap(8)
        .align(Align::Start)
        .children((Button::new("Plain"), Button::new("Tweaked").native(tweak)))
}

#[mitsuami_test::story(sizes = [(240, fit)])]
fn text_styles() -> impl View {
    Column::new().padding(16).gap(4).children((
        Text::new("Large title").text_style(TextStyle::LargeTitle),
        Text::new("Title").text_style(TextStyle::Title),
        Text::new("Headline").text_style(TextStyle::Headline),
        Text::new("Body"),
        Text::new("Callout").text_style(TextStyle::Callout),
        Text::new("Caption").text_style(TextStyle::Caption),
        Text::new("Monospace").text_style(TextStyle::Monospace),
    ))
}

#[mitsuami_test::story(sizes = [(200, fit)])]
fn toggles() -> impl View {
    Column::new().padding(16).gap(8).align(Align::Start).children((
        Checkbox::new("Unchecked"),
        Checkbox::new("Checked").checked(true),
        Checkbox::new("Disabled").enabled(false),
        Switch::new("Off"),
        Switch::new("On").checked(true),
    ))
}

/// Every state of a checkbox, enabled and disabled.
#[mitsuami_test::story(sizes = [(360, fit)])]
fn checkboxes() -> impl View {
    let row = |enabled: bool| {
        Row::new().gap(16).children((
            Checkbox::new("Unchecked").enabled(enabled),
            Checkbox::new("Checked").checked(true).enabled(enabled),
            Checkbox::new("Mixed").mixed(true).enabled(enabled),
        ))
    };
    Column::new().padding(16).gap(8).align(Align::Start).children((row(true), row(false)))
}

/// A raw platform setting through `.native()`: the box after its label on
/// AppKit, a round check on GTK and WinUI, more room before the label on
/// Qt.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn checkbox_tweaked() -> impl View {
    Column::new().padding(16).gap(8).align(Align::Start).children((
        Checkbox::new("Plain").checked(true),
        Checkbox::new("Tweaked").checked(true).native(checkbox_tweak()),
    ))
}

fn checkbox_tweak() -> Tweak<Checkbox> {
    platform! {
        macos => mitsuami::appkit::tweak(|b: &mitsuami::appkit::objc2_app_kit::NSButton| {
            b.setImagePosition(mitsuami::appkit::objc2_app_kit::NSCellImagePosition::ImageTrailing)
        }),
        gtk => mitsuami::gtk::tweak(|b: &mitsuami::gtk::gtk::CheckButton| {
            use mitsuami::gtk::gtk::prelude::*;
            b.add_css_class("selection-mode")
        }),
        kde => mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| b.set_real("spacing", 24.0)),
        windows => mitsuami::winui::tweak(|b: &mitsuami::winui::bindings::CheckBox| {
            use mitsuami::winui::bindings::{CornerRadius, IControl};
            use mitsuami::winui::windows_core::Interface;
            let round = CornerRadius { top_left: 10.0, top_right: 10.0, bottom_right: 10.0, bottom_left: 10.0 };
            b.cast::<IControl>()?.SetCornerRadius(round)
        }),
    }
}

/// Sliders and progress bars as wide as the story; without a step, and
/// with one.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn sliders() -> impl View {
    Column::new().padding(16).gap(12).children((
        Slider::new("Volume").value(30.0),
        Slider::new("Rating").range(0.0, 5.0).step(1.0).value(4.0),
        Slider::new("Disabled").value(70.0).enabled(false),
        Progress::new("Upload").value(0.6),
        Progress::new("Done").value(1.0),
    ))
}

/// AppKit sizes pop-up buttons for their widest option, the others for the
/// chosen one.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn selects() -> impl View {
    let sizes = ["Small", "Medium", "Extra large"];
    Column::new().padding(16).gap(8).align(Align::Start).children((
        Select::new("Size").options(sizes),
        Select::new("Size, chosen").options(sizes).selected(2),
        Select::new("Size, disabled").options(sizes).selected(1).enabled(false),
    ))
}

fn signup() -> impl View {
    let agreed = signal(false);
    Column::new().padding(16).gap(8).children((
        TextInput::new().a11y_label("Name").placeholder("Your name"),
        TextInput::new().a11y_label("Email").value("ada@example.com"),
        Checkbox::new("I agree to the terms").bind(agreed),
        Row::new().justify(Justify::End).child(Button::new("Sign up").role(ButtonRole::Default).enabled(agreed)),
    ))
}

/// The form at a phone's width and a window's: inputs stretch, the button
/// stays at the end.
#[mitsuami_test::story(sizes = [(280, fit), (480, fit)], play = agree)]
fn signup_ready() -> impl View {
    signup()
}

async fn agree(app: &TestApp) {
    app.get_by_role(Role::Checkbox, "I agree to the terms").click().await;
    app.expect(by_role(Role::Button, "Sign up")).to_be_enabled().await;
}

#[derive(Clone)]
struct Contact {
    id: u32,
    name: &'static str,
    email: &'static str,
}

const CONTACTS: [Contact; 8] = [
    Contact { id: 1, name: "Ada Lovelace", email: "ada@example.com" },
    Contact { id: 2, name: "Alan Turing", email: "alan@example.com" },
    Contact { id: 3, name: "Grace Hopper", email: "grace@example.com" },
    Contact { id: 4, name: "Edsger Dijkstra", email: "edsger@example.com" },
    Contact { id: 5, name: "Barbara Liskov", email: "barbara@example.com" },
    Contact { id: 6, name: "Donald Knuth", email: "don@example.com" },
    Contact { id: 7, name: "Frances Allen", email: "fran@example.com" },
    Contact { id: 8, name: "John Backus", email: "john@example.com" },
];

/// A list with a selected row, cut off at the bottom: the platform draws the
/// rows' selection and the scroll bar.
#[mitsuami_test::story(sizes = [(280, fit)], play = select_grace)]
fn list() -> impl View {
    let selected = signal(Vec::<u32>::new());
    Column::new().child(
        List::new(
            || CONTACTS.to_vec(),
            |c: &Contact| c.id,
            |c| {
                Column::new()
                    .padding_x(12)
                    .padding_y(6)
                    .children((Text::new(c.name), Text::new(c.email).text_style(TextStyle::Caption)))
            },
        )
        .selected(selected)
        .height(180),
    )
}

async fn select_grace(app: &TestApp) {
    app.get_by_role(Role::ListItem, "Grace Hopper grace@example.com").select().await;
}

mitsuami_test::main!();
