//! Numbers and dates, written the user's way: by the platform's own
//! formatters, which follow the region settings (decimal and grouping
//! separators, date order, 12 or 24 hours).

use std::borrow::Cow;
use std::time::{SystemTime, UNIX_EPOCH};

use fluent_bundle::types::{FluentNumberCurrencyDisplayStyle, FluentNumberOptions, FluentNumberStyle, FluentType};
use fluent_bundle::{FluentArgs, FluentValue};

/// The user's languages and how they write numbers and dates, as the
/// platform has them. Each backend gives one ([`Backend::locale`]).
///
/// [`Backend::locale`]: crate::Backend::locale
pub trait PlatformLocale {
    /// The languages the user prefers, most preferred first, as BCP 47
    /// tags (`"pt-BR"`, `"zh-Hant-TW"`).
    fn languages(&self) -> Vec<String>;

    /// `value` written as the user's region writes numbers, or `None` to
    /// have mitsuami write it.
    fn format_number(&self, value: f64, format: &NumberFormat) -> Option<String>;

    /// `time` written as the user's region writes dates and times, in the
    /// user's time zone, or `None` to have mitsuami write it.
    fn format_date_time(&self, time: SystemTime, format: &DateTimeFormat) -> Option<String>;
}

/// How a number is written: Fluent's `NUMBER()` options, which are
/// ECMAScript `Intl.NumberFormat`'s. Each platform does what its own
/// formatter can of them.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberFormat {
    pub style: NumberStyle,
    /// Separators between groups of digits, where the region has them.
    pub grouping: bool,
    pub minimum_integer_digits: Option<usize>,
    pub minimum_fraction_digits: Option<usize>,
    pub maximum_fraction_digits: Option<usize>,
    /// Significant digits, in place of the fraction digits when set.
    pub minimum_significant_digits: Option<usize>,
    pub maximum_significant_digits: Option<usize>,
}

impl Default for NumberFormat {
    fn default() -> NumberFormat {
        NumberFormat {
            style: NumberStyle::Decimal,
            grouping: true,
            minimum_integer_digits: None,
            minimum_fraction_digits: None,
            maximum_fraction_digits: None,
            minimum_significant_digits: None,
            maximum_significant_digits: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum NumberStyle {
    #[default]
    Decimal,
    /// The value times 100, with the region's percent sign.
    Percent,
    /// An amount of this currency (ISO 4217, `"EUR"`).
    Currency { code: String, display: CurrencyDisplay },
}

/// How a currency is named next to its amount.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CurrencyDisplay {
    /// Its symbol (€), where the region has one.
    #[default]
    Symbol,
    /// Its code (EUR).
    Code,
    /// Its name (euros), where the platform has one.
    Name,
}

impl NumberFormat {
    /// The fraction digits the style has when the format gives none, as
    /// `Intl.NumberFormat` has them.
    pub fn fraction_digits(&self) -> (usize, usize) {
        let (min, max) = match &self.style {
            NumberStyle::Decimal => (0, 3),
            NumberStyle::Percent => (0, 0),
            NumberStyle::Currency { code, .. } => {
                let digits = currency_digits(code);
                (digits, digits)
            }
        };
        // A maximum below the style's minimum lowers it, as Intl's does.
        let min = self.minimum_fraction_digits.unwrap_or(match self.maximum_fraction_digits {
            Some(max) => min.min(max),
            None => min,
        });
        let max = self.maximum_fraction_digits.unwrap_or(max).max(min);
        (min, max)
    }

    pub(crate) fn from_fluent(options: &FluentNumberOptions) -> NumberFormat {
        NumberFormat {
            style: match options.style {
                FluentNumberStyle::Decimal => NumberStyle::Decimal,
                FluentNumberStyle::Percent => NumberStyle::Percent,
                FluentNumberStyle::Currency => NumberStyle::Currency {
                    code: options.currency.clone().unwrap_or_default(),
                    display: match options.currency_display {
                        FluentNumberCurrencyDisplayStyle::Symbol => CurrencyDisplay::Symbol,
                        FluentNumberCurrencyDisplayStyle::Code => CurrencyDisplay::Code,
                        FluentNumberCurrencyDisplayStyle::Name => CurrencyDisplay::Name,
                    },
                },
            },
            grouping: options.use_grouping,
            // In the ranges `Intl.NumberFormat` takes, so a translation
            // can't ask for a billion digits.
            minimum_integer_digits: options.minimum_integer_digits.map(|n| n.clamp(1, 21)),
            minimum_fraction_digits: options.minimum_fraction_digits.map(|n| n.min(100)),
            maximum_fraction_digits: options.maximum_fraction_digits.map(|n| n.min(100)),
            minimum_significant_digits: options.minimum_significant_digits.map(|n| n.clamp(1, 21)),
            maximum_significant_digits: options.maximum_significant_digits.map(|n| n.clamp(1, 21)),
        }
    }
}

/// How a date and time are written: Fluent's `DATETIME()` `dateStyle`
/// and `timeStyle`, `Intl.DateTimeFormat`'s. Each platform maps them to
/// the styles it has. Neither: the date, short.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DateTimeFormat {
    pub date: Option<DateTimeStyle>,
    pub time: Option<DateTimeStyle>,
}

impl DateTimeFormat {
    /// What's shown: the date short when the format says nothing.
    pub fn resolved(self) -> (Option<DateTimeStyle>, Option<DateTimeStyle>) {
        match (self.date, self.time) {
            (None, None) => (Some(DateTimeStyle::Short), None),
            styles => styles,
        }
    }

