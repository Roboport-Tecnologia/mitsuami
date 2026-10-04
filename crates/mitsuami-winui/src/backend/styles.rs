//! Fluent styles and brushes, text styles and fonts.

use std::cell::RefCell;
use std::collections::HashMap;

use mitsuami_core::backend::FontSizes;
use mitsuami_core::{Color, FontWeight};
use mitsuami_core::{Orientation, TextStyle};
use windows_core::{HSTRING, IInspectable, Interface};

use super::R;
use crate::bindings as w;

/// A resource of the app's merged dictionaries (Fluent styles and brushes).
fn resource<T: Interface>(name: &str) -> Option<T> {
    let resources = w::Application::Current().ok()?.cast::<w::IApplication>().ok()?.Resources().ok()?;
    let map = resources.cast::<windows_collections::IMap<IInspectable, IInspectable>>().ok()?;
    map.Lookup(&windows_reference::IReference::from(HSTRING::from(name))).ok()?.cast().ok()
}

pub(crate) fn style(name: &str) -> w::Style {
    resource(name).unwrap_or_else(|| panic!("winui backend: missing XAML style {name}"))
}

/// Fluent's type ramp (Segoe UI Variable): Caption 12, Body 14, Subtitle 20,
/// Title 28, Title Large 40.
fn text_style_resource(style: TextStyle) -> &'static str {
    match style {
        TextStyle::LargeTitle => "TitleLargeTextBlockStyle",
        TextStyle::Title => "TitleTextBlockStyle",
        TextStyle::Headline => "SubtitleTextBlockStyle",
        TextStyle::Body | TextStyle::Callout | TextStyle::Monospace => "BodyTextBlockStyle",
        TextStyle::Caption => "CaptionTextBlockStyle",
    }
}

pub(super) fn font_sizes() -> FontSizes {
    FontSizes {
        large_title: 40.0,
        title: 28.0,
        headline: 20.0,
        body: 14.0,
        callout: 14.0,
        caption: 12.0,
        monospace: 14.0,
    }
}

pub(super) fn font_size(style: TextStyle) -> f64 {
    font_sizes().get(style) as f64
}

/// A label's style: its text style's, with its colour on top. The colour
/// is a setter, so a theme brush follows the theme live, as
/// `{ThemeResource}` does in a style.
pub(super) fn set_label_style(label: &w::TextBlock, text_style: Option<TextStyle>, color: Option<Color>) -> R<()> {
    let base = text_style.map(text_style_resource);
    let style = match (color, base) {
        (None, Some(base)) => style(base),
        (None, None) => return Ok(()),
        (Some(color), base) => made_style(foreground_markup("TextBlock", color), base)?,
    };
    label.cast::<w::IFrameworkElement>()?.SetStyle(&style)
}

thread_local! {
    /// Styles made from markup, by their markup and the style they're
    /// based on, so each coloured label and separator doesn't parse XAML
    /// again. One style serves every element with that look, as a
    /// resource dictionary's do (a style can't change once applied).
    static MADE_STYLES: RefCell<HashMap<(String, Option<&'static str>), w::Style>> = RefCell::default();
}

/// Past this many looks, the made styles start over (an app animating a
/// colour makes one a frame).
const MADE_STYLES_LIMIT: usize = 256;

fn made_style(markup: String, based_on: Option<&'static str>) -> R<w::Style> {
    let key = (markup, based_on);
    if let Some(made) = MADE_STYLES.with(|m| m.borrow().get(&key).cloned()) {
        return Ok(made);
    }
    let made: w::Style = w::XamlReader::Load(&key.0)?.cast()?;
    if let Some(base) = based_on {
        made.cast::<w::IStyle>()?.SetBasedOn(&style(base))?;
    }
    MADE_STYLES.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= MADE_STYLES_LIMIT {
            m.clear();
        }
        m.insert(key, made.clone());
    });
    Ok(made)
}

/// A separator's look: a line in the divider brush, 1 epx across, as
/// Fluent apps draw one between groups of content. The layout gives its
/// length.
pub(super) fn separator_style(orientation: Orientation) -> R<w::Style> {
    let across = if orientation.vertical() { "MinWidth" } else { "MinHeight" };
    let markup = format!(
        r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="Border"><Setter Property="Background" Value="{{ThemeResource DividerStrokeColorDefaultBrush}}"/><Setter Property="{across}" Value="1"/></Style>"#
    );
    made_style(markup, None)
}

/// A style that sets a `target`'s foreground to a colour, as markup, so a
/// theme resource keeps following the theme once set.
pub(super) fn foreground_style(target: &str, color: Color) -> R<w::Style> {
    made_style(foreground_markup(target, color), None)
}

fn foreground_markup(target: &str, color: Color) -> String {
    format!(
        r#"<Style xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" TargetType="{target}"><Setter Property="Foreground" Value="{}"/></Style>"#,
        crate::custom::text_brush(color)
    )
}

/// Fluent's weights (Segoe UI Variable has each of them).
pub(super) fn weight_value(weight: FontWeight) -> u16 {
    match weight {
        FontWeight::Regular => 400,
        FontWeight::Medium => 500,
        FontWeight::Semibold => 600,
        FontWeight::Bold => 700,
    }
}

/// The nearest of our weights to a font's.
pub(super) fn weight_of(weight: u16) -> FontWeight {
    match weight {
        0..450 => FontWeight::Regular,
        450..550 => FontWeight::Medium,
        550..650 => FontWeight::Semibold,
        _ => FontWeight::Bold,
    }
}

pub(super) fn font_weight(style: TextStyle) -> u16 {
    match style {
        TextStyle::LargeTitle | TextStyle::Title | TextStyle::Headline => 600,
        _ => 400,
    }
}

pub(super) const MONOSPACE: &str = "Cascadia Mono, Consolas";
