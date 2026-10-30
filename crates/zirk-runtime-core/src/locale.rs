//! Versioned, explicit locale handling for civil temporal values.
//!
//! The formatter data is ICU4X 2.4.0's compiled data, which is generated from
//! CLDR 49. No API in this module reads process-global locale state.

use crate::zoned_temporal::{DstPolicy, GapPolicy, OverlapPolicy, TimeZone, ZonedDateTime};
use icu_datetime::{
    DateTimeFormatter, DateTimeFormatterPreferences, NoCalendarFormatter,
    fieldsets::{T, YMD, YMDE, YMDET, YMDT, zone::SpecificLong},
    input::{Date, DateTime, Time},
};
use icu_locale_core::Locale;
use std::collections::HashMap;
use std::str::FromStr;

/// Components returned by successful localized date-time parsing.
pub type ParsedDateTimeParts = (i32, u8, u8, u8, u8, u8, u32);

mod exact_locale_data {
    mod icu {
        pub use icu_datetime as datetime;
    }
    use icu_provider::DataLocale;

    struct Provider;

    const _: () = {
        use icu_datetime_data::*;
        icu_datetime_data::make_provider!(Provider);
        impl_datetime_patterns_date_gregorian_v1!(Provider);
        impl_datetime_patterns_time_v1!(Provider);
        impl_datetime_names_weekday_v1!(Provider);
    };

    fn supported_data_locales() -> &'static std::collections::HashSet<DataLocale> {
        use icu_datetime::provider::names::DatetimeNamesWeekdayV1;
        use icu_datetime::provider::semantic_skeletons::{
            DatetimePatternsDateGregorianV1, DatetimePatternsTimeV1,
        };
        use icu_provider::baked::DataStore;
        use std::sync::OnceLock;

        static SUPPORTED: OnceLock<std::collections::HashSet<DataLocale>> = OnceLock::new();
        SUPPORTED.get_or_init(|| {
            let dates = <_ as DataStore<DatetimePatternsDateGregorianV1>>::iter(
                &Provider::DATA_DATETIME_PATTERNS_DATE_GREGORIAN_V1,
            )
            .map(|identifier| identifier.locale)
            .collect::<std::collections::HashSet<_>>();
            <_ as DataStore<DatetimePatternsTimeV1>>::iter(
                &Provider::DATA_DATETIME_PATTERNS_TIME_V1,
            )
            .map(|identifier| identifier.locale)
            .filter(|locale| dates.contains(locale))
            .filter(|locale| {
                <_ as DataStore<DatetimeNamesWeekdayV1>>::iter(
                    &Provider::DATA_DATETIME_NAMES_WEEKDAY_V1,
                )
                .any(|identifier| &identifier.locale == locale)
            })
            .collect()
        })
    }

    pub(super) fn supports(locale: &icu_locale_core::Locale) -> bool {
        let fallbacker = icu_locale_fallback::LocaleFallbacker::new();
        let mut fallback = fallbacker
            .for_config(Default::default())
            .fallback_for(DataLocale::from(locale));
        for _ in 0..16 {
            let candidate = fallback.get();
            // The root locale is a generic emergency fallback, not evidence
            // that the requested language is represented in the pinned CLDR.
            if candidate.is_unknown() {
                return false;
            }
            if supported_data_locales().contains(candidate) {
                return true;
            }
            fallback.step();
        }
        false
    }
}

/// The ICU4X component release used to compile the locale data.
pub const ICU4X_DATA_VERSION: &str = "2.4.0";

/// The CLDR release embedded by ICU4X 2.4.0.
pub const CLDR_DATA_VERSION: &str = "49";

/// Locale-sensitive operation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocaleError {
    /// The input is not a well-formed BCP 47 / UTS #35 locale identifier.
    InvalidTag,
    /// The pinned compiled data has no matching temporal resources.
    UnsupportedLocale,
    /// A date or time component is outside its calendar range.
    InvalidTemporalValue,
    /// The requested presentation width is not recognized.
    InvalidStyle,
    /// Compiled locale data could not be loaded.
    DataUnavailable,
    /// The requested IANA time-zone identifier is not in the pinned database.
    UnsupportedTimeZone,
    /// A local wall time occurs twice under the selected IANA rules.
    AmbiguousLocalTime,
    /// A local wall time does not exist under the selected IANA rules.
    NonexistentLocalTime,
    /// The overlap or gap policy name is not supported.
    InvalidDstPolicy,
    /// Localized text does not match the requested style or contains an invalid value.
    InvalidInput,
    /// Localized text has more than one valid interpretation or uses a two-digit year.
    AmbiguousInput,
}

/// Requested date/time presentation length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeStyle {
    /// Numeric, compact representation.
    Short,
    /// Abbreviated representation.
    Medium,
    /// Expanded representation with names.
    Long,
    /// Full representation with weekday and expanded names.
    Full,
}

/// Parses a stable public style name (`short`, `medium`, `long`, or `full`).
pub fn parse_style(style: &str) -> Result<DateTimeStyle, LocaleError> {
    match style {
        "short" => Ok(DateTimeStyle::Short),
        "medium" => Ok(DateTimeStyle::Medium),
        "long" => Ok(DateTimeStyle::Long),
        "full" => Ok(DateTimeStyle::Full),
        _ => Err(LocaleError::InvalidStyle),
    }
}

fn parse_locale(tag: &str) -> Result<Locale, LocaleError> {
    Locale::from_str(tag).map_err(|_| LocaleError::InvalidTag)
}

