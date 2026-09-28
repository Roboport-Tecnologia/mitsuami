//! The theme: Kirigami's fonts, spacing units and colors, and forcing light
//! or dark.

use std::path::PathBuf;
use std::sync::OnceLock;

use mitsuami_core::backend::{Appearance, FontSizes, PlatformMetrics};
use mitsuami_core::units::SpacingScale;

use crate::ffi::{self, QmlObject};
use crate::qml;

thread_local! {
    static THEME: QmlObject = QmlObject::load(&qml::theme());
}

/// The object that exposes Kirigami's theme and units (see [`qml::theme`]).
pub(crate) fn theme() -> QmlObject {
    THEME.with(|t| *t)
}

/// An RGBA color in 0…1.
pub(crate) type Rgba = [f32; 4];

/// `#rrggbb` or `#aarrggbb`, as Qt prints colors.
fn parse(color: &str) -> Rgba {
    let hex = color.trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f32 / 255.0;
    match hex.len() {
        8 => [byte(2), byte(4), byte(6), byte(0)],
        _ => [byte(0), byte(2), byte(4), 1.0],
    }
}

/// The theme's colors, for drawn widgets.
pub(crate) struct Colors {
    pub(crate) text: Rgba,
    pub(crate) disabled_text: Rgba,
    pub(crate) highlight: Rgba,
    pub(crate) background: Rgba,
    pub(crate) view_background: Rgba,
    pub(crate) separator: Rgba,
    pub(crate) negative_text: Rgba,
    pub(crate) neutral_text: Rgba,
    pub(crate) positive_text: Rgba,
}

pub(crate) fn colors() -> Colors {
    let theme = theme();
    let color = |name: &str| parse(&theme.str(name));
    Colors {
        text: color("textColor"),
        disabled_text: color("disabledTextColor"),
        highlight: color("highlightColor"),
        background: color("backgroundColor"),
        view_background: color("viewBackgroundColor"),
        separator: color("separatorColor"),
        negative_text: color("negativeTextColor"),
        neutral_text: color("neutralTextColor"),
        positive_text: color("positiveTextColor"),
    }
}

fn luminance([r, g, b, _]: Rgba) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

pub(crate) fn metrics() -> PlatformMetrics {
    let theme = theme();
    let body = theme.font_px("defaultFont") as f32;
    // Kirigami.Heading's scale (levels 1, 2 and 3), as in `qml.rs`.
    let heading = |factor: f32| body * factor;
    let small = theme.real("smallSpacing") as f32;
    let large = theme.real("largeSpacing") as f32;
    PlatformMetrics {
        scale_factor: ffi::device_pixel_ratio() as f32,
        // Kirigami's units: smallSpacing (4) between related controls,
        // largeSpacing (8) between groups, gridUnit (18) for margins.
        spacing: SpacingScale {
            xs: small / 2.0,
            sm: small,
            md: large,
            lg: large * 1.5,
            xl: theme.real("gridUnit") as f32,
        },
        font_sizes: FontSizes {
            large_title: heading(1.35),
            title: heading(1.2),
            headline: heading(1.15),
            body,
            callout: body,
            // As in `qml.rs`: the small font, if it's smaller.
            caption: Some(theme.font_px("smallFont") as f32).filter(|small| *small < body).unwrap_or(body * 0.8),
            monospace: theme.font_px("fixedWidthFont") as f32,
        },
        dark_mode: luminance(parse(&theme.str("backgroundColor"))) < 0.5,
        high_contrast: false,
        // Kirigami's durations drop to zero when animations are off.
        reduced_motion: theme.real("longDuration") <= 0.0,
    }
}

/// Plasma's defaults, so tests don't depend on the desktop's fonts.
pub(crate) fn use_default_font() {
    ffi::set_app_font("Noto Sans", 10.0);
}

/// Switches the app to Breeze Light or Breeze Dark, the way KDE apps'
/// color scheme menus do.
pub(crate) fn force(appearance: Appearance) {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("mitsuami-kirigami-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        for (name, scheme) in [("BreezeLight.colors", BREEZE_LIGHT), ("BreezeDark.colors", BREEZE_DARK)] {
            let _ = std::fs::write(dir.join(name), scheme);
        }
        dir
    });
    let file = match appearance {
        Appearance::Light => "BreezeLight.colors",
        Appearance::Dark => "BreezeDark.colors",
    };
    ffi::set_color_scheme(&dir.join(file));
}

// Plasma's Breeze color schemes (the groups Kirigami reads).

const BREEZE_LIGHT: &str = "\
[General]
ColorScheme=BreezeLight
Name=Breeze Light

[KDE]
contrast=4

