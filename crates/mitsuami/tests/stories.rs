//! The built-in widgets as stories: captured on each platform at each size,
//! in light and dark, and compared with their baselines. Heights fit the
//! content, since control heights differ per platform.

use mitsuami::prelude::*;
use mitsuami_test::prelude::*;

/// Every role, bordered and borderless, and disabled. Default buttons are
/// the accent colour on AppKit, GTK and WinUI, and tinted on Qt (not
/// borderless ones);
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
/// round ends on GTK and WinUI, more padding on Qt.
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
        kde => mitsuami::kirigami::tweak(|b: &mitsuami::kirigami::QmlObject| b.set_real("padding", 16.0)),
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

/// Colours, weights, italics and alignment, each the platform's own.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn text_options() -> impl View {
    let colors = Column::new().gap(4).children((
        Text::new("Label"),
        Text::new("Secondary label").color(Color::SecondaryLabel),
        Text::new("Accent").color(Color::Accent),
        Text::new("Error").color(Color::Error),
        Text::new("Warning").color(Color::Warning),
        Text::new("Success").color(Color::Success),
        Text::new("Fixed purple").color(Color::rgb(0x80, 0x40, 0xc0)),
    ));
    let fonts = Column::new().gap(4).children((
        Text::new("Medium").weight(FontWeight::Medium),
        Text::new("Semibold").weight(FontWeight::Semibold),
        Text::new("Bold").weight(FontWeight::Bold),
        Text::new("Italic").italic(true),
        Text::new("Bold italic").weight(FontWeight::Bold).italic(true),
        Text::new("Centred").text_align(TextAlign::Center),
        Text::new("End").text_align(TextAlign::End),
        // Drawn as a label on every platform.
        Text::new("Selectable").selectable(true),
    ));
    Column::new().padding(16).gap(4).children((colors, fonts))
}

/// A paragraph cut off at two lines and at one, with the platform's
/// ellipsis, and a tweaked label: underlined on GTK, Markdown on Qt, spread
/// letters on WinUI. AppKit's tweak shows the whole text in a tooltip,
/// which a still capture doesn't show.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn text_tweaked() -> impl View {
    let paragraph = "Widgets behave, size, animate and respond as the platform's own controls do, \
                     and text wraps as the platform wraps it.";
    Column::new().padding(16).gap(8).children((
        Text::new(paragraph).max_lines(2),
        Text::new(paragraph).max_lines(1),
        // Markup only where the tweak renders it; cut off where it has a
        // tooltip.
        Text::new(platform! {
            kde => "Tweaked, with **some** of it _marked up_",
            macos => "Tweaked: a line too long for the story, whole in its tooltip",
            _ => "Tweaked",
        })
        .max_lines(1)
        .native(text_tweak()),
    ))
}