fn require_locale_data(tag: &str) -> Result<Locale, LocaleError> {
    let locale = parse_locale(tag)?;
    if !exact_locale_data::supports(&locale) {
        return Err(LocaleError::UnsupportedLocale);
    }
    Ok(locale)
}

/// Parses and canonically serializes a BCP 47 locale tag.
pub fn canonicalize_bcp47(tag: &str) -> Result<String, LocaleError> {
    parse_locale(tag).map(|locale| locale.to_string())
}

fn formatter_error(_: impl std::fmt::Debug) -> LocaleError {
    // The compiled provider is part of this crate's dependency graph. A
    // formatter load error therefore means that the requested locale lacks
    // one of the requested resources, rather than that a host lookup failed.
    LocaleError::UnsupportedLocale
}

/// Formats an ISO civil date under an explicit locale and CLDR style.
pub fn format_date(
    year: i32,
    month: u8,
    day: u8,
    locale_tag: &str,
    style: DateTimeStyle,
) -> Result<String, LocaleError> {
    let locale = require_locale_data(locale_tag)?;
    let date =
        Date::try_new_iso(year, month, day).map_err(|_| LocaleError::InvalidTemporalValue)?;
    let preferences = DateTimeFormatterPreferences::from(locale);
    let rendered = match style {
        DateTimeStyle::Short => DateTimeFormatter::<YMD>::try_new(preferences, YMD::short())
            .map_err(formatter_error)?
            .format(&date)
            .to_string(),
        DateTimeStyle::Medium => DateTimeFormatter::<YMD>::try_new(preferences, YMD::medium())
            .map_err(formatter_error)?
            .format(&date)
            .to_string(),
        DateTimeStyle::Long => DateTimeFormatter::<YMD>::try_new(preferences, YMD::long())
            .map_err(formatter_error)?
            .format(&date)
            .to_string(),
        DateTimeStyle::Full => DateTimeFormatter::<YMDE>::try_new(preferences, YMDE::long())
            .map_err(formatter_error)?
            .format(&date)
            .to_string(),
    };
    Ok(rendered)
}

/// Formats a civil date with a stable named style suitable for language APIs.
pub fn format_date_named(
    year: i32,
    month: u8,
    day: u8,
    locale_tag: &str,
    style: &str,
) -> Result<String, LocaleError> {
    format_date(year, month, day, locale_tag, parse_style(style)?)
}

/// Parses a localized Gregorian date using the exact pinned CLDR pattern for
/// `locale_tag` and `style`. Textual month and weekday fields are validated by
/// formatting each numeric candidate back through the same formatter. A
/// two-digit year is rejected as ambiguous because CLDR does not define a
/// century pivot.
pub fn parse_date_named(text: &str, locale_tag: &str, style: &str) -> Result<i64, LocaleError> {
    let style = parse_style(style)?;
    let pattern = date_pattern(locale_tag, style)?;
    let parsed = parse_pattern_numbers(&pattern, text)?;
    if parsed.iter().any(|fields| fields.two_digit_year) {
        return Err(LocaleError::AmbiguousInput);
    }

    let mut matches = std::collections::BTreeSet::new();
    for fields in parsed {
        let Some(year) = fields
            .values
            .get(&'y')
            .or_else(|| fields.values.get(&'Y'))
            .copied()
            .or_else(|| fields.values.get(&'u').copied())
        else {
            continue;
        };
        let Some(day) = fields.values.get(&'d').copied() else {
            continue;
        };
        let months: Vec<u32> = fields
            .values
            .get(&'M')
            .or_else(|| fields.values.get(&'L'))
            .copied()
            .map(|month| vec![month])
            .unwrap_or_else(|| (1..=12).collect());
        for month in months {
            let Ok(year) = i32::try_from(year) else {
                continue;
            };
            let Ok(month) = u8::try_from(month) else {
                continue;
            };
            let Ok(day) = u8::try_from(day) else {
                continue;
            };
            if format_date_named(year, month, day, locale_tag, style_name(style))
                .is_ok_and(|rendered| rendered == text)
                && let Some(date) =
                    chrono::NaiveDate::from_ymd_opt(year, u32::from(month), u32::from(day))
            {
                let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1)
                    .expect("the ISO epoch is always valid");
                matches.insert(date.signed_duration_since(epoch).num_days());
            }
        }
    }
    match matches.len() {
        0 => Err(LocaleError::InvalidInput),
        1 => Ok(*matches.first().expect("one match")),
        _ => Err(LocaleError::AmbiguousInput),
    }
}

