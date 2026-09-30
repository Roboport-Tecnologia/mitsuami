//! The user's languages and formats, and the app's direction, for WinUI.

use std::time::{SystemTime, UNIX_EPOCH};

use mitsuami_core::l10n::{
    DateTimeFormat, DateTimeStyle, NumberFormat, NumberStyle, NumberSymbols, PlatformLocale, format_number_with,
};
use windows_core::{HSTRING, PCWSTR};

use crate::bindings as w;

/// Windows' own: the display languages in Settings, and the region's
/// formats with the user's changes to them (`LOCALE_NAME_USER_DEFAULT`).
pub(crate) struct WinLocale {
    /// Tests': one locale, dates in UTC, whatever the machine's settings.
    fixed: Option<HSTRING>,
}

impl WinLocale {
    pub(crate) fn new(fixed: Option<&str>) -> WinLocale {
        WinLocale { fixed: fixed.map(HSTRING::from) }
    }

    /// The locale's name, or null: the user's.
    fn name(&self) -> PCWSTR {
        self.fixed.as_ref().map_or(PCWSTR::null(), |name| PCWSTR(name.as_ptr()))
    }

    fn info(&self, kind: i32) -> String {
        let mut buffer = [0u16; 128];
        let len = unsafe { w::GetLocaleInfoEx(self.name(), kind as _, windows_core::PWSTR(buffer.as_mut_ptr()), 128) };
        String::from_utf16_lossy(&buffer[..(len.max(1) - 1) as usize])
    }

    fn number(&self, kind: i32) -> u32 {
        let mut value = 0u32;
        let buffer = windows_core::PWSTR(&mut value as *mut u32 as *mut u16);
        unsafe { w::GetLocaleInfoEx(self.name(), (kind | w::LOCALE_RETURN_NUMBER) as _, buffer, 2) };
        value
    }
}

impl PlatformLocale for WinLocale {
    fn languages(&self) -> Vec<String> {
        if let Some(fixed) = &self.fixed {
            return vec![fixed.to_string_lossy()];
        }
        let (mut count, mut len) = (0u32, 0u32);
        // The size first.
        _ = unsafe {
            w::GetUserPreferredUILanguages(w::MUI_LANGUAGE_NAME as u32, &mut count, std::ptr::null_mut(), &mut len)
        };
        let mut buffer = vec![0u16; len as usize];
        let ok = unsafe {
            w::GetUserPreferredUILanguages(w::MUI_LANGUAGE_NAME as u32, &mut count, buffer.as_mut_ptr(), &mut len)
        };
        if !ok.as_bool() {
            return Vec::new();
        }
        // NUL-separated, ending in two.
        buffer.split(|c| *c == 0).filter(|l| !l.is_empty()).map(String::from_utf16_lossy).collect()
    }

    /// With the region's symbols, and where its currency and percent signs
    /// go; `GetNumberFormatEx` takes neither a percentage nor a currency
    /// other than the region's.
    fn format_number(&self, value: f64, format: &NumberFormat) -> Option<String> {
        let money = matches!(format.style, NumberStyle::Currency { .. });
        let (decimal, group, grouping) = if money {
            (w::LOCALE_SMONDECIMALSEP, w::LOCALE_SMONTHOUSANDSEP, w::LOCALE_SMONGROUPING)
        } else {
            (w::LOCALE_SDECIMAL, w::LOCALE_STHOUSAND, w::LOCALE_SGROUPING)
        };
        // "3;0" is threes, "3;2;0" India's three then twos.
        let grouping = self.info(grouping).split(';').filter_map(|g| g.parse().ok()).filter(|g| *g > 0).collect();
        // 0: $1.1, 1: 1.1$, 2: $ 1.1, 3: 1.1 $.
        let currency = self.number(w::LOCALE_ICURRENCY);
        // 0: 1 %, 1: 1%, 2: %1, 3: % 1.
        let percent = self.number(w::LOCALE_IPOSITIVEPERCENT);
        let symbols = NumberSymbols {
            decimal: self.info(decimal),
            group: self.info(group),
            grouping,
            currency_before: currency.is_multiple_of(2),
            currency_space: currency >= 2,
            percent: match percent {
                0 => "\u{a0}%".into(),
                3 => "%\u{a0}".into(),
                _ => "%".into(),
            },
            percent_before: percent >= 2,
        };
        Some(format_number_with(value, format, &symbols))
    }

    /// Windows has a short and a long date, and a time with or without
    /// seconds: medium is short, full is long, and a short time has no
    /// seconds.
    fn format_date_time(&self, time: SystemTime, format: &DateTimeFormat) -> Option<String> {
        let ticks = match time.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_nanos() / 100,
            Err(_) => return None,
        } + 116_444_736_000_000_000;
        let file_time = w::FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 };
        let mut utc = unsafe { std::mem::zeroed::<w::SYSTEMTIME>() };
        if !unsafe { w::FileTimeToSystemTime(&file_time, &mut utc) }.as_bool() {
            return None;
        }
        let mut at = utc;
        if self.fixed.is_none()
            && !unsafe { w::SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut at) }.as_bool()
        {
            return None;
        }
        let (date, time_style) = format.resolved();
        let mut parts = Vec::new();
        let mut buffer = [0u16; 256];
        if let Some(style) = date {
            let flags = match style {
                DateTimeStyle::Short | DateTimeStyle::Medium => w::DATE_SHORTDATE,
                DateTimeStyle::Long | DateTimeStyle::Full => w::DATE_LONGDATE,
            };
            let out = windows_core::PWSTR(buffer.as_mut_ptr());
            let len =
                unsafe { w::GetDateFormatEx(self.name(), flags as u32, &at, PCWSTR::null(), out, 256, PCWSTR::null()) };
            parts.push(String::from_utf16_lossy(&buffer[..(len.max(1) - 1) as usize]));
        }
        if let Some(style) = time_style {
            let flags = if style == DateTimeStyle::Short { w::TIME_NOSECONDS as u32 } else { 0 };
            let out = windows_core::PWSTR(buffer.as_mut_ptr());
            let len = unsafe { w::GetTimeFormatEx(self.name(), flags, &at, PCWSTR::null(), out, 256) };
            parts.push(String::from_utf16_lossy(&buffer[..(len.max(1) - 1) as usize]));
        }
        Some(parts.join(" "))
    }
}