fn text_tweak() -> Tweak<Text> {
    platform! {
        macos => mitsuami::appkit::tweak(|t: &mitsuami::appkit::objc2_app_kit::NSTextField| t.setAllowsExpansionToolTips(true)),
        gtk => mitsuami::gtk::tweak(|l: &mitsuami::gtk::gtk::Label| {
            use mitsuami::gtk::gtk::pango::{AttrInt, AttrList, Underline};
            let attributes = AttrList::new();
            attributes.insert(AttrInt::new_underline(Underline::Single));
            l.set_attributes(Some(&attributes))
        }),
        kde => mitsuami::kirigami::tweak(|l: &mitsuami::kirigami::QmlObject| l.set_int("textFormat", 3)),
        windows => mitsuami::winui::tweak(|t: &mitsuami::winui::bindings::TextBlock| {
            use mitsuami::winui::windows_core::Interface;
            t.cast::<mitsuami::winui::bindings::ITextBlock>()?.SetCharacterSpacing(200)
        }),
    }
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

/// Toggle buttons off and on, bordered and borderless, with an icon, and
/// disabled: pressed ones are a darker bezel on AppKit, the accent colour
/// on WinUI, and shown pressed on GTK and Qt.
#[mitsuami_test::story(sizes = [(420, fit)])]
fn toggle_buttons() -> impl View {
    let bold = platform! {
        macos => "bold",
        gtk => "format-text-bold-symbolic",
        kde => "format-text-bold",
        windows => "\u{E8DD}",
        _ => "bold",
    };
    let row = |style: ButtonStyle, enabled: bool| {
        Row::new().gap(8).children((
            ToggleButton::new("Off").button_style(style).enabled(enabled),
            ToggleButton::new("On").checked(true).button_style(style).enabled(enabled),
            ToggleButton::new("Bold").icon(bold).icon_only(true).checked(true).button_style(style).enabled(enabled),
        ))
    };
    Column::new().padding(16).gap(8).align(Align::Start).children((
        row(ButtonStyle::Bordered, true),
        row(ButtonStyle::Borderless, true),
        row(ButtonStyle::Bordered, false),
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

/// A raw platform setting through `.native()`: a small switch on AppKit,
/// its own text on Qt, on and off text on WinUI. GTK's tweak delays the
/// switch's state, which a still capture doesn't show.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn switch_tweaked() -> impl View {
    Column::new()
        .padding(16)
        .gap(8)
        .align(Align::Start)
        .children((Switch::new("Plain").checked(true), Switch::new("Tweaked").checked(true).native(switch_tweak())))
}

fn switch_tweak() -> Tweak<Switch> {
    platform! {
        macos => mitsuami::appkit::tweak(|s: &mitsuami::appkit::objc2_app_kit::NSSwitch| {
            s.setControlSize(mitsuami::appkit::objc2_app_kit::NSControlSize::Small)
        }),
        gtk => mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::Switch| {
            use mitsuami::gtk::gtk::{glib, prelude::*};
            // Once: a tweak runs again whenever the props change.
            if s.widget_name() != "delayed" {
                s.set_widget_name("delayed");
                s.connect_state_set(|s, on| {
                    let s = s.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_secs(1), move || s.set_state(on));
                    glib::Propagation::Stop
                });
            }
        }),
        kde => mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| s.set_str("text", "Tweaked")),
        windows => mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::ToggleSwitch| {
            use mitsuami::winui::bindings::{IToggleSwitch, PropertyValue};
            use mitsuami::winui::windows_core::Interface;
            let s = s.cast::<IToggleSwitch>()?;
            s.SetOnContent(&PropertyValue::CreateString("On")?)?;
            s.SetOffContent(&PropertyValue::CreateString("Off")?)
        }),
    }
}

/// A raw platform setting through `.native()`: a borderless pop-up on
/// AppKit, a flat combo box on Qt, a header on WinUI. GTK's tweak turns on
/// search, which shows only in the open pop-up.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn select_tweaked() -> impl View {
    let sizes = ["Small", "Medium", "Large"];
    Column::new().padding(16).gap(8).align(Align::Start).children((
        Select::new("Plain").options(sizes).selected(1),
        Select::new("Tweaked").options(sizes).selected(1).native(select_tweak()),
    ))
}

fn select_tweak() -> Tweak<Select> {
    platform! {
        macos => mitsuami::appkit::tweak(|p: &mitsuami::appkit::objc2_app_kit::NSPopUpButton| p.setBordered(false)),
        gtk => mitsuami::gtk::tweak(|d: &mitsuami::gtk::gtk::DropDown| d.set_enable_search(true)),
        kde => mitsuami::kirigami::tweak(|c: &mitsuami::kirigami::QmlObject| c.set_bool("flat", true)),
        windows => mitsuami::winui::tweak(|c: &mitsuami::winui::bindings::ComboBox| {
            use mitsuami::winui::bindings::{IComboBox, PropertyValue};
            use mitsuami::winui::windows_core::Interface;
            c.cast::<IComboBox>()?.SetHeader(&PropertyValue::CreateString("Size")?)
        }),
    }
}

/// Vertical sliders, as tall as the story makes them: up is more on every
/// platform.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn vertical_sliders() -> impl View {
    let slider = |name: &str, value: f64| Slider::new(name).orientation(Orientation::Vertical).value(value);
    Row::new().padding(16).gap(24).height(180).children((
        slider("Bass", 20.0),
        slider("Mid", 50.0),
        slider("Treble", 80.0),
        slider("Off", 50.0).enabled(false),
    ))
}

/// A raw platform setting through `.native()`: a circular slider on AppKit,
/// the value drawn beside the scale on GTK, tick marks on WinUI. Qt's tweak
/// snaps while dragging, which a still capture doesn't show.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn slider_tweaked() -> impl View {
    Column::new().padding(16).gap(12).children((
        Slider::new("Plain").value(40.0),
        Row::new().child(Slider::new("Tweaked").range(0.0, 10.0).step(1.0).value(4.0).native(slider_tweak())),
    ))
}

