//! The platform's metrics: spacing, the theme's font sizes, dark mode.

use gtk::prelude::*;
use gtk::{gdk, pango};
use mitsuami_core::TextStyle;
use mitsuami_core::backend::{FontSizes, PlatformMetrics};
use mitsuami_core::units::SpacingScale;

use super::text::text_style_class;

/// The font size the theme gives a text style, in logical px.
fn font_size(style: TextStyle) -> f32 {
    let label = gtk::Label::new(None);
    if let Some(class) = text_style_class(style) {
        label.add_css_class(class);
    }
    let Some(font) = label.pango_context().font_description() else { return 16.0 };
    let size = font.size() as f32 / pango::SCALE as f32;
    // Point sizes are CSS points: 4/3 px.
    if font.is_size_absolute() { size } else { size * 4.0 / 3.0 }
}

pub(super) fn metrics() -> PlatformMetrics {
    let settings = gtk::Settings::default();
    let theme = settings.as_ref().and_then(|s| s.gtk_theme_name()).unwrap_or_default().to_lowercase();
    let scale = gdk::Display::default()
        .and_then(|d| d.monitors().item(0))
        .and_then(|m| m.downcast::<gdk::Monitor>().ok())
        .map_or(1.0, |m| m.scale_factor() as f32);
    PlatformMetrics {
        scale_factor: scale,
        // GNOME spaces in multiples of 6px; 24px is a generous window margin.
        spacing: SpacingScale { xs: 3.0, sm: 6.0, md: 12.0, lg: 18.0, xl: 24.0 },
        font_sizes: FontSizes {
            large_title: font_size(TextStyle::LargeTitle),
            title: font_size(TextStyle::Title),
            headline: font_size(TextStyle::Headline),
            body: font_size(TextStyle::Body),
            callout: font_size(TextStyle::Callout),
            caption: font_size(TextStyle::Caption),
            monospace: font_size(TextStyle::Monospace),
        },
        // libadwaita's style manager knows the system's preference too.
        dark_mode: if adw::is_initialized() {
            adw::StyleManager::default().is_dark()
        } else {
            settings.as_ref().is_some_and(|s| s.is_gtk_application_prefer_dark_theme())
        } || theme.contains("dark"),
        high_contrast: theme.contains("highcontrast"),
        reduced_motion: settings.as_ref().is_some_and(|s| !s.is_gtk_enable_animations()),
        tab_insets: crate::tabs::default_insets(),
        group_insets: crate::group::insets(false),
        titled_group_insets: crate::group::insets(true),
    }
}