/// Parses a localized time under the requested pinned CLDR style. Missing
/// fields are interpreted as zero; 12-hour fields are resolved only when the
/// localized day-period text distinguishes morning from afternoon.
pub fn parse_time_named(text: &str, locale_tag: &str, style: &str) -> Result<i64, LocaleError> {
    let style = parse_style(style)?;
    let pattern = time_pattern(locale_tag, style)?;
    let parsed = parse_pattern_numbers(&pattern, text)?;
    let mut matches = std::collections::BTreeSet::new();
    for fields in parsed {
        let minute = fields.values.get(&'m').copied().unwrap_or(0);
        let second = fields.values.get(&'s').copied().unwrap_or(0);
        let nanosecond = fields
            .values
            .get(&'S')
            .copied()
            .map(|fraction| {
                let width = fields.fraction_width.unwrap_or(1).min(9) as u32;
                fraction.saturating_mul(10_u32.pow(9 - width))
            })
            .unwrap_or(0);
        let hour_candidates = if let Some(hour_symbol) = fields.hour_symbol {
            let Some(hour) = fields.values.get(&hour_symbol).copied() else {
                continue;
            };
            match hour_symbol {
                'H' if hour <= 23 => vec![hour],
                'h' if (1..=12).contains(&hour) => vec![hour % 12, hour % 12 + 12],
                'K' if hour <= 11 => vec![hour, hour + 12],
                'k' if (1..=24).contains(&hour) => vec![hour % 24],
                _ => continue,
            }
        } else {
            continue;
        };
        for hour in hour_candidates {
            let (Ok(hour), Ok(minute), Ok(second)) = (
                u8::try_from(hour),
                u8::try_from(minute),
                u8::try_from(second),
            ) else {
                continue;
            };
            if format_time_named(
                hour,
                minute,
                second,
                nanosecond,
                locale_tag,
                style_name(style),
            )
            .is_ok_and(|rendered| rendered == text)
            {
                matches.insert(
                    i64::from(hour) * 3_600_000_000_000
                        + i64::from(minute) * 60_000_000_000
                        + i64::from(second) * 1_000_000_000
                        + i64::from(nanosecond),
                );
            }
        }
    }
    match matches.len() {
        0 => Err(LocaleError::InvalidInput),
        1 => Ok(*matches.first().expect("one match")),
        _ => Err(LocaleError::AmbiguousInput),
    }
}

/// Parses a localized civil date-time under the requested pinned CLDR style.
/// A numeric two-digit year is ambiguous because the locale data does not
/// prescribe a century pivot.
pub fn parse_datetime_named(
    text: &str,
    locale_tag: &str,
    style: &str,
) -> Result<ParsedDateTimeParts, LocaleError> {
    let style = parse_style(style)?;
    let pattern = datetime_pattern(locale_tag, style)?;
    let parsed = parse_pattern_numbers(&pattern, text)?;
    if parsed.iter().any(|fields| fields.two_digit_year) {
        return Err(LocaleError::AmbiguousInput);
    }
    let mut matches = std::collections::BTreeSet::new();
    for fields in parsed {
        let Some(year) = fields.values.get(&'y').copied() else {
            continue;
        };
        let Some(day) = fields.values.get(&'d').copied() else {
            continue;
        };
        let months: Vec<u32> = fields
            .values
            .get(&'M')
            .or_else(|| fields.values.get(&'L'))
            .copied()
            .map(|month| vec![month])
            .unwrap_or_else(|| (1..=12).collect());
        let minute = fields.values.get(&'m').copied().unwrap_or(0);
        let second = fields.values.get(&'s').copied().unwrap_or(0);
        let nanosecond = fields
            .values
            .get(&'S')
            .copied()
            .map(|fraction| {
                let width = fields.fraction_width.unwrap_or(1).min(9) as u32;
                fraction.saturating_mul(10_u32.pow(9 - width))
            })
            .unwrap_or(0);
        let hour_symbol = fields.hour_symbol;
        let Some(hour_symbol) = hour_symbol else {
            continue;
        };
        let Some(hour_value) = fields.values.get(&hour_symbol).copied() else {
            continue;
        };
        let hours = match hour_symbol {
            'H' if hour_value <= 23 => vec![hour_value],
            'h' if (1..=12).contains(&hour_value) => {
                vec![hour_value % 12, hour_value % 12 + 12]
            }
            'K' if hour_value <= 11 => vec![hour_value, hour_value + 12],
            'k' if (1..=24).contains(&hour_value) => vec![hour_value % 24],
            _ => continue,
        };
        for month in &months {
            for hour in &hours {
                let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(minute), Ok(second)) = (
                    i32::try_from(year),
                    u8::try_from(*month),
                    u8::try_from(day),
                    u8::try_from(*hour),
                    u8::try_from(minute),
                    u8::try_from(second),
                ) else {
                    continue;
                };
                if format_datetime_named(
                    year,
                    month,
                    day,
                    hour,
                    minute,
                    second,
                    nanosecond,
                    locale_tag,
                    style_name(style),
                )
                .is_ok_and(|rendered| rendered == text)
                {
                    matches.insert((year, month, day, hour, minute, second, nanosecond));
                }
            }
        }
    }
    match matches.len() {
        0 => Err(LocaleError::InvalidInput),
        1 => Ok(*matches.first().expect("one match")),
        _ => Err(LocaleError::AmbiguousInput),
    }
}

fn style_name(style: DateTimeStyle) -> &'static str {
    match style {
        DateTimeStyle::Short => "short",
        DateTimeStyle::Medium => "medium",
        DateTimeStyle::Long => "long",
        DateTimeStyle::Full => "full",
    }
}

fn date_pattern(locale_tag: &str, style: DateTimeStyle) -> Result<String, LocaleError> {
    let locale = require_locale_data(locale_tag)?;
    let date = Date::try_new_iso(2001, 11, 23).map_err(|_| LocaleError::InvalidTemporalValue)?;
    let preferences = DateTimeFormatterPreferences::from(locale);
    let pattern = match style {
        DateTimeStyle::Short => DateTimeFormatter::<YMD>::try_new(preferences, YMD::short())
            .map_err(formatter_error)?
            .format(&date)
            .pattern(),
        DateTimeStyle::Medium => DateTimeFormatter::<YMD>::try_new(preferences, YMD::medium())
            .map_err(formatter_error)?
            .format(&date)
            .pattern(),
        DateTimeStyle::Long => DateTimeFormatter::<YMD>::try_new(preferences, YMD::long())
            .map_err(formatter_error)?
            .format(&date)
            .pattern(),
        DateTimeStyle::Full => DateTimeFormatter::<YMDE>::try_new(preferences, YMDE::long())
            .map_err(formatter_error)?
            .format(&date)
            .pattern(),
    };
    Ok(pattern.to_string())
}

