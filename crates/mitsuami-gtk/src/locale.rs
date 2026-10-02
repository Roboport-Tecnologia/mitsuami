//! The user's languages and formats, and the app's direction, for GTK.

use std::ffi::{CStr, CString};
use std::time::{SystemTime, UNIX_EPOCH};

use gtk::glib;
use mitsuami_core::l10n::{
    DateTimeFormat, DateTimeStyle, LanguageIdentifier, NumberFormat, NumberStyle, NumberSymbols, PlatformLocale,
    format_number_with, parse_tag,
};

/// What GNOME apps go by: GLib's languages (from `LANGUAGE`, `LC_ALL`,
/// `LC_MESSAGES` and `LANG`, as gettext reads them), and the C library's
/// region settings (`LC_NUMERIC`, `LC_MONETARY`, `LC_TIME`), which GTK
/// takes on when it starts.
pub(crate) struct GtkLocale {
    /// Tests': one locale, dates in UTC, whatever the machine's settings.
    /// Without that locale installed, numbers and dates are mitsuami's.
    fixed: Option<(String, Option<Locale>)>,
}

impl GtkLocale {
    pub(crate) fn new(fixed: Option<&str>) -> GtkLocale {
        GtkLocale { fixed: fixed.map(|tag| (tag.to_owned(), Locale::new(tag))) }
    }

    /// Runs `f` in the fixed locale, or the process's; `None` when the
    /// fixed one isn't installed.
    fn in_locale<R>(&self, f: impl FnOnce() -> R) -> Option<R> {
        match &self.fixed {
            Some((_, Some(locale))) => Some(locale.with(f)),
            Some((_, None)) => None,
            None => Some(f()),
        }
    }
}

impl PlatformLocale for GtkLocale {
    fn languages(&self) -> Vec<String> {
        if let Some((tag, _)) = &self.fixed {
            return vec![tag.clone()];
        }
        let mut languages: Vec<String> = Vec::new();
        for name in glib::language_names() {
            if let Some(tag) = parse_tag(&name).map(|t| t.to_string())
                && !languages.contains(&tag)
            {
                languages.push(tag);
            }
        }
        languages
    }

    /// With the region's separators and grouping; the C library has no
    /// percent or currency patterns, so those are mitsuami's around them.
    fn format_number(&self, value: f64, format: &NumberFormat) -> Option<String> {
        let symbols = self.in_locale(|| symbols(matches!(format.style, NumberStyle::Currency { .. })))?;
        Some(format_number_with(value, format, &symbols))
    }

    /// GLib has one date and one time format per locale (the C library's
    /// `%x` and `%X`), which every style maps to.
    fn format_date_time(&self, time: SystemTime, format: &DateTimeFormat) -> Option<String> {
        let seconds = match time.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_secs() as i64,
            Err(e) => -(e.duration().as_secs() as i64),
        };
        let date_time = match self.fixed {
            Some(_) => glib::DateTime::from_unix_utc(seconds),
            None => glib::DateTime::from_unix_local(seconds),
        }
        .ok()?;
        let pattern = match format.resolved() {
            (Some(DateTimeStyle::Full), None) => "%A %x",
            (Some(_), None) => "%x",
            (None, Some(_)) => "%X",
            (Some(DateTimeStyle::Full), Some(_)) => "%A %x %X",
            (Some(_), Some(_)) => "%x %X",
            (None, None) => "%x",
        };
        self.in_locale(|| date_time.format(pattern).ok().map(|s| s.to_string())).flatten()
    }
}

/// The locale's number symbols, from `localeconv`: its monetary ones for
/// currencies.
fn symbols(money: bool) -> NumberSymbols {
    // SAFETY: `localeconv` returns the thread's locale's conventions, read
    // before anything on this thread changes it.
    let conv = unsafe { &*libc::localeconv() };
    let text = |p: *const libc::c_char| {
        if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
    };
    let (decimal, group, grouping) = if money {
        (text(conv.mon_decimal_point), text(conv.mon_thousands_sep), text(conv.mon_grouping))
    } else {
        (text(conv.decimal_point), text(conv.thousands_sep), text(conv.grouping))
    };
    // Each byte is a group's size, from the decimal point out; the last
    // repeats, and CHAR_MAX ends the grouping.
    let grouping = grouping.bytes().take_while(|b| *b != 0 && *b != 127).map(usize::from).collect();
    NumberSymbols {
        decimal: if decimal.is_empty() { ".".into() } else { decimal },
        group,
        grouping,
        currency_before: conv.p_cs_precedes != 0,
        currency_space: conv.p_sep_by_space != 0,
        percent: "%".into(),
        percent_before: false,
    }
}

/// A C locale, installed on the system, to format in for a while.
pub(crate) struct Locale(libc::locale_t);

impl Locale {
    fn new(tag: &str) -> Option<Locale> {
        let name = CString::new(format!("{}.UTF-8", tag.replace('-', "_"))).ok()?;
        // SAFETY: a fresh locale object, freed on drop.
        let locale = unsafe { libc::newlocale(libc::LC_ALL_MASK, name.as_ptr(), std::ptr::null_mut()) };
        // Lazily: a `Locale` made and dropped would free the null one.
        (!locale.is_null()).then(|| Locale(locale))
    }

    fn with<R>(&self, f: impl FnOnce() -> R) -> R {
        // SAFETY: the thread's locale, put back after.
        let previous = unsafe { libc::uselocale(self.0) };
        let result = f();
        unsafe { libc::uselocale(previous) };
        result
    }
}

impl Drop for Locale {
    fn drop(&mut self) {
        unsafe { libc::freelocale(self.0) };
    }
}

/// GTK's direction for widgets that don't set theirs: header bars,
/// dialogs, popovers and menus. GNOME apps get it from their translation
/// of GTK's `default:LTR`; the app's language says here.
pub(crate) fn set_app_locale(_language: &LanguageIdentifier, right_to_left: bool) {
    gtk::Widget::set_default_direction(if right_to_left { gtk::TextDirection::Rtl } else { gtk::TextDirection::Ltr });
}
