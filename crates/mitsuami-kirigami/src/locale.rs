//! The user's languages and formats, and the app's language, for Qt.

use std::time::{SystemTime, UNIX_EPOCH};

use mitsuami_core::l10n::{
    CurrencyDisplay, DateTimeFormat, DateTimeStyle, LanguageIdentifier, NumberFormat, NumberStyle, PlatformLocale,
    rounded,
};

use crate::ffi;

/// `QLocale::system()`'s: the languages and the region formats in KDE's
/// settings, which Plasma's platform theme gives Qt.
pub(crate) struct QtLocale {
    /// Tests': one locale, dates in UTC, whatever the machine's settings.
    fixed: Option<String>,
}

impl QtLocale {
    pub(crate) fn new(fixed: Option<&str>) -> QtLocale {
        QtLocale { fixed: fixed.map(str::to_owned) }
    }
}

impl PlatformLocale for QtLocale {
    fn languages(&self) -> Vec<String> {
        ffi::ui_languages(self.fixed.as_deref())
    }

    /// Qt writes a number with the digits it's given, grouped and with
    /// the region's separators; the rounding is mitsuami's. Qt has no
    /// percent pattern, so a percentage is the number and `%`.
    fn format_number(&self, value: f64, format: &NumberFormat) -> Option<String> {
        let (value, decimals) = rounded(value, format);
        let locale = self.fixed.as_deref();
        Some(match &format.style {
            NumberStyle::Decimal => ffi::format_number(locale, value, decimals, format.grouping, None),
            NumberStyle::Percent => format!("{}%", ffi::format_number(locale, value, decimals, format.grouping, None)),
            NumberStyle::Currency { code, display } => {
                let symbol = match display {
                    CurrencyDisplay::Symbol => currency_symbol(code).unwrap_or(code),
                    CurrencyDisplay::Code | CurrencyDisplay::Name => code,
                };
                ffi::format_number(locale, value, decimals, format.grouping, Some(symbol))
            }
        })
    }

    /// Qt has a short and a long format: medium is short, full is long.
    fn format_date_time(&self, time: SystemTime, format: &DateTimeFormat) -> Option<String> {
        let msecs = match time.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_millis() as i64,
            Err(e) => -(e.duration().as_millis() as i64),
        };
        let style = |style: Option<DateTimeStyle>| match style {
            None => 0,
            Some(DateTimeStyle::Short | DateTimeStyle::Medium) => 1,
            Some(DateTimeStyle::Long | DateTimeStyle::Full) => 2,
        };
        let (date, time_style) = format.resolved();
        Some(ffi::format_date_time(self.fixed.as_deref(), msecs, style(date), style(time_style), self.fixed.is_some()))
    }
}

/// Qt takes the symbol to write, not the currency: the common ones'.
fn currency_symbol(code: &str) -> Option<&'static str> {
    Some(match code {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        "BRL" => "R$",
        "INR" => "₹",
        _ => return None,
    })
}

pub(crate) fn set_app_locale(language: &LanguageIdentifier, right_to_left: bool) {
    ffi::set_app_locale(&language.to_string(), right_to_left);
}