fn time_pattern(locale_tag: &str, style: DateTimeStyle) -> Result<String, LocaleError> {
    let locale = require_locale_data(locale_tag)?;
    let time =
        Time::try_new(16, 12, 20, 345_000_000).map_err(|_| LocaleError::InvalidTemporalValue)?;
    let preferences = DateTimeFormatterPreferences::from(locale);
    let pattern = match style {
        DateTimeStyle::Short => NoCalendarFormatter::try_new(preferences, T::short())
            .map_err(formatter_error)?
            .format(&time)
            .pattern(),
        DateTimeStyle::Medium => NoCalendarFormatter::try_new(preferences, T::medium())
            .map_err(formatter_error)?
            .format(&time)
            .pattern(),
        DateTimeStyle::Long | DateTimeStyle::Full => {
            NoCalendarFormatter::try_new(preferences, T::long())
                .map_err(formatter_error)?
                .format(&time)
                .pattern()
        }
    };
    Ok(pattern.to_string())
}

fn datetime_pattern(locale_tag: &str, style: DateTimeStyle) -> Result<String, LocaleError> {
    let locale = require_locale_data(locale_tag)?;
    let value = DateTime {
        date: Date::try_new_iso(2001, 11, 23).map_err(|_| LocaleError::InvalidTemporalValue)?,
        time: Time::try_new(16, 12, 20, 345_000_000)
            .map_err(|_| LocaleError::InvalidTemporalValue)?,
    };
    let preferences = DateTimeFormatterPreferences::from(locale);
    let pattern = match style {
        DateTimeStyle::Short => DateTimeFormatter::<YMDT>::try_new(preferences, YMDT::short())
            .map_err(formatter_error)?
            .format(&value)
            .pattern(),
        DateTimeStyle::Medium => DateTimeFormatter::<YMDT>::try_new(preferences, YMDT::medium())
            .map_err(formatter_error)?
            .format(&value)
            .pattern(),
        DateTimeStyle::Long => DateTimeFormatter::<YMDT>::try_new(preferences, YMDT::long())
            .map_err(formatter_error)?
            .format(&value)
            .pattern(),
        DateTimeStyle::Full => DateTimeFormatter::<YMDET>::try_new(preferences, YMDET::long())
            .map_err(formatter_error)?
            .format(&value)
            .pattern(),
    };
    Ok(pattern.to_string())
}

#[derive(Default, Clone)]
struct ParsedPatternFields {
    values: HashMap<char, u32>,
    two_digit_year: bool,
    hour_symbol: Option<char>,
    fraction_width: Option<usize>,
    has_day_period: bool,
}

#[derive(Clone)]
enum LocalizedPatternPart {
    Literal(String),
    Field(char, usize),
}

fn parse_pattern_numbers(
    pattern: &str,
    input: &str,
) -> Result<Vec<ParsedPatternFields>, LocaleError> {
    let parts = tokenize_pattern(pattern)?;
    let input: Vec<char> = input.chars().collect();
    let mut results = Vec::new();
    let mut current = ParsedPatternFields::default();
    let mut budget = 4096usize;
    match_pattern_parts(
        &parts,
        &input,
        0,
        0,
        current.clone(),
        &mut results,
        &mut budget,
    );
    if results.is_empty() {
        Err(LocaleError::InvalidInput)
    } else {
        results.sort_by(|left, right| {
            let mut left_values: Vec<_> = left.values.iter().collect();
            let mut right_values: Vec<_> = right.values.iter().collect();
            left_values.sort_by_key(|(key, _)| **key);
            right_values.sort_by_key(|(key, _)| **key);
            left_values.cmp(&right_values)
        });
        results.dedup_by(|left, right| {
            left.values == right.values && left.two_digit_year == right.two_digit_year
        });
        current.values.clear();
        Ok(results)
    }
}

fn tokenize_pattern(pattern: &str) -> Result<Vec<LocalizedPatternPart>, LocaleError> {
    let mut parts = Vec::new();
    let mut chars = pattern.chars().peekable();
    let mut literal = String::new();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            if chars.peek() == Some(&'\'') {
                chars.next();
                literal.push('\'');
            } else {
                quoted = !quoted;
            }
            continue;
        }
        if !quoted && ch.is_ascii_alphabetic() {
            if !literal.is_empty() {
                parts.push(LocalizedPatternPart::Literal(std::mem::take(&mut literal)));
            }
            let mut width = 1;
            while chars.peek() == Some(&ch) {
                chars.next();
                width += 1;
            }
            parts.push(LocalizedPatternPart::Field(ch, width));
        } else {
            literal.push(ch);
        }
    }
    if quoted {
        return Err(LocaleError::DataUnavailable);
    }
    if !literal.is_empty() {
        parts.push(LocalizedPatternPart::Literal(literal));
    }
    Ok(parts)
}