[ColorEffects:Disabled]
Color=56,56,56
ColorAmount=0
ColorEffect=0
ContrastAmount=0.65
ContrastEffect=1
IntensityAmount=0.1
IntensityEffect=2

[ColorEffects:Inactive]
ChangeSelectionColor=true
Color=112,111,110
ColorAmount=0.025
ColorEffect=2
ContrastAmount=0.1
ContrastEffect=2
Enable=false
IntensityAmount=0
IntensityEffect=0

[Colors:Button]
BackgroundAlternate=163,212,250
BackgroundNormal=252,252,252
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=112,125,138
ForegroundLink=41,128,185
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=35,38,41
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[Colors:Complementary]
BackgroundAlternate=27,30,32
BackgroundNormal=42,46,50
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=161,169,177
ForegroundLink=29,153,243
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=252,252,252
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[Colors:Header]
BackgroundAlternate=239,240,241
BackgroundNormal=222,224,226
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=112,125,138
ForegroundLink=41,128,185
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=35,38,41
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[Colors:Selection]
BackgroundAlternate=163,212,250
BackgroundNormal=61,174,233
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=255,255,255
ForegroundInactive=112,125,138
ForegroundLink=253,188,75
ForegroundNegative=176,55,69
ForegroundNeutral=198,92,0
ForegroundNormal=255,255,255
ForegroundPositive=23,104,57
ForegroundVisited=155,89,182

[Colors:Tooltip]
BackgroundAlternate=239,240,241
BackgroundNormal=247,247,247
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=112,125,138
ForegroundLink=41,128,185
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=35,38,41
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[Colors:View]
BackgroundAlternate=247,247,247
BackgroundNormal=255,255,255
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=112,125,138
ForegroundLink=41,128,185
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=35,38,41
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[Colors:Window]
BackgroundAlternate=227,229,231
BackgroundNormal=239,240,241
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=112,125,138
ForegroundLink=41,128,185
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=35,38,41
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[WM]
activeBackground=222,224,226
activeBlend=35,38,41
activeForeground=35,38,41
inactiveBackground=239,240,241
inactiveBlend=112,125,138
inactiveForeground=112,125,138
";

const BREEZE_DARK: &str = "\
[General]
ColorScheme=BreezeDark
Name=Breeze Dark

[KDE]
contrast=4

[ColorEffects:Disabled]
Color=56,56,56
ColorAmount=0
ColorEffect=0
ContrastAmount=0.65
ContrastEffect=1
IntensityAmount=0.1
IntensityEffect=2

[ColorEffects:Inactive]
ChangeSelectionColor=true
Color=112,111,110
ColorAmount=0.025
ColorEffect=2
ContrastAmount=0.1
ContrastEffect=2
Enable=false
IntensityAmount=0
IntensityEffect=0

[Colors:Button]
BackgroundAlternate=30,87,116
BackgroundNormal=41,44,48
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=161,169,177
ForegroundLink=29,153,243
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=252,252,252
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[Colors:Complementary]
BackgroundAlternate=27,30,32
BackgroundNormal=42,46,50
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=161,169,177
ForegroundLink=29,153,243
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=252,252,252
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[Colors:Header]
BackgroundAlternate=32,35,38
BackgroundNormal=41,44,48
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=161,169,177
ForegroundLink=29,153,243
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=252,252,252
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[Colors:Selection]
BackgroundAlternate=30,87,116
BackgroundNormal=61,174,233
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=252,252,252
ForegroundInactive=161,169,177
ForegroundLink=253,188,75
ForegroundNegative=176,55,69
ForegroundNeutral=198,92,0
ForegroundNormal=252,252,252
ForegroundPositive=23,104,57
ForegroundVisited=155,89,182

[Colors:Tooltip]
BackgroundAlternate=32,35,38
BackgroundNormal=41,44,48
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=161,169,177
ForegroundLink=29,153,243
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=252,252,252
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[Colors:View]
BackgroundAlternate=29,31,34
BackgroundNormal=20,22,24
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=161,169,177
ForegroundLink=29,153,243
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=252,252,252
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[Colors:Window]
BackgroundAlternate=41,44,48
BackgroundNormal=32,35,38
DecorationFocus=61,174,233
DecorationHover=61,174,233
ForegroundActive=61,174,233
ForegroundInactive=161,169,177
ForegroundLink=29,153,243
ForegroundNegative=218,68,83
ForegroundNeutral=246,116,0
ForegroundNormal=252,252,252
ForegroundPositive=39,174,96
ForegroundVisited=155,89,182

[WM]
activeBackground=39,44,49
activeBlend=252,252,252
activeForeground=252,252,252
inactiveBackground=32,36,40
inactiveBlend=161,169,177
inactiveForeground=161,169,177
";