    fn merge(&mut self, args: &FluentArgs) {
        for (key, value) in args.iter() {
            let FluentValue::String(style) = value else { continue };
            let style = match style.as_ref() {
                "short" => DateTimeStyle::Short,
                "medium" => DateTimeStyle::Medium,
                "long" => DateTimeStyle::Long,
                "full" => DateTimeStyle::Full,
                _ => continue,
            };
            match key {
                "dateStyle" => self.date = Some(style),
                "timeStyle" => self.time = Some(style),
                _ => {}
            }
        }
    }
}

/// From the shortest (`9/29/26`, `3:04 PM`) to the longest (`Tuesday,
/// September 29, 2026`, `3:04:05 PM Brasília Standard Time`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DateTimeStyle {
    Short,
    Medium,
    Long,
    Full,
}

/// A date and time passed to a message: a [`SystemTime`], with the
/// format `DATETIME()` gives it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DateTimeArg {
    pub time: SystemTime,
    pub format: DateTimeFormat,
}

impl FluentType for DateTimeArg {
    fn duplicate(&self) -> Box<dyn FluentType + Send> {
        Box::new(self.clone())
    }

    fn as_string(&self, _: &intl_memoizer::IntlLangMemoizer) -> Cow<'static, str> {
        format_date_time(self.time, &self.format).into()
    }

    fn as_string_threadsafe(&self, _: &intl_memoizer::concurrent::IntlLangMemoizer) -> Cow<'static, str> {
        basic_date_time(self.time, &self.format).into()
    }
}

/// Fluent's `DATETIME($date, dateStyle: "long", timeStyle: "short")`.
#[allow(non_snake_case)]
pub(crate) fn DATETIME<'a>(positional: &[FluentValue<'a>], named: &FluentArgs) -> FluentValue<'a> {
    let Some(FluentValue::Custom(value)) = positional.first() else { return FluentValue::Error };
    let Some(date) = value.as_any().downcast_ref::<DateTimeArg>() else { return FluentValue::Error };
    let mut date = date.clone();
    // Given styles replace the default, so a time alone is a time alone.
    date.format = DateTimeFormat::default();
    date.format.merge(named);
    FluentValue::Custom(Box::new(date))
}

/// Every value a message shows goes through here: numbers and dates are
/// the platform's to write.
pub(crate) fn format_value(value: &FluentValue, _: &intl_memoizer::IntlLangMemoizer) -> Option<String> {
    match value {
        FluentValue::Number(n) => Some(format_number(n.value, &NumberFormat::from_fluent(&n.options))),
        FluentValue::Custom(custom) => {
            custom.as_any().downcast_ref::<DateTimeArg>().map(|d| format_date_time(d.time, &d.format))
        }
        _ => None,
    }
}

/// `value` written the user's way, as messages write numbers: by the
/// platform, or else in the style of US English.
pub fn format_number(value: f64, format: &NumberFormat) -> String {
    super::platform_locale()
        .and_then(|platform| platform.format_number(value, format))
        .unwrap_or_else(|| basic_number(value, format))
}

/// `time` written the user's way, as messages write dates: by the
/// platform, or else in the style of US English, in UTC.
pub fn format_date_time(time: SystemTime, format: &DateTimeFormat) -> String {
    super::platform_locale()
        .and_then(|platform| platform.format_date_time(time, format))
        .unwrap_or_else(|| basic_date_time(time, format))
}