fn match_pattern_parts(
    parts: &[LocalizedPatternPart],
    input: &[char],
    part_index: usize,
    input_index: usize,
    fields: ParsedPatternFields,
    results: &mut Vec<ParsedPatternFields>,
    budget: &mut usize,
) {
    if *budget == 0 || results.len() >= 256 {
        return;
    }
    *budget -= 1;
    if part_index == parts.len() {
        if input_index == input.len() {
            results.push(fields);
        }
        return;
    }
    match &parts[part_index] {
        LocalizedPatternPart::Literal(literal) => {
            let literal: Vec<char> = literal.chars().collect();
            if input.get(input_index..input_index + literal.len()) == Some(literal.as_slice()) {
                match_pattern_parts(
                    parts,
                    input,
                    part_index + 1,
                    input_index + literal.len(),
                    fields,
                    results,
                    budget,
                );
            }
        }
        LocalizedPatternPart::Field(symbol, width) => {
            if is_numeric_pattern_field(*symbol, *width) {
                let mut end = input_index;
                let mut value = 0u32;
                while end < input.len() {
                    let Some(digit) = unicode_decimal_digit(input[end]) else {
                        break;
                    };
                    value = match value
                        .checked_mul(10)
                        .and_then(|value| value.checked_add(u32::from(digit)))
                    {
                        Some(value) => value,
                        None => return,
                    };
                    end += 1;
                }
                let digit_count = end.saturating_sub(input_index);
                let (min, max) = numeric_width(*symbol, *width);
                if digit_count < min || digit_count > max {
                    return;
                }
                let key = canonical_field_symbol(*symbol);
                if fields.values.get(&key).is_some_and(|old| *old != value) {
                    return;
                }
                let mut next = fields;
                next.values.insert(key, value);
                if matches!(*symbol, 'H' | 'h' | 'K' | 'k') {
                    next.hour_symbol = Some(*symbol);
                }
                if *symbol == 'S' {
                    next.fraction_width = Some(*width);
                }
                if matches!(*symbol, 'y' | 'Y') && *width == 2 {
                    next.two_digit_year = true;
                }
                match_pattern_parts(parts, input, part_index + 1, end, next, results, budget);
            } else {
                if matches!(*symbol, 'a' | 'b' | 'B') {
                    let mut fields = fields;
                    fields.has_day_period = true;
                    match_text_field(
                        parts,
                        input,
                        part_index,
                        input_index,
                        fields,
                        results,
                        budget,
                    );
                    return;
                }
                match_text_field(
                    parts,
                    input,
                    part_index,
                    input_index,
                    fields,
                    results,
                    budget,
                );
            }
        }
    }
}

fn match_text_field(
    parts: &[LocalizedPatternPart],
    input: &[char],
    part_index: usize,
    input_index: usize,
    fields: ParsedPatternFields,
    results: &mut Vec<ParsedPatternFields>,
    budget: &mut usize,
) {
    let max_end = input.len();
    let fixed_end = next_literal_position(parts, part_index + 1)
        .and_then(|literal| find_literal(input, input_index, literal));
    if let Some(end) = fixed_end {
        match_pattern_parts(parts, input, part_index + 1, end, fields, results, budget);
    } else if !parts[part_index + 1..].is_empty() {
        for end in input_index..=max_end {
            match_pattern_parts(
                parts,
                input,
                part_index + 1,
                end,
                fields.clone(),
                results,
                budget,
            );
        }
    } else {
        match_pattern_parts(
            parts,
            input,
            part_index + 1,
            max_end,
            fields,
            results,
            budget,
        );
    }
}

fn next_literal_position(parts: &[LocalizedPatternPart], start: usize) -> Option<&str> {
    parts[start..].iter().find_map(|part| match part {
        LocalizedPatternPart::Literal(value) if !value.is_empty() => Some(value.as_str()),
        _ => None,
    })
}

fn find_literal(input: &[char], start: usize, literal: &str) -> Option<usize> {
    let literal: Vec<char> = literal.chars().collect();
    (start..=input.len().saturating_sub(literal.len()))
        .find(|index| input.get(*index..*index + literal.len()) == Some(literal.as_slice()))
}

fn is_numeric_pattern_field(symbol: char, width: usize) -> bool {
    matches!(
        symbol,
        'y' | 'Y' | 'u' | 'M' | 'L' | 'd' | 'e' | 'c' | 'H' | 'h' | 'K' | 'k' | 'm' | 's' | 'S'
    ) && (!matches!(symbol, 'M' | 'L' | 'e' | 'c') || width <= 2)
}

fn canonical_field_symbol(symbol: char) -> char {
    match symbol {
        'L' => 'M',
        'Y' | 'u' => 'y',
        _ => symbol,
    }
}

fn numeric_width(symbol: char, width: usize) -> (usize, usize) {
    if symbol == 'y' || symbol == 'Y' || symbol == 'u' {
        if width == 2 { (2, 2) } else { (1, 6) }
    } else if width >= 2 {
        (width, width)
    } else {
        (1, 2)
    }
}

fn unicode_decimal_digit(ch: char) -> Option<u8> {
    use unicode_general_category::{GeneralCategory, get_general_category};
    if get_general_category(ch) != GeneralCategory::DecimalNumber {
        return None;
    }
    let original = ch as u32;
    let mut first = original;
    while first > 0 {
        let previous = char::from_u32(first - 1)?;
        if get_general_category(previous) != GeneralCategory::DecimalNumber {
            break;
        }
        first -= 1;
    }
    Some(((original - first) % 10) as u8)
}