fn slider_tweak() -> Tweak<Slider> {
    platform! {
        macos => mitsuami::appkit::tweak(|s: &mitsuami::appkit::objc2_app_kit::NSSlider| {
            s.setSliderType(mitsuami::appkit::objc2_app_kit::NSSliderType::Circular)
        }),
        gtk => mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::Scale| {
            use mitsuami::gtk::gtk::prelude::*;
            s.set_draw_value(true)
        }),
        kde => mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| s.set_int("snapMode", 1)),
        windows => mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::Slider| {
            use mitsuami::winui::bindings::{ISlider, TickPlacement};
            use mitsuami::winui::windows_core::Interface;
            let s = s.cast::<ISlider>()?;
            s.SetTickFrequency(1.0)?;
            s.SetTickPlacement(TickPlacement::Outside)
        }),
    }
}

/// A raw platform setting through `.native()`: a small bar on AppKit, the
/// percentage on GTK, the palette's highlight on Qt, the paused state on
/// WinUI.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn progress_tweaked() -> impl View {
    Column::new()
        .padding(16)
        .gap(12)
        .children((Progress::new("Plain").value(0.6), Progress::new("Tweaked").value(0.6).native(progress_tweak())))
}

fn progress_tweak() -> Tweak<Progress> {
    platform! {
        macos => mitsuami::appkit::tweak(|p: &mitsuami::appkit::objc2_app_kit::NSProgressIndicator| {
            p.setControlSize(mitsuami::appkit::objc2_app_kit::NSControlSize::Small)
        }),
        gtk => mitsuami::gtk::tweak(|p: &mitsuami::gtk::gtk::ProgressBar| p.set_show_text(true)),
        kde => mitsuami::kirigami::tweak(|p: &mitsuami::kirigami::QmlObject| {
            if let Some(palette) = p.object("palette") {
                palette.set_str("highlight", "#8e44ad");
            }
        }),
        windows => mitsuami::winui::tweak(|p: &mitsuami::winui::bindings::ProgressBar| {
            use mitsuami::winui::windows_core::Interface;
            p.cast::<mitsuami::winui::bindings::IProgressBar>()?.SetShowPaused(true)
        }),
    }
}

/// Spinners running beside text, and one stopped: it shows nothing but
/// keeps its place. The tweaked one: small on AppKit, larger on GTK and
/// Qt, a determinate ring on WinUI.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn spinners() -> impl View {
    let row = |spinner: Spinner, text: &str| {
        Row::new().gap(8).align(Align::Center).children((spinner, Text::new(text.to_string())))
    };
    Column::new().padding(16).gap(12).children((
        row(Spinner::new("Loading"), "Loading"),
        row(Spinner::new("Stopped").running(false), "Stopped"),
        row(Spinner::new("Tweaked").native(spinner_tweak()), "Tweaked"),
    ))
}

fn spinner_tweak() -> Tweak<Spinner> {
    platform! {
        macos => mitsuami::appkit::tweak(|s: &mitsuami::appkit::objc2_app_kit::NSProgressIndicator| {
            s.setControlSize(mitsuami::appkit::objc2_app_kit::NSControlSize::Small)
        }),
        gtk => mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::Spinner| {
            use mitsuami::gtk::gtk::prelude::*;
            s.set_size_request(32, 32)
        }),
        kde => mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| {
            s.set_real("implicitWidth", 48.0);
            s.set_real("implicitHeight", 48.0);
        }),
        windows => mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::ProgressRing| {
            use mitsuami::winui::windows_core::Interface;
            let ring = s.cast::<mitsuami::winui::bindings::IProgressRing>()?;
            ring.SetIsIndeterminate(false)?;
            ring.SetValue(60.0)
        }),
    }
}

/// Separators across a column, between groups, and down a row between
/// buttons, as thick as each platform draws them.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn separators() -> impl View {
    Column::new().padding(16).gap(12).children((
        Text::new("General"),
        Separator::new(),
        Text::new("Advanced"),
        Row::new().gap(8).children((
            Button::new("Cut"),
            Button::new("Copy"),
            Separator::vertical(),
            Button::new("Undo"),
        )),
    ))
}

/// Every state of a text field: empty with a placeholder, with text,
/// read-only (drawn as usual on every platform), disabled.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn text_inputs() -> impl View {
    Column::new().padding(16).gap(8).children((
        TextInput::new().a11y_label("Empty").placeholder("Your name"),
        TextInput::new().a11y_label("With text").value("Ada Lovelace"),
        TextInput::new().a11y_label("Read-only").value("ABCD-1234").read_only(true),
        TextInput::new().a11y_label("Disabled").value("Ada Lovelace").enabled(false),
    ))
}

/// A raw platform setting through `.native()`: no border on AppKit, a
/// search icon on GTK, a header on WinUI. Qt's tweak limits the length,
/// which a still capture doesn't show.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn text_input_tweaked() -> impl View {
    Column::new().padding(16).gap(8).children((
        TextInput::new().a11y_label("Plain").value("Ada"),
        TextInput::new().a11y_label("Tweaked").value("Ada").native(text_input_tweak()),
    ))
}

