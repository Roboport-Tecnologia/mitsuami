//! Fonts for text styles, and the platform metrics made of them.

use mitsuami_core::backend::{Appearance, FontSizes, PlatformMetrics};
use mitsuami_core::units::SpacingScale;
use mitsuami_core::{FontWeight, TextStyle};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, msg_send};
use objc2_app_kit::{
    NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSFont, NSFontDescriptorSymbolicTraits,
    NSFontTextStyle, NSFontTextStyleBody, NSFontTextStyleCallout, NSFontTextStyleCaption1, NSFontTextStyleHeadline,
    NSFontTextStyleLargeTitle, NSFontTextStyleTitle1, NSFontTraitsAttribute, NSFontWeightBold, NSFontWeightMedium,
    NSFontWeightRegular, NSFontWeightSemibold, NSFontWeightTrait, NSScreen, NSWorkspace,
};
use objc2_foundation::{NSArray, NSDictionary};

use super::group::group_insets;

pub(super) fn font(style: TextStyle) -> Retained<NSFont> {
    let text_style: &NSFontTextStyle = unsafe {
        match style {
            TextStyle::LargeTitle => NSFontTextStyleLargeTitle,
            TextStyle::Title => NSFontTextStyleTitle1,
            TextStyle::Headline => NSFontTextStyleHeadline,
            TextStyle::Body => NSFontTextStyleBody,
            TextStyle::Callout => NSFontTextStyleCallout,
            TextStyle::Caption => NSFontTextStyleCaption1,
            TextStyle::Monospace => {
                let size = font(TextStyle::Body).pointSize();
                return NSFont::monospacedSystemFontOfSize_weight(size, NSFontWeightRegular);
            }
        }
    };
    unsafe { NSFont::preferredFontForTextStyle_options(text_style, &NSDictionary::new()) }
}

/// A label's font: its text style's, with the app's weight and italics.
pub(super) fn label_font(
    style: Option<TextStyle>,
    weight: Option<FontWeight>,
    italic: Option<bool>,
) -> Retained<NSFont> {
    let style = style.unwrap_or(TextStyle::Body);
    let mut font = font(style);
    if let Some(weight) = weight {
        let weight = unsafe {
            match weight {
                FontWeight::Regular => NSFontWeightRegular,
                FontWeight::Medium => NSFontWeightMedium,
                FontWeight::Semibold => NSFontWeightSemibold,
                FontWeight::Bold => NSFontWeightBold,
            }
        };
        let size = font.pointSize();
        font = match style {
            TextStyle::Monospace => NSFont::monospacedSystemFontOfSize_weight(size, weight),
            _ => NSFont::systemFontOfSize_weight(size, weight),
        };
    }
    if italic == Some(true) {
        let descriptor = font.fontDescriptor();
        let italic = descriptor.fontDescriptorWithSymbolicTraits(
            descriptor.symbolicTraits() | NSFontDescriptorSymbolicTraits::TraitItalic,
        );
        if let Some(slanted) = NSFont::fontWithDescriptor_size(&italic, font.pointSize()) {
            font = slanted;
        }
    }
    font
}

/// The nearest `FontWeight` to a font's weight trait (-1 to 1).
pub(super) fn font_weight(font: &NSFont) -> FontWeight {
    let descriptor = font.fontDescriptor();
    let weight = unsafe { descriptor.objectForKey(NSFontTraitsAttribute) }
        .and_then(|traits| {
            let traits: Retained<NSDictionary> = traits.downcast().ok()?;
            let weight: Retained<AnyObject> = unsafe { msg_send![&*traits, objectForKey: NSFontWeightTrait] };
            Some(unsafe { msg_send![&*weight, doubleValue] })
        })
        .unwrap_or(0.0_f64);
    let weights = unsafe {
        [
            (FontWeight::Regular, NSFontWeightRegular),
            (FontWeight::Medium, NSFontWeightMedium),
            (FontWeight::Semibold, NSFontWeightSemibold),
            (FontWeight::Bold, NSFontWeightBold),
        ]
    };
    weights.into_iter().min_by(|a, b| (a.1 - weight).abs().total_cmp(&(b.1 - weight).abs())).unwrap().0
}

pub(super) fn metrics(mtm: MainThreadMarker, forced: Option<Appearance>) -> PlatformMetrics {
    let size = |style| font(style).pointSize() as f32;
    let dark = match forced {
        Some(appearance) => appearance == Appearance::Dark,
        None => {
            let appearance = NSApplication::sharedApplication(mtm).effectiveAppearance();
            let candidates = unsafe { [NSAppearanceNameAqua, NSAppearanceNameDarkAqua] };
            let names = NSArray::from_slice(&candidates);
            appearance
                .bestMatchFromAppearancesWithNames(&names)
                .is_some_and(|name| &*name == unsafe { NSAppearanceNameDarkAqua })
        }
    };
    let workspace = NSWorkspace::sharedWorkspace();
    PlatformMetrics {
        scale_factor: NSScreen::mainScreen(mtm).map_or(1.0, |s| s.backingScaleFactor() as f32),
        // The macOS HIG favours tight spacing; 20pt is the standard window margin.
        spacing: SpacingScale { xs: 4.0, sm: 6.0, md: 8.0, lg: 12.0, xl: 20.0 },
        font_sizes: FontSizes {
            large_title: size(TextStyle::LargeTitle),
            title: size(TextStyle::Title),
            headline: size(TextStyle::Headline),
            body: size(TextStyle::Body),
            callout: size(TextStyle::Callout),
            caption: size(TextStyle::Caption),
            monospace: size(TextStyle::Monospace),
        },
        dark_mode: dark,
        high_contrast: workspace.accessibilityDisplayShouldIncreaseContrast(),
        reduced_motion: workspace.accessibilityDisplayShouldReduceMotion(),
        tab_insets: crate::tabs::insets(mtm),
        group_insets: group_insets(mtm, ""),
        titled_group_insets: group_insets(mtm, "Title"),
    }
}