/// Formats a local clock time using the hour-cycle preference of its locale.
pub fn format_time(
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
    locale_tag: &str,
    style: DateTimeStyle,
) -> Result<String, LocaleError> {
    let locale = require_locale_data(locale_tag)?;
    let time = Time::try_new(hour, minute, second, nanosecond)
        .map_err(|_| LocaleError::InvalidTemporalValue)?;
    let preferences = DateTimeFormatterPreferences::from(locale);
    let rendered = match style {
        DateTimeStyle::Short => NoCalendarFormatter::try_new(preferences, T::short())
            .map_err(formatter_error)?
            .format(&time)
            .to_string(),
        DateTimeStyle::Medium => NoCalendarFormatter::try_new(preferences, T::medium())
            .map_err(formatter_error)?
            .format(&time)
            .to_string(),
        DateTimeStyle::Long => NoCalendarFormatter::try_new(preferences, T::long())
            .map_err(formatter_error)?
            .format(&time)
            .to_string(),
        DateTimeStyle::Full => NoCalendarFormatter::try_new(preferences, T::long())
            .map_err(formatter_error)?
            .format(&time)
            .to_string(),
    };
    Ok(rendered)
}

/// Formats a clock time with a stable named style suitable for language APIs.
pub fn format_time_named(
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
    locale_tag: &str,
    style: &str,
) -> Result<String, LocaleError> {
    format_time(
        hour,
        minute,
        second,
        nanosecond,
        locale_tag,
        parse_style(style)?,
    )
}

/// Formats a civil date-time under an explicit locale and CLDR style.
#[allow(clippy::too_many_arguments)] // Keeps the runtime's scalar ABI boundary explicit.
pub fn format_datetime(
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
    locale_tag: &str,
    style: DateTimeStyle,
) -> Result<String, LocaleError> {
    let locale = require_locale_data(locale_tag)?;
    let date =
        Date::try_new_iso(year, month, day).map_err(|_| LocaleError::InvalidTemporalValue)?;
    let time = Time::try_new(hour, minute, second, nanosecond)
        .map_err(|_| LocaleError::InvalidTemporalValue)?;
    let value = DateTime { date, time };
    let preferences = DateTimeFormatterPreferences::from(locale);
    let rendered = match style {
        DateTimeStyle::Short => DateTimeFormatter::<YMDT>::try_new(preferences, YMDT::short())
            .map_err(formatter_error)?
            .format(&value)
            .to_string(),
        DateTimeStyle::Medium => DateTimeFormatter::<YMDT>::try_new(preferences, YMDT::medium())
            .map_err(formatter_error)?
            .format(&value)
            .to_string(),
        DateTimeStyle::Long => DateTimeFormatter::<YMDT>::try_new(preferences, YMDT::long())
            .map_err(formatter_error)?
            .format(&value)
            .to_string(),
        DateTimeStyle::Full => DateTimeFormatter::<YMDET>::try_new(preferences, YMDET::long())
            .map_err(formatter_error)?
            .format(&value)
            .to_string(),
    };
    Ok(rendered)
}

/// Formats a civil date-time with a stable named style suitable for language APIs.
#[allow(clippy::too_many_arguments)] // Keeps the runtime's scalar ABI boundary explicit.
pub fn format_datetime_named(
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
    locale_tag: &str,
    style: &str,
) -> Result<String, LocaleError> {
    format_datetime(
        year,
        month,
        day,
        hour,
        minute,
        second,
        nanosecond,
        locale_tag,
        parse_style(style)?,
    )
}

/// Formats a timeline instant in a named IANA zone or fixed offset, including
/// a localized zone name or offset. Locale and IANA rules come from pinned
/// data; this function never consults host locale or time-zone settings.
pub fn format_zoned_datetime(
    unix_nanoseconds: i128,
    zone_identifier: &str,
    locale_tag: &str,
    style: DateTimeStyle,
) -> Result<String, LocaleError> {
    let locale = require_locale_data(locale_tag)?;
    let seconds = unix_nanoseconds.div_euclid(1_000_000_000);
    let nanoseconds = unix_nanoseconds.rem_euclid(1_000_000_000) as u32;
    let seconds = i64::try_from(seconds).map_err(|_| LocaleError::InvalidTemporalValue)?;
    let utc = chrono::DateTime::from_timestamp(seconds, nanoseconds)
        .ok_or(LocaleError::InvalidTemporalValue)?;
    let preferences = DateTimeFormatterPreferences::from(locale);
    let formatter = match style {
        DateTimeStyle::Short => {
            DateTimeFormatter::try_new(preferences, YMDT::short().with_zone(SpecificLong))
        }
        DateTimeStyle::Medium => {
            DateTimeFormatter::try_new(preferences, YMDT::medium().with_zone(SpecificLong))
        }
        DateTimeStyle::Long | DateTimeStyle::Full => {
            DateTimeFormatter::try_new(preferences, YMDT::long().with_zone(SpecificLong))
        }
    }
    .map_err(formatter_error)?;
    if let Ok(zone) = chrono_tz::Tz::from_str(zone_identifier) {
        return Ok(formatter.format(&utc.with_timezone(&zone)).to_string());
    }
    let offset = parse_fixed_offset(zone_identifier).ok_or(LocaleError::UnsupportedTimeZone)?;
    Ok(formatter.format(&utc.with_timezone(&offset)).to_string())
}