fn text_input_tweak() -> Tweak<TextInput> {
    platform! {
        macos => mitsuami::appkit::tweak(|f: &mitsuami::appkit::objc2_app_kit::NSTextField| f.setBezeled(false)),
        gtk => mitsuami::gtk::tweak(|e: &mitsuami::gtk::gtk::Entry| {
            use mitsuami::gtk::gtk::prelude::EntryExt;
            e.set_icon_from_icon_name(mitsuami::gtk::gtk::EntryIconPosition::Primary, Some("system-search-symbolic"))
        }),
        kde => mitsuami::kirigami::tweak(|f: &mitsuami::kirigami::QmlObject| f.set_int("maximumLength", 8)),
        windows => mitsuami::winui::tweak(|f: &mitsuami::winui::bindings::TextBox| {
            use mitsuami::winui::bindings::{ITextBox, PropertyValue};
            use mitsuami::winui::windows_core::Interface;
            f.cast::<ITextBox>()?.SetHeader(&PropertyValue::CreateString("Name")?)
        }),
    }
}

/// Password fields: empty with a placeholder, filled (hidden as each
/// platform hides it), disabled, and tweaked: no bullets on AppKit, the
/// peek icon on GTK, the password shown on Qt, asterisks on WinUI.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn password_inputs() -> impl View {
    Column::new().padding(16).gap(8).children((
        PasswordInput::new().a11y_label("Empty").placeholder("Password"),
        PasswordInput::new().a11y_label("Filled").value("correct horse"),
        PasswordInput::new().a11y_label("Disabled").value("correct horse").enabled(false),
        PasswordInput::new().a11y_label("Tweaked").value("correct horse").native(password_input_tweak()),
    ))
}

fn password_input_tweak() -> Tweak<PasswordInput> {
    platform! {
        macos => mitsuami::appkit::tweak(|f: &mitsuami::appkit::objc2_app_kit::NSSecureTextField| {
            use mitsuami::appkit::objc2_app_kit::NSSecureTextFieldCell;
            if let Some(cell) = f.cell().and_then(|c| c.downcast::<NSSecureTextFieldCell>().ok()) {
                cell.setEchosBullets(false);
            }
        }),
        gtk => mitsuami::gtk::tweak(|e: &mitsuami::gtk::gtk::PasswordEntry| e.set_show_peek_icon(true)),
        kde => mitsuami::kirigami::tweak(|f: &mitsuami::kirigami::QmlObject| f.set_bool("showPassword", true)),
        windows => mitsuami::winui::tweak(|f: &mitsuami::winui::bindings::PasswordBox| {
            use mitsuami::winui::windows_core::Interface;
            f.cast::<mitsuami::winui::bindings::IPasswordBox>()?.SetPasswordChar("*")
        }),
    }
}

/// Search fields: empty with the platform's placeholder, with the app's,
/// filled (with the clear button), disabled, and tweaked: searching on
/// every keystroke on AppKit, a longer search delay on GTK and Qt (neither
/// shows), a header on WinUI.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn search_inputs() -> impl View {
    Column::new().padding(16).gap(8).children((
        SearchInput::new().a11y_label("Empty"),
        SearchInput::new().a11y_label("Placeholder").placeholder("Find in page"),
        SearchInput::new().a11y_label("Filled").value("apricot"),
        SearchInput::new().a11y_label("Disabled").value("apricot").enabled(false),
        SearchInput::new().a11y_label("Tweaked").value("apricot").native(search_input_tweak()),
    ))
}

fn search_input_tweak() -> Tweak<SearchInput> {
    platform! {
        macos => mitsuami::appkit::tweak(|f: &mitsuami::appkit::objc2_app_kit::NSSearchField| {
            f.setSendsSearchStringImmediately(true)
        }),
        gtk => mitsuami::gtk::tweak(|e: &mitsuami::gtk::gtk::SearchEntry| e.set_search_delay(1000)),
        kde => mitsuami::kirigami::tweak(|f: &mitsuami::kirigami::QmlObject| f.set_bool("delaySearch", true)),
        windows => mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::AutoSuggestBox| {
            use mitsuami::winui::bindings::{IAutoSuggestBox, PropertyValue};
            use mitsuami::winui::windows_core::Interface;
            s.cast::<IAutoSuggestBox>()?.SetHeader(&PropertyValue::CreateString("Find in mail")?)
        }),
    }
}

