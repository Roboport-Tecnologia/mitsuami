//! Text: style classes, Pango attributes, label and icon colours, text areas.

use std::cell::RefCell;

use gtk::pango;
use gtk::prelude::*;
use mitsuami_core::{Color, FontWeight, TextStyle};

/// GNOME's type scale, as style classes of the theme. There is no callout
/// size in GNOME, so callouts use the body size.
pub(super) fn text_style_class(style: TextStyle) -> Option<&'static str> {
    match style {
        TextStyle::LargeTitle => Some("title-1"),
        TextStyle::Title => Some("title-2"),
        TextStyle::Headline => Some("heading"),
        TextStyle::Body | TextStyle::Callout => None,
        TextStyle::Caption => Some("caption"),
        TextStyle::Monospace => Some("monospace"),
    }
}

pub(super) const TEXT_STYLE_CLASSES: [&str; 5] = ["title-1", "title-2", "heading", "caption", "monospace"];

/// Text colours the theme has a class for, so they follow it (light, dark,
/// high contrast, the accent). The rest are Pango attributes.
pub(super) fn color_class(color: Color) -> Option<&'static str> {
    match color {
        Color::SecondaryLabel => Some("dim-label"),
        Color::Accent => Some("accent"),
        Color::Error => Some("error"),
        Color::Warning => Some("warning"),
        Color::Success => Some("success"),
        _ => None,
    }
}

pub(super) const COLOR_CLASSES: [(&str, Color); 5] = [
    ("dim-label", Color::SecondaryLabel),
    ("accent", Color::Accent),
    ("error", Color::Error),
    ("warning", Color::Warning),
    ("success", Color::Success),
];

/// Replaces a label's Pango attributes of these types with `new`, keeping
/// the others: colour, weight and slant are set one at a time.
pub(super) fn replace_attrs(label: &gtk::Label, types: &[pango::AttrType], new: Vec<pango::Attribute>) {
    let list = pango::AttrList::new();
    for attr in label.attributes().map(|l| l.attributes()).unwrap_or_default() {
        if !types.contains(&attr.type_()) {
            list.insert(attr);
        }
    }
    for attr in new {
        list.insert(attr);
    }
    label.set_attributes(Some(&list));
}

pub(super) fn find_attr(label: &gtk::Label, type_: pango::AttrType) -> Option<pango::Attribute> {
    label.attributes()?.attributes().into_iter().find(|a| a.type_() == type_)
}

pub(super) fn pango_weight(weight: FontWeight) -> pango::Weight {
    match weight {
        FontWeight::Regular => pango::Weight::Normal,
        FontWeight::Medium => pango::Weight::Medium,
        FontWeight::Semibold => pango::Weight::Semibold,
        FontWeight::Bold => pango::Weight::Bold,
    }
}

pub(super) fn font_weight(weight: i32) -> FontWeight {
    match weight {
        ..450 => FontWeight::Regular,
        450..550 => FontWeight::Medium,
        550..650 => FontWeight::Semibold,
        _ => FontWeight::Bold,
    }
}

/// A label's colour as its classes and attributes show it. Theme colours
/// without a class are attributes that can't be told from `Rgba`, so
/// those come from what the app set.
pub(super) fn label_color(label: &gtk::Label, set: Option<Color>) -> Option<Color> {
    if let Some((_, color)) = COLOR_CLASSES.iter().find(|(class, _)| label.has_css_class(class)) {
        return Some(*color);
    }
    let Some(fg) = find_attr(label, pango::AttrType::Foreground) else { return set.map(|_| Color::Label) };
    if let Some(c @ (Color::Separator | Color::ControlBackground | Color::WindowBackground)) = set {
        return Some(c);
    }
    let fg = fg.downcast_ref::<pango::AttrColor>()?.color();
    let alpha = find_attr(label, pango::AttrType::ForegroundAlpha)
        .and_then(|a| a.downcast_ref::<pango::AttrInt>().map(|a| a.value()))
        .unwrap_or(65535);
    let byte = |v: u16| (v / 257) as u8;
    Some(Color::Rgba(byte(fg.red()), byte(fg.green()), byte(fg.blue()), byte(alpha as u16)))
}

/// The prefix of the classes that give an icon a colour of its own:
/// `mitsuami-color-rrggbbaa`.
const ICON_COLOR_CLASS: &str = "mitsuami-color-";

/// Colours an icon, as symbolic icons take CSS `color`: a semantic colour
/// by the label's style class, which follows the theme; `Label` by none;
/// others (fixed, or theme colours without a class, resolved now) by a
/// class of their own, whose rule goes in a style sheet for the display.
pub(super) fn set_icon_color(image: &gtk::Image, color: Color) {
    for class in image.css_classes() {
        if class.starts_with(ICON_COLOR_CLASS) || COLOR_CLASSES.iter().any(|(c, _)| *c == class.as_str()) {
            image.remove_css_class(&class);
        }
    }
    match color_class(color) {
        Some(class) => image.add_css_class(class),
        None if color == Color::Label => {}
        None => {
            let rgba = crate::custom::rgba(image.upcast_ref(), color);
            let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            let [r, g, b, a] = [rgba.red(), rgba.green(), rgba.blue(), rgba.alpha()].map(byte);
            let class = format!("{ICON_COLOR_CLASS}{r:02x}{g:02x}{b:02x}{a:02x}");
            install_icon_color(&class, (r, g, b, a));
            image.add_css_class(&class);
        }
    }
}

/// Adds a colour class's rule to the display's style sheet, once.
fn install_icon_color(class: &str, (r, g, b, a): (u8, u8, u8, u8)) {
    thread_local!(static SHEET: RefCell<(Option<gtk::CssProvider>, String)> = RefCell::default());
    SHEET.with_borrow_mut(|(provider, css)| {
        if css.contains(&format!(".{class} ")) {
            return;
        }
        css.push_str(&format!("image.{class} {{ color: rgba({r}, {g}, {b}, {}); }}\n", a as f32 / 255.0));
        let provider = provider.get_or_insert_with(|| {
            let provider = gtk::CssProvider::new();
            if let Some(display) = gtk::gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
            provider
        });
        provider.load_from_data(css);
    });
}

/// An icon's colour as its classes show it. Theme colours without a class
/// can't be told from `Rgba`, so those come from what the app set.
pub(super) fn icon_color(image: &gtk::Image, set: Option<Color>) -> Option<Color> {
    if let Some((_, color)) = COLOR_CLASSES.iter().find(|(class, _)| image.has_css_class(class)) {
        return Some(*color);
    }
    let classes = image.css_classes();
    let Some(hex) = classes.iter().find_map(|c| c.strip_prefix(ICON_COLOR_CLASS)) else {
        return set.map(|_| Color::Label);
    };
    if let Some(c @ (Color::Separator | Color::ControlBackground | Color::WindowBackground)) = set {
        return Some(c);
    }
    let byte = |at: usize| hex.get(at..at + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
    Some(Color::Rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?))
}

/// A text area's width: a text field's, as on the other platforms.
pub(super) const TEXT_AREA_WIDTH: f32 = 200.0;

/// Between a text area's frame and its text.
pub(super) const TEXT_AREA_MARGIN: i32 = 6;

/// All of a buffer's text.
pub(super) fn buffer_text(buffer: &gtk::TextBuffer) -> String {
    buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string()
}

/// A text area's text view, from its scrolled window.
pub(super) fn text_view(widget: &gtk::Widget) -> Option<gtk::TextView> {
    widget.downcast_ref::<gtk::ScrolledWindow>()?.child()?.downcast().ok()
}