/// How a region writes numbers, for backends whose platform gives only
/// its symbols (glibc's `localeconv`), not a formatter.
#[derive(Clone, Debug, PartialEq)]
pub struct NumberSymbols {
    pub decimal: String,
    pub group: String,
    /// The sizes of the digit groups, from the decimal point out; the last
    /// repeats.
    pub grouping: Vec<usize>,
    /// Where a currency's symbol goes, and whether a space parts it from
    /// the amount.
    pub currency_before: bool,
    pub currency_space: bool,
    /// The percent sign, with any space the region puts beside it, and
    /// whether it goes before the number.
    pub percent: String,
    pub percent_before: bool,
}

impl NumberSymbols {
    /// US English's.
    pub fn us_english() -> NumberSymbols {
        NumberSymbols {
            decimal: ".".into(),
            group: ",".into(),
            grouping: vec![3],
            currency_before: true,
            currency_space: false,
            percent: "%".into(),
            percent_before: false,
        }
    }
}

/// US English, the way `Intl.NumberFormat("en-US")` writes it: what the
/// headless backend shows, so tests don't depend on the machine.
pub fn basic_number(value: f64, format: &NumberFormat) -> String {
    format_number_with(value, format, &NumberSymbols::us_english())
}

/// `value` written with a region's symbols, the digits as
/// `Intl.NumberFormat` rounds them.
pub fn format_number_with(value: f64, format: &NumberFormat, symbols: &NumberSymbols) -> String {
    if !value.is_finite() {
        return if value.is_nan() {
            "NaN".into()
        } else if value > 0.0 {
            "∞".into()
        } else {
            "-∞".into()
        };
    }
    let scaled = if format.style == NumberStyle::Percent { value * 100.0 } else { value };
    let digits = digits(scaled.abs(), format);
    let (int, frac) = match digits.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (digits, None),
    };
    let int = format!("{int:0>width$}", width = format.minimum_integer_digits.unwrap_or(1));
    let mut out = if format.grouping { group(&int, symbols) } else { int };
    if let Some(frac) = frac {
        out.push_str(&symbols.decimal);
        out.push_str(&frac);
    }
    let negative = scaled < 0.0 && out.chars().any(|c| c.is_ascii_digit() && c != '0');
    let sign = if negative { "-" } else { "" };
    match &format.style {
        NumberStyle::Decimal => format!("{sign}{out}"),
        NumberStyle::Percent if symbols.percent_before => format!("{sign}{}{out}", symbols.percent),
        NumberStyle::Percent => format!("{sign}{out}{}", symbols.percent),
        NumberStyle::Currency { code, display } => {
            let symbol = match (display, currency_symbol(code)) {
                (CurrencyDisplay::Symbol, Some(symbol)) => symbol.to_owned(),
                _ => code.clone(),
            };
            // A code is always parted from the amount.
            let space = if symbols.currency_space || symbol == *code { "\u{a0}" } else { "" };
            if symbols.currency_before {
                format!("{sign}{symbol}{space}{out}")
            } else {
                format!("{sign}{out}{space}{symbol}")
            }
        }
    }
}

/// What a format shows of `value` (times 100 for a percentage), rounded
/// as `Intl.NumberFormat` rounds it, and how many fraction digits it
/// shows: for platforms whose formatter takes a number of digits (Qt's).
pub fn rounded(value: f64, format: &NumberFormat) -> (f64, usize) {
    let scaled = if format.style == NumberStyle::Percent { value * 100.0 } else { value };
    if !scaled.is_finite() {
        return (scaled, 0);
    }
    let digits = digits(scaled.abs(), format);
    let decimals = digits.split_once('.').map_or(0, |(_, fraction)| fraction.len());
    (digits.parse::<f64>().unwrap_or(scaled.abs()).copysign(scaled), decimals)
}

/// The digits of `abs`, with a `.` before its fraction.
fn digits(abs: f64, format: &NumberFormat) -> String {
    match (format.minimum_significant_digits, format.maximum_significant_digits) {
        (None, None) => {
            let (min, max) = format.fraction_digits();
            trim_fraction(round_to(abs, max as i32), min)
        }
        (min, max) => {
            let max = max.unwrap_or(21).max(1);
            let min = min.unwrap_or(1).min(max);
            significant(abs, min, max)
        }
    }
}