/// Text areas: empty with a placeholder (shown on Qt and WinUI), filled
/// with a line that wraps, more lines than fit, read-only, disabled, and
/// tweaked: a fixed-pitch font on AppKit and GTK, lines broken inside words
/// on Qt, a header on WinUI.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn text_areas() -> impl View {
    const NOTE: &str = "A note that runs past the edge of the area, and wraps.\nAnd a second paragraph.";
    let many = (1..=8).map(|n| format!("Line {n}")).collect::<Vec<_>>().join("\n");
    Column::new().padding(16).gap(8).children((
        TextArea::new().a11y_label("Empty").placeholder("Comments"),
        TextArea::new().a11y_label("Filled").value(NOTE),
        TextArea::new().a11y_label("Scrolled").lines(2).value(many),
        TextArea::new().a11y_label("Unwrapped").lines(2).value(NOTE).line_wrap(false),
        TextArea::new().a11y_label("Read-only").value(NOTE).read_only(true),
        TextArea::new().a11y_label("Disabled").value(NOTE).enabled(false),
        TextArea::new().a11y_label("Tweaked").value(NOTE).native(text_area_tweak()),
    ))
}

fn text_area_tweak() -> Tweak<TextArea> {
    platform! {
        macos => mitsuami::appkit::tweak(|t: &mitsuami::appkit::objc2_app_kit::NSTextView| {
            use mitsuami::appkit::objc2_app_kit::NSFont;
            t.setFont(NSFont::userFixedPitchFontOfSize(0.0).as_deref())
        }),
        gtk => mitsuami::gtk::tweak(|v: &mitsuami::gtk::gtk::TextView| {
            use mitsuami::gtk::gtk::prelude::*;
            v.set_monospace(true)
        }),
        // TextEdit.WrapAnywhere.
        kde => mitsuami::kirigami::tweak(|a: &mitsuami::kirigami::QmlObject| a.set_int("wrapMode", 3)),
        windows => mitsuami::winui::tweak(|t: &mitsuami::winui::bindings::TextBox| {
            use mitsuami::winui::bindings::{ITextBox, PropertyValue};
            use mitsuami::winui::windows_core::Interface;
            t.cast::<ITextBox>()?.SetHeader(&PropertyValue::CreateString("Note")?)
        }),
    }
}

/// Scroll views over more rows than fit: as the platform shows them,
/// without scroll bars, and tweaked: a bezel border on AppKit, classic
/// scroll bars on GTK. Qt's and WinUI's tweaks change how it scrolls, which
/// a still capture doesn't show. Overlay scroll bars (macOS, GTK, WinUI)
/// show only while scrolling.
#[mitsuami_test::story(sizes = [(360, fit)])]
fn scroll_views() -> impl View {
    let rows = || Column::new().gap(4).children((1..=12).map(|i| Text::new(format!("Row {i}"))).collect::<Vec<_>>());
    Row::new().padding(16).gap(12).children((
        ScrollView::new().height(96).grow(1.0).child(rows()),
        ScrollView::new().scroll_bars(false).height(96).grow(1.0).child(rows()),
        ScrollView::new().native(scroll_view_tweak()).height(96).grow(1.0).child(rows()),
    ))
}

