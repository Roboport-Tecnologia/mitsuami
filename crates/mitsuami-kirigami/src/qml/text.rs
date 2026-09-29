//! Labels, and the colours and weights they're shown in.

use mitsuami_core::{Color, FontWeight};

use super::{TEXT_STYLE, a11y_hover};

pub(crate) fn label() -> String {
    // Word wrapping: a word longer than the line overflows rather than
    // breaking, so the longest word is the min-content width.
    format!(
        "QQC2.Label {{ readonly property bool mitsuamiSelectable: false; wrapMode: Text.WordWrap; \
         verticalAlignment: Text.AlignTop {TEXT_STYLE} {LABEL_OPTIONS} {} }}",
        a11y_hover("text")
    )
}

/// A label whose text can be selected: Kirigami's selectable label, a
/// read-only text area drawn as a label, as KDE apps show text to copy.
/// It has no line limit or elision, so those are kept, as a label has them.
pub(crate) fn selectable_label() -> String {
    format!(
        "Kirigami.SelectableLabel {{ readonly property bool mitsuamiSelectable: true; \
         property int maximumLineCount: 2147483647; property int elide: Text.ElideNone; wrapMode: Text.WordWrap; \
         verticalAlignment: Text.AlignTop {TEXT_STYLE} {LABEL_OPTIONS} {} }}",
        a11y_hover("text")
    )
}

/// The number a semantic colour has in a label's `mitsuamiColor`; `Rgba`
/// is `RGBA_COLOR`, with the colour in `mitsuamiRgba`.
pub(crate) fn color(color: Color) -> i32 {
    match color {
        Color::Label => 0,
        Color::SecondaryLabel => 1,
        Color::Accent => 2,
        Color::Separator => 3,
        Color::ControlBackground => 4,
        Color::WindowBackground => 5,
        Color::Error => 6,
        Color::Warning => 7,
        Color::Success => 8,
        Color::Rgba(..) => RGBA_COLOR,
    }
}

pub(crate) const RGBA_COLOR: i32 = 9;

/// The colour `color` gives `mitsuamiColor`, back.
pub(crate) fn color_from(index: i32, rgba: u32) -> Option<Color> {
    let [r, g, b, a] = rgba.to_be_bytes();
    Some(match index {
        0 => Color::Label,
        1 => Color::SecondaryLabel,
        2 => Color::Accent,
        3 => Color::Separator,
        4 => Color::ControlBackground,
        5 => Color::WindowBackground,
        6 => Color::Error,
        7 => Color::Warning,
        8 => Color::Success,
        RGBA_COLOR => Color::Rgba(r, g, b, a),
        _ => return None,
    })
}

/// `Font.Normal`, `Font.Medium`, `Font.DemiBold` and `Font.Bold`.
pub(crate) fn font_weight(weight: FontWeight) -> i32 {
    match weight {
        FontWeight::Regular => 400,
        FontWeight::Medium => 500,
        FontWeight::Semibold => 600,
        FontWeight::Bold => 700,
    }
}

/// The nearest weight to a font's.
pub(crate) fn font_weight_from(weight: i32) -> FontWeight {
    match weight {
        ..450 => FontWeight::Regular,
        450..550 => FontWeight::Medium,
        550..650 => FontWeight::Semibold,
        _ => FontWeight::Bold,
    }
}

/// A label's colour, weight and italics. The colours are Kirigami's, bound
/// so they follow the colour scheme (and the set the label is in, such as
/// a selected row's): secondary text is `disabledTextColor`, as Kirigami's
/// own subtitles use; the accent is `highlightColor`; errors, warnings and
/// successes are the negative, neutral and positive text colours. Without
/// one, the label is coloured as the desktop style colours it. The weight
/// and italics are the font's own sub-properties, so the text style's
/// family and size bindings stay; without a weight, it's the theme's.
/// `mitsuamiShownWeight` and `mitsuamiShownItalic` read the font back.
const LABEL_OPTIONS: &str = concat!(
    r#"
    property int mitsuamiColor: -1
    property int mitsuamiRgba: 0
    color: "#,
    color_binding!(),
    r#"
        ?? (enabled ? Kirigami.Theme.textColor : Kirigami.Theme.disabledTextColor)
    property int mitsuamiWeight: -1
    property bool mitsuamiItalic: false
    readonly property int mitsuamiShownWeight: font.weight
    readonly property bool mitsuamiShownItalic: font.italic
    font.weight: mitsuamiWeight >= 0 ? mitsuamiWeight : Kirigami.Theme.defaultFont.weight
    font.italic: mitsuamiItalic
"#
);