/// `abs` rounded to `decimals` fraction digits (negative: to tens,
/// hundreds…), half away from zero as `Intl.NumberFormat` rounds, from its
/// shortest decimal form: `{:.N}` would round ties to even, and show the
/// binary value's digits past the 17th.
fn round_to(abs: f64, decimals: i32) -> String {
    let text = format!("{abs}");
    let (int, fraction) = text.split_once('.').unwrap_or((&text, ""));
    let mut digits: Vec<u8> = int.bytes().chain(fraction.bytes()).map(|b| b - b'0').collect();
    let mut point = int.len() as i32;
    let keep = point + decimals;
    if keep < 0 {
        digits.clear();
    } else if (keep as usize) < digits.len() {
        let up = digits[keep as usize] >= 5;
        digits.truncate(keep as usize);
        if up {
            match digits.iter().rposition(|d| *d < 9) {
                Some(i) => {
                    digits[i] += 1;
                    digits.truncate(i + 1);
                }
                None => {
                    digits.clear();
                    digits.push(1);
                    point += 1;
                }
            }
        }
    }
    let digit =
        |i: i32| if i >= 0 && (i as usize) < digits.len() { char::from(b'0' + digits[i as usize]) } else { '0' };
    let int: String = (0..point).map(digit).collect();
    let int = match int.trim_start_matches('0') {
        "" => "0".to_owned(),
        trimmed => trimmed.to_owned(),
    };
    if decimals <= 0 {
        return int;
    }
    let fraction: String = (point..point + decimals).map(digit).collect();
    format!("{int}.{fraction}")
}

/// Drops trailing zeros past `min` fraction digits.
fn trim_fraction(mut digits: String, min: usize) -> String {
    if let Some(dot) = digits.find('.') {
        let keep = dot + 1 + min;
        while digits.len() > keep && digits.ends_with('0') {
            digits.pop();
        }
        if digits.ends_with('.') {
            digits.pop();
        }
    }
    digits
}

fn significant(value: f64, min: usize, max: usize) -> String {
    if value == 0.0 {
        return trim_fraction(round_to(0.0, max as i32 - 1), min.saturating_sub(1));
    }
    let exponent = value.log10().floor() as i32;
    let min_decimals = (min as i32 - 1 - exponent).max(0) as usize;
    trim_fraction(round_to(value, max as i32 - 1 - exponent), min_decimals)
}

/// A currency's minor unit, as ISO 4217 and Intl have it.
fn currency_digits(code: &str) -> usize {
    match code {
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF" | "UGX" | "VND" | "VUV"
        | "XAF" | "XOF" | "XPF" => 0,
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
        _ => 2,
    }
}

fn group(int: &str, symbols: &NumberSymbols) -> String {
    let Some(&last) = symbols.grouping.last().filter(|size| **size > 0) else { return int.to_owned() };
    // Where groups start, counted from the right.
    let mut cuts = Vec::new();
    let mut at = 0;
    for i in 0.. {
        at += symbols.grouping.get(i).copied().unwrap_or(last);
        if at >= int.len() {
            break;
        }
        cuts.push(int.len() - at);
    }
    let mut out = String::with_capacity(int.len() + cuts.len() * symbols.group.len());
    for (i, c) in int.chars().enumerate() {
        if cuts.contains(&i) {
            out.push_str(&symbols.group);
        }
        out.push(c);
    }
    out
}

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

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/// US English in UTC, as `Intl.DateTimeFormat("en-US", { timeZone: "UTC" })`
/// writes it: what the headless backend shows.
pub fn basic_date_time(time: SystemTime, format: &DateTimeFormat) -> String {
    let seconds = match time.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs_f64().ceil() as i64),
    };
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let weekday = WEEKDAYS[(days + 4).rem_euclid(7) as usize];
    let month_name = MONTHS[month as usize - 1];
    let (date_style, time_style) = format.resolved();
    let date = date_style.map(|style| match style {
        DateTimeStyle::Short => format!("{month}/{day}/{:02}", year.rem_euclid(100)),
        DateTimeStyle::Medium => format!("{} {day}, {year}", &month_name[..3]),
        DateTimeStyle::Long => format!("{month_name} {day}, {year}"),
        DateTimeStyle::Full => format!("{weekday}, {month_name} {day}, {year}"),
    });
    let time = time_style.map(|style| {
        let (hour, minute, second) = (of_day / 3600, of_day / 60 % 60, of_day % 60);
        let (hour12, half) = (if hour % 12 == 0 { 12 } else { hour % 12 }, if hour < 12 { "AM" } else { "PM" });
        match style {
            DateTimeStyle::Short => format!("{hour12}:{minute:02}\u{202f}{half}"),
            DateTimeStyle::Medium => format!("{hour12}:{minute:02}:{second:02}\u{202f}{half}"),
            DateTimeStyle::Long | DateTimeStyle::Full => {
                format!("{hour12}:{minute:02}:{second:02}\u{202f}{half} UTC")
            }
        }
    });
    match (date, time) {
        (Some(date), Some(time)) => format!("{date}, {time}"),
        (Some(one), None) | (None, Some(one)) => one,
        (None, None) => String::new(),
    }
}

/// Howard Hinnant's `civil_from_days`: the proleptic Gregorian date of a
/// day counted from 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}