/// Resolves local civil fields with explicit DST policies, then formats the
/// resulting instant with the selected zone's localized name or offset.
#[allow(clippy::too_many_arguments)] // Mirrors the language API's explicit temporal fields.
pub fn format_local_datetime_in_zone(
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
    zone_identifier: &str,
    locale_tag: &str,
    style: &str,
    overlap: &str,
    gap: &str,
) -> Result<String, LocaleError> {
    let style = parse_style(style)?;
    let overlap = match overlap {
        "reject" => OverlapPolicy::Reject,
        "earlier" => OverlapPolicy::EarlierInstant,
        "later" => OverlapPolicy::LaterInstant,
        _ => return Err(LocaleError::InvalidDstPolicy),
    };
    let gap = match gap {
        "reject" => GapPolicy::Reject,
        "forward" => GapPolicy::ShiftForwardByGap,
        "backward" => GapPolicy::ShiftBackwardByGap,
        _ => return Err(LocaleError::InvalidDstPolicy),
    };
    let zone = match TimeZone::iana(zone_identifier) {
        Ok(zone) => zone,
        Err(_) => {
            let offset =
                parse_fixed_offset(zone_identifier).ok_or(LocaleError::UnsupportedTimeZone)?;
            TimeZone::fixed_offset_seconds(offset.local_minus_utc())
                .map_err(|_| LocaleError::UnsupportedTimeZone)?
        }
    };
    let date = chrono::NaiveDate::from_ymd_opt(year, u32::from(month), u32::from(day))
        .ok_or(LocaleError::InvalidTemporalValue)?;
    let time = chrono::NaiveTime::from_hms_nano_opt(
        u32::from(hour),
        u32::from(minute),
        u32::from(second),
        nanosecond,
    )
    .ok_or(LocaleError::InvalidTemporalValue)?;
    let value = ZonedDateTime::from_local(date.and_time(time), zone, DstPolicy { overlap, gap })
        .map_err(|error| match error {
            crate::zoned_temporal::TemporalError::AmbiguousLocalTime => {
                LocaleError::AmbiguousLocalTime
            }
            crate::zoned_temporal::TemporalError::NonexistentLocalTime => {
                LocaleError::NonexistentLocalTime
            }
            _ => LocaleError::InvalidTemporalValue,
        })?;
    format_zoned_datetime(
        value.instant().unix_nanoseconds(),
        &value.zone().canonical_identifier(),
        locale_tag,
        style,
    )
}

