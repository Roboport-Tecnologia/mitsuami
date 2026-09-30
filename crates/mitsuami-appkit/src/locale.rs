//! The user's languages and formats, and the app's language for AppKit.

use std::time::{SystemTime, UNIX_EPOCH};

use mitsuami_core::l10n::{
    CurrencyDisplay, DateTimeFormat, DateTimeStyle, LanguageIdentifier, NumberFormat, NumberStyle, PlatformLocale,
};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{
    NSArgumentDomain, NSArray, NSDate, NSDateFormatter, NSDateFormatterStyle, NSLocale, NSMutableDictionary, NSNumber,
    NSNumberFormatter, NSNumberFormatterStyle, NSString, NSTimeZone, NSUserDefaults,
};

use crate::backend::ns;

/// `NSLocale`'s: the languages in System Settings (with the app's own, if
/// the user chose one for it), and the formats of the user's region, with
/// any changes the user made to them.
pub(crate) struct AppKitLocale {
    /// Read before the backend sets the app's language, which AppKit then
    /// lists first.
    languages: Vec<String>,
    /// Tests': one locale, in UTC, whatever the machine's settings.
    fixed: Option<Retained<NSLocale>>,
}

impl AppKitLocale {
    pub(crate) fn new(fixed: Option<&str>) -> AppKitLocale {
        match fixed {
            Some(tag) => AppKitLocale {
                languages: vec![tag.to_owned()],
                fixed: Some(NSLocale::localeWithLocaleIdentifier(&ns(&tag.replace('-', "_")))),
            },
            None => AppKitLocale {
                languages: NSLocale::preferredLanguages().iter().map(|l| l.to_string()).collect(),
                fixed: None,
            },
        }
    }

    fn locale(&self) -> Retained<NSLocale> {
        self.fixed.clone().unwrap_or_else(NSLocale::autoupdatingCurrentLocale)
    }
}

impl PlatformLocale for AppKitLocale {
    fn languages(&self) -> Vec<String> {
        self.languages.clone()
    }

    fn format_number(&self, value: f64, format: &NumberFormat) -> Option<String> {
        let formatter = NSNumberFormatter::new();
        formatter.setLocale(Some(&self.locale()));
        formatter.setNumberStyle(match &format.style {
            NumberStyle::Decimal => NSNumberFormatterStyle::DecimalStyle,
            NumberStyle::Percent => NSNumberFormatterStyle::PercentStyle,
            NumberStyle::Currency { display: CurrencyDisplay::Symbol, .. } => NSNumberFormatterStyle::CurrencyStyle,
            NumberStyle::Currency { display: CurrencyDisplay::Code, .. } => {
                NSNumberFormatterStyle::CurrencyISOCodeStyle
            }
            NumberStyle::Currency { display: CurrencyDisplay::Name, .. } => NSNumberFormatterStyle::CurrencyPluralStyle,
        });
        if let NumberStyle::Currency { code, .. } = &format.style {
            formatter.setCurrencyCode(Some(&ns(code)));
        }
        formatter.setUsesGroupingSeparator(format.grouping);
        if let Some(digits) = format.minimum_integer_digits {
            formatter.setMinimumIntegerDigits(digits);
        }
        if format.minimum_significant_digits.is_some() || format.maximum_significant_digits.is_some() {
            formatter.setUsesSignificantDigits(true);
            formatter.setMinimumSignificantDigits(format.minimum_significant_digits.unwrap_or(1));
            formatter.setMaximumSignificantDigits(format.maximum_significant_digits.unwrap_or(21));
        } else {
            // Only what the message asks for: the style's own digits are
            // the region's and the currency's (none for yen).
            if let Some(min) = format.minimum_fraction_digits {
                formatter.setMinimumFractionDigits(min);
                formatter.setMaximumFractionDigits(formatter.maximumFractionDigits().max(min));
            }
            if let Some(max) = format.maximum_fraction_digits {
                formatter.setMaximumFractionDigits(max.max(format.minimum_fraction_digits.unwrap_or(0)));
            }
        }
        formatter.stringFromNumber(&NSNumber::numberWithDouble(value)).map(|s| s.to_string())
    }

    fn format_date_time(&self, time: SystemTime, format: &DateTimeFormat) -> Option<String> {
        let formatter = NSDateFormatter::new();
        formatter.setLocale(Some(&self.locale()));
        if self.fixed.is_some() {
            formatter.setTimeZone(Some(&NSTimeZone::timeZoneForSecondsFromGMT(0)));
        }
        let style = |style: Option<DateTimeStyle>| match style {
            None => NSDateFormatterStyle::NoStyle,
            Some(DateTimeStyle::Short) => NSDateFormatterStyle::ShortStyle,
            Some(DateTimeStyle::Medium) => NSDateFormatterStyle::MediumStyle,
            Some(DateTimeStyle::Long) => NSDateFormatterStyle::LongStyle,
            Some(DateTimeStyle::Full) => NSDateFormatterStyle::FullStyle,
        };
        let (date, time_style) = format.resolved();
        formatter.setDateStyle(style(date));
        formatter.setTimeStyle(style(time_style));
        let seconds = match time.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_secs_f64(),
            Err(e) => -e.duration().as_secs_f64(),
        };
        Some(formatter.stringFromDate(&NSDate::dateWithTimeIntervalSince1970(seconds)).to_string())
    }
}

/// Shows AppKit in the app's language, as a bundle localized in it would
/// be: AppKit's own strings (the Services menu, text fields' context
/// menus, open panels), and, for a right-to-left language, window title
/// bars, menus and controls mirrored. It's what Xcode's App Language
/// option passes, in the process's argument domain. AppKit reads the
/// direction once, when the first window is made, so a later change
/// doesn't mirror the chrome; native widgets still get theirs.
pub(crate) fn set_app_locale(language: &LanguageIdentifier, right_to_left: bool) {
    let defaults = NSUserDefaults::standardUserDefaults();
    let domain = unsafe { NSArgumentDomain };
    let current = defaults.volatileDomainForName(domain);
    let arguments: Retained<NSMutableDictionary<NSString, AnyObject>> =
        NSMutableDictionary::dictionaryWithDictionary(&current);
    let languages = NSArray::from_retained_slice(&[ns(&language.to_string())]);
    let rtl = NSNumber::numberWithBool(right_to_left);
    arguments.insert(&*ns("AppleLanguages"), &**languages);
    arguments.insert(&*ns("AppleTextDirection"), &**rtl);
    arguments.insert(&*ns("NSForceRightToLeftWritingDirection"), &**rtl);
    unsafe { defaults.setVolatileDomain_forName(&arguments, domain) };
}