fn scroll_view_tweak() -> Tweak<ScrollView> {
    platform! {
        macos => mitsuami::appkit::tweak(|s: &mitsuami::appkit::objc2_app_kit::NSScrollView| {
            s.setBorderType(mitsuami::appkit::objc2_app_kit::NSBorderType::BezelBorder)
        }),
        gtk => mitsuami::gtk::tweak(|s: &mitsuami::gtk::gtk::ScrolledWindow| s.set_overlay_scrolling(false)),
        kde => mitsuami::kirigami::tweak(|s: &mitsuami::kirigami::QmlObject| {
            if let Some(wheel) = s.find("scrollFlickableTarget", "true") {
                wheel.set_real("verticalStepSize", 20.0);
            }
        }),
        windows => mitsuami::winui::tweak(|s: &mitsuami::winui::bindings::ScrollViewer| {
            use mitsuami::winui::windows_core::Interface;
            s.cast::<mitsuami::winui::bindings::IScrollViewer>()?.SetIsScrollInertiaEnabled(false)
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

/// A checkerboard of 8-pixel squares, in two colours.
fn checkerboard(width: u32, height: u32, a: [u8; 4], b: [u8; 4]) -> Pixels {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(if (x / 8 + y / 8).is_multiple_of(2) { &a } else { &b });
        }
    }
    Pixels::new(width, height, rgba)
}

/// Images at their own size, and fitted to a frame of another shape
/// (wide, so the difference shows): each platform's drawing, from pixels
/// and from a file.
#[mitsuami_test::story(sizes = [(320, fit)])]
fn images() -> impl View {
    let board = || checkerboard(48, 48, [40, 90, 200, 255], [240, 240, 240, 255]);
    let file = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/assets/blue-red-20x10.png");
    Column::new().padding(16).gap(8).align(Align::Start).children((
        Row::new().gap(8).children((Image::pixels(board()).label("Pixels"), Image::file(file).label("File"))),
        Row::new().gap(8).children((
            Image::pixels(board()).label("Contain").fit(ImageFit::Contain).width(120).height(48),
            Image::pixels(board()).label("Stretch").fit(ImageFit::Stretch).width(120).height(48),
        )),
    ))
}

/// Icons from each platform's own set, at its own size and larger, in the
/// platform's colours, and buttons with an icon before their caption or
/// alone. The names differ
/// per platform; the shapes are each set's.
#[mitsuami_test::story(sizes = [(320, fit)])]
fn icons() -> impl View {
    let (disc, folder, trash) = platform! {
        macos => ("opticaldisc", "folder", "trash"),
        gtk => ("media-optical-symbolic", "folder-symbolic", "user-trash-symbolic"),
        kde => ("media-optical", "folder", "edit-delete"),
        windows => ("\u{E958}", "\u{E8B7}", "\u{E74D}"),
        _ => ("disc", "folder", "trash"),
    };
    Column::new().padding(16).gap(12).align(Align::Start).children((
        Row::new().gap(12).align(Align::Center).children((
            Icon::new(disc).label("Disc"),
            Icon::new(folder).label("Folder"),
            Icon::new(trash).label("Trash"),
            Icon::new(disc).label("Large disc").icon_size(32.0),
        )),
        Row::new().gap(12).align(Align::Center).children((
            Icon::new(disc).label("Accent").color(Color::Accent),
            Icon::new(disc).label("Secondary").color(Color::SecondaryLabel),
            Icon::new(disc).label("Error").color(Color::Error),
            Icon::new(disc).label("Warning").color(Color::Warning),
            Icon::new(disc).label("Success").color(Color::Success),
            Icon::new(disc).label("Rgba").color(Color::rgb(0x8e, 0x44, 0xad)),
        )),
        Row::new().gap(8).align(Align::Center).children((
            Button::new("Add folder").icon(folder),
            Button::new("Delete").icon(trash).icon_only(true),
            Button::new("Remove").icon(trash).icon_only(true).button_style(ButtonStyle::Borderless),
        )),
    ))
}

/// Files' icons as each platform's file manager shows them: a folder, a
/// document and a picture at the small size and larger, and the picture's
/// thumbnail where the platform makes one (AppKit and Windows; GTK only
/// if the desktop has one cached, Kirigami never). The files are the
/// crate's own, so every machine has them.
#[mitsuami_test::story(sizes = [(320, fit)])]
fn file_icons() -> impl View {
    let crate_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let (folder, document) = (crate_dir.join("tests/assets"), crate_dir.join("Cargo.toml"));
    let picture = crate_dir.join("tests/assets/blue-red-20x10.png");
    Column::new().padding(16).gap(12).align(Align::Start).children((
        Row::new().gap(12).align(Align::Center).children((
            FileIcon::new(&folder).label("Folder"),
            FileIcon::new(&document).label("Document"),
            FileIcon::new(&picture).label("Picture"),
        )),
        Row::new().gap(12).align(Align::Center).children((
            FileIcon::new(&folder).label("Large folder").icon_size(48.0),
            FileIcon::new(&document).label("Large document").icon_size(48.0),
            FileIcon::new(&picture).label("Large picture").icon_size(48.0),
            FileIcon::new(&picture).label("Thumbnail").icon_size(48.0).thumbnail(true),
        )),
    ))
}

/// Menu buttons as each platform draws them, closed: with a caption, an
/// icon before it, the icon alone, borderless, and disabled. Each draws
/// its own arrow (GNOME's icon-only menu buttons have none).
#[mitsuami_test::story(sizes = [(420, fit)])]
fn menu_buttons() -> impl View {
    let plus = platform! {
        macos => "plus",
        gtk => "list-add-symbolic",
        kde => "list-add",
        windows => "\u{E710}",
        _ => "plus",
    };
    let menu = || (MenuItem::new("Disc image…"), MenuItem::new("Folder…"));
    Column::new().padding(16).gap(12).align(Align::Start).children((
        Row::new().gap(8).align(Align::Center).children((
            MenuButton::new("Add").menu(menu()),
            MenuButton::new("Add").icon(plus).menu(menu()),
            MenuButton::new("Add").icon(plus).icon_only(true).menu(menu()),
        )),
        Row::new().gap(8).align(Align::Center).children((
            MenuButton::new("Add").icon(plus).button_style(ButtonStyle::Borderless).menu(menu()),
            MenuButton::new("Add").enabled(false).menu(menu()),
        )),
    ))
}

/// Groups as each platform draws them: a box with its heading inside on
/// AppKit and Qt, a card under a heading on GNOME and Windows; one
/// without a heading; content laid out inside each platform's margins.
#[mitsuami_test::story(sizes = [(360, fit)])]
fn groups() -> impl View {
    Column::new().padding(16).gap(16).children((
        Group::new().title("CD drive").child(Row::new().gap(8).align(Align::Center).children((
            Column::new().grow(1.0).children((
                Text::new("Total Annihilation (1997)").weight(FontWeight::Semibold),
                Text::new("ISO image").color(Color::SecondaryLabel),
            )),
            Button::new("Eject"),
        ))),
        Group::new().gap(8).children((Checkbox::new("Mount at start"), Checkbox::new("Read only"))),
    ))
}

/// A GPU surface nothing has presented to yet, between two lines: where
/// it sits and how large it is. Captures show what the platform draws
/// before the app presents (captures don't read GPU surfaces' content).
#[mitsuami_test::story(sizes = [(240, fit)])]
fn gpu_surfaces() -> impl View {
    Column::new().padding(16).gap(8).children((
        Text::new("Above"),
        GpuSurface::new().label("Screen").height(96),
        Text::new("Below"),
    ))
}

/// Spin boxes as each platform draws them: a field and a stepper on
/// AppKit, buttons inside the field elsewhere. GTK sizes them for their
/// range's widest number.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn number_inputs() -> impl View {
    Column::new().padding(16).gap(8).align(Align::Start).children((
        NumberInput::new("Copies").range(1, 99).value(2),
        NumberInput::new("Memory").range(16, 4096).step(16).value(512),
        NumberInput::new("Disabled").value(7).enabled(false),
    ))
}

/// 2ksbox's header: a download's progress and a status line, at the
/// trailing end of the window's toolbar. Captures are of the content, so
/// the toolbar shows only in the layout, above it. Wide enough for both
/// items: narrower, AppKit and WinUI move what doesn't fit to an overflow
/// menu.
#[mitsuami_test::story(sizes = [(760, fit)])]
fn toolbar() -> impl View {
    Column::new().padding(16).children((
        Toolbar::new().children((
            Row::new()
                .gap(6)
                .align(Align::Center)
                .children((Spinner::new("Downloading"), Text::new("Downloading presets… 12 MB").max_lines(1))),
            Button::new("New machine"),
        )),
        Text::new("No machines yet."),
    ))
}

/// A settings window's sidebar: items with icons, in sections, the second
/// chosen. Captures are of the content, so the sidebar shows only in the
/// layout, beside it.
#[mitsuami_test::story(sizes = [(420, 280)])]
fn sidebar() -> impl View {
    let page = signal(1);
    // SF Symbols, symbolic theme icons, Segoe Fluent Icons glyphs.
    let [general, appearance, wifi, bluetooth] = platform! {
        macos => ["gearshape", "paintbrush", "wifi", "dot.radiowaves.left.and.right"],
        linux => [
            "preferences-system-symbolic",
            "applications-graphics-symbolic",
            "network-wireless-symbolic",
            "bluetooth-symbolic",
        ],
        windows => ["\u{E713}", "\u{E790}", "\u{E701}", "\u{E702}"],
    };
    Column::new().padding(16).gap(8).children((
        Sidebar::new(page).children((
            SidebarItem::new("General", 0).icon(general),
            SidebarItem::new("Appearance", 1).icon(appearance),
            SidebarSection::new("Network")
                .children((SidebarItem::new("Wi-Fi", 2).icon(wifi), SidebarItem::new("Bluetooth", 3).icon(bluetooth))),
        )),
        Text::new("Appearance").text_style(TextStyle::Title),
        Text::new("Light, dark, or following the time of day."),
    ))
}

/// A settings pane's tabs, the second shown. Each platform draws its own
/// strip of tabs and border; the pages are inside, as big as the biggest.
#[mitsuami_test::story(sizes = [(420, fit)])]
fn tabs() -> impl View {
    settings_tabs(TabsStyle::Automatic)
}

/// The same with a tab bar: Qt's `TabBar` on KDE rather than Kirigami's
/// navigation bar. The others show theirs.
#[mitsuami_test::story(sizes = [(420, fit)])]
fn tabs_tab_bar() -> impl View {
    settings_tabs(TabsStyle::TabBar)
}

/// Tabs with icons, where the platform's tabs show them: libadwaita's view
/// switcher, WinUI's selector bar, Kirigami's navigation bar. AppKit's tab
/// view shows only titles.
#[mitsuami_test::story(sizes = [(420, fit)])]
fn tabs_with_icons() -> impl View {
    let (general, network) = platform! {
        macos => ("gearshape", "network"),
        gtk => ("emblem-system-symbolic", "network-wired-symbolic"),
        kde => ("configure", "network-wired"),
        windows => ("\u{E713}", "\u{E968}"),
    };
    let page = signal(0);
    Column::new().padding(16).child(Tabs::new(page).children((
        Tab::new("General", 0).icon(general).padding(12).child(Text::new("Name")),
        Tab::new("Network", 1).icon(network).padding(12).child(Text::new("Nothing here yet.")),
    )))
}

fn settings_tabs(style: TabsStyle) -> impl View {
    let page = signal(1);
    Column::new().padding(16).child(
        Tabs::new(page).tabs_style(style).children((
            Tab::new("General", 0)
                .padding(12)
                .gap(8)
                .children((Text::new("Name"), TextInput::new().placeholder("Untitled"))),
            Tab::new("Appearance", 1)
                .padding(12)
                .gap(8)
                .children((Checkbox::new("Show icons"), Checkbox::new("Show the status bar"))),
            Tab::new("Advanced", 2).padding(12).child(Text::new("Nothing here yet.")),
        )),
    )
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

/// Down a column, at each platform's spacing: none chosen, one chosen,
/// and disabled.
#[mitsuami_test::story(sizes = [(240, fit)])]
fn radio_groups() -> impl View {
    let sizes = ["Small", "Medium", "Extra large"];
    Column::new().padding(16).gap(16).align(Align::Start).children((
        RadioGroup::new("Size").options(sizes),
        RadioGroup::new("Size, chosen").options(sizes).selected(Some(2)),
        RadioGroup::new("Size, disabled").options(sizes).selected(Some(1)).enabled(false),
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

/// A raw platform setting through `.native()`, on the list view: striped
/// rows on AppKit, separators on GTK. Qt's and WinUI's tweaks change key
/// navigation, which a still capture doesn't show.
#[mitsuami_test::story(sizes = [(280, fit)], play = select_grace)]
fn list_tweaked() -> impl View {
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
        .native(list_tweak())
        .height(180),
    )
}

fn list_tweak() -> Tweak<List> {
    platform! {
        macos => mitsuami::appkit::tweak(|t: &mitsuami::appkit::objc2_app_kit::NSTableView| {
            t.setUsesAlternatingRowBackgroundColors(true)
        }),
        gtk => mitsuami::gtk::tweak(|v: &mitsuami::gtk::gtk::ListView| v.set_show_separators(true)),
        kde => mitsuami::kirigami::tweak(|v: &mitsuami::kirigami::QmlObject| v.set_bool("keyNavigationWraps", true)),
        windows => mitsuami::winui::tweak(|v: &mitsuami::winui::bindings::ListView| {
            use mitsuami::winui::windows_core::Interface;
            v.cast::<mitsuami::winui::bindings::IListViewBase>()?.SetSingleSelectionFollowsFocus(false)
        }),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ContactColumn {
    Name,
    Email,
}

/// A table sorted by name, the other way round, with a selected row: the
/// platform draws the header, its sort indicator, the rows' selection and
/// the scroll bar.
#[mitsuami_test::story(sizes = [(420, fit)], play = select_grace_row)]
fn table() -> impl View {
    let (selected, sort) = (signal(Vec::<u32>::new()), signal(Sort::descending(ContactColumn::Name)));
    let contacts = move || {
        let mut contacts = CONTACTS.to_vec();
        match sort.get().by {
            ContactColumn::Name => contacts.sort_by_key(|c| c.name),
            ContactColumn::Email => contacts.sort_by_key(|c| c.email),
        }
        if sort.get().order == SortOrder::Descending {
            contacts.reverse();
        }
        contacts
    };
    Column::new().child(
        Table::new(contacts, |c: &Contact| c.id)
            .column(TableColumn::new("Name", |c: Contact| Text::new(c.name)).width(150).sort_key(ContactColumn::Name))
            .column(TableColumn::new("Email", |c: Contact| Text::new(c.email)).expand().sort_key(ContactColumn::Email))
            .sort(sort)
            .selected(selected)
            .height(180),
    )
}

async fn select_grace_row(app: &TestApp) {
    app.get_by_role(Role::Cell, "Grace Hopper").select().await;
}

mitsuami_test::main!();