fn parse_fixed_offset(identifier: &str) -> Option<chrono::FixedOffset> {
    if identifier == "UTC" || identifier == "Z" {
        return chrono::FixedOffset::east_opt(0);
    }
    let (sign, value) = match identifier.as_bytes().first()? {
        b'+' => (1_i32, &identifier[1..]),
        b'-' => (-1_i32, &identifier[1..]),
        _ => return None,
    };
    let mut components = value.split(':');
    let hours = components.next()?.parse::<i32>().ok()?;
    let minutes = components.next()?.parse::<i32>().ok()?;
    let seconds = match components.next() {
        Some(value) => value.parse::<i32>().ok()?,
        None => 0,
    };
    if components.next().is_some() || hours > 23 || minutes > 59 || seconds > 59 {
        return None;
    }
    let total = sign * (hours * 3600 + minutes * 60 + seconds);
    chrono::FixedOffset::east_opt(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_tags_are_validated_and_canonicalized() {
        assert_eq!(canonicalize_bcp47("es-cl").unwrap(), "es-CL");
        assert_eq!(canonicalize_bcp47("zz-ZZ").unwrap(), "zz-ZZ");
        assert_eq!(
            canonicalize_bcp47("not_a_locale"),
            Err(LocaleError::InvalidTag)
        );
        assert_eq!(parse_style("invalid"), Err(LocaleError::InvalidStyle));
    }

    #[test]
    fn date_formatting_uses_the_explicit_locale() {
        let us = format_date(2026, 5, 17, "en-US", DateTimeStyle::Short).unwrap();
        let chile = format_date(2026, 5, 17, "es-CL", DateTimeStyle::Short).unwrap();
        let full = format_date(2026, 5, 17, "en-US", DateTimeStyle::Full).unwrap();
        let arabic = format_date(2026, 5, 17, "ar-EG-u-nu-arab", DateTimeStyle::Short).unwrap();
        assert_ne!(us, chile);
        assert_eq!(us, "5/17/26");
        assert_eq!(chile, "17-05-26");
        assert!(full.contains("Sunday"));
        assert!(arabic.contains('١'));
        assert_eq!(
            format_date(2026, 5, 17, "zz-ZZ", DateTimeStyle::Short),
            Err(LocaleError::UnsupportedLocale)
        );
    }

    #[test]
    fn localized_date_parsing_round_trips_cldr_styles_and_numbering_systems() {
        let expected = chrono::NaiveDate::from_ymd_opt(2026, 5, 17).unwrap();
        let expected_days = expected
            .signed_duration_since(chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
            .num_days();
        for (locale, style) in [
            ("en-US", "medium"),
            ("en-US", "full"),
            ("es-CL", "long"),
            ("ar-EG-u-nu-arab", "long"),
        ] {
            let formatted = format_date_named(2026, 5, 17, locale, style).unwrap();
            assert_eq!(
                parse_date_named(&formatted, locale, style),
                Ok(expected_days),
                "{locale} {style}: {formatted}"
            );
        }
        assert_eq!(
            parse_date_named("5/17/26", "en-US", "short"),
            Err(LocaleError::AmbiguousInput)
        );
        assert_eq!(
            parse_date_named("not a date", "en-US", "medium"),
            Err(LocaleError::InvalidInput)
        );
    }

    #[test]
    fn localized_time_and_datetime_parsing_round_trip_and_reject_ambiguity() {
        for (locale, style) in [
            ("en-US", "medium"),
            ("es-CL", "medium"),
            ("ar-EG-u-nu-arab", "medium"),
        ] {
            let formatted_time = format_time_named(16, 12, 20, 0, locale, style).unwrap();
            assert_eq!(
                parse_time_named(&formatted_time, locale, style),
                Ok(16 * 3_600_000_000_000_i64 + 12 * 60_000_000_000_i64 + 20 * 1_000_000_000_i64),
                "{locale} {style}: {formatted_time}"
            );

            let formatted_datetime =
                format_datetime_named(2026, 5, 17, 16, 12, 20, 0, locale, style).unwrap();
            assert_eq!(
                parse_datetime_named(&formatted_datetime, locale, style),
                Ok((2026, 5, 17, 16, 12, 20, 0)),
                "{locale} {style}: {formatted_datetime}"
            );
        }
        assert_eq!(
            parse_time_named("1:12 PM", "en-US", "short"),
            Err(LocaleError::InvalidInput)
        );
        assert_eq!(
            parse_datetime_named("not a date-time", "en-US", "medium"),
            Err(LocaleError::InvalidInput)
        );
    }

    #[test]
    fn datetime_uses_locale_hour_cycle_and_reports_invalid_values() {
        let us = format_time(16, 12, 20, 0, "en-US", DateTimeStyle::Medium).unwrap();
        let japan = format_time(16, 12, 20, 0, "ja-JP", DateTimeStyle::Medium).unwrap();
        assert!(us.contains("PM"));
        assert!(!japan.contains("PM"));
        assert_eq!(
            format_datetime(2026, 2, 30, 12, 0, 0, 0, "en", DateTimeStyle::Long),
            Err(LocaleError::InvalidTemporalValue)
        );
    }

    #[test]
    fn locale_data_versions_are_explicit() {
        assert_eq!(ICU4X_DATA_VERSION, "2.4.0");
        assert_eq!(CLDR_DATA_VERSION, "49");
    }

    #[test]
    fn zoned_formatting_uses_the_pinned_zone_name_data() {
        let instant = 1_726_011_440_123_456_789_i128;
        let tokyo =
            format_zoned_datetime(instant, "Asia/Tokyo", "ja-JP", DateTimeStyle::Medium).unwrap();
        assert!(tokyo.contains("日本標準時"), "{tokyo}");
        assert_eq!(
            format_zoned_datetime(instant, "Not/AZone", "ja-JP", DateTimeStyle::Medium),
            Err(LocaleError::UnsupportedTimeZone)
        );
        assert_eq!(
            format_zoned_datetime(i128::MAX, "Asia/Tokyo", "ja-JP", DateTimeStyle::Medium),
            Err(LocaleError::InvalidTemporalValue)
        );

        let winter = format_zoned_datetime(
            1_704_067_200_000_000_000,
            "America/New_York",
            "en-US",
            DateTimeStyle::Long,
        )
        .unwrap();
        let summer = format_zoned_datetime(
            1_719_792_000_000_000_000,
            "America/New_York",
            "en-US",
            DateTimeStyle::Long,
        )
        .unwrap();
        assert!(winter.contains("Eastern Standard Time"), "{winter}");
        assert!(summer.contains("Eastern Daylight Time"), "{summer}");

        let fixed = format_zoned_datetime(instant, "+09:00", "en-US", DateTimeStyle::Long).unwrap();
        assert!(fixed.contains("GMT+09:00"), "{fixed}");
        assert_eq!(
            format_zoned_datetime(instant, "+24:00", "en-US", DateTimeStyle::Long),
            Err(LocaleError::UnsupportedTimeZone)
        );
    }

    #[test]
    fn local_zone_formatting_requires_explicit_dst_resolution() {
        let overlap = format_local_datetime_in_zone(
            2024,
            11,
            3,
            1,
            30,
            0,
            0,
            "America/New_York",
            "en-US",
            "long",
            "reject",
            "reject",
        );
        assert_eq!(overlap, Err(LocaleError::AmbiguousLocalTime));
        let earlier = format_local_datetime_in_zone(
            2024,
            11,
            3,
            1,
            30,
            0,
            0,
            "America/New_York",
            "en-US",
            "long",
            "earlier",
            "reject",
        )
        .unwrap();
        let later = format_local_datetime_in_zone(
            2024,
            11,
            3,
            1,
            30,
            0,
            0,
            "America/New_York",
            "en-US",
            "long",
            "later",
            "reject",
        )
        .unwrap();
        assert!(earlier.contains("Eastern Daylight Time"), "{earlier}");
        assert!(later.contains("Eastern Standard Time"), "{later}");

        assert_eq!(
            format_local_datetime_in_zone(
                2024,
                3,
                10,
                2,
                30,
                0,
                0,
                "America/New_York",
                "en-US",
                "long",
                "reject",
                "reject",
            ),
            Err(LocaleError::NonexistentLocalTime)
        );
        let shifted = format_local_datetime_in_zone(
            2024,
            3,
            10,
            2,
            30,
            0,
            0,
            "America/New_York",
            "en-US",
            "long",
            "reject",
            "forward",
        )
        .unwrap();
        assert!(shifted.contains("3:30:00"), "{shifted}");
        assert!(shifted.contains("Eastern Daylight Time"), "{shifted}");
        assert_eq!(
            format_local_datetime_in_zone(
                2024,
                3,
                10,
                2,
                30,
                0,
                0,
                "America/New_York",
                "en-US",
                "long",
                "invalid",
                "reject",
            ),
            Err(LocaleError::InvalidDstPolicy)
        );
    }
}
