//! Platform-independent zoned temporal primitives.
//!
//! This module owns the civil-to-instant rules used by the zone-aware
//! temporal API. Named zones come only from the pinned `chrono-tz` database;
//! fixed offsets retain a distinct identity and never consult host settings.

use chrono::{
    DateTime, Datelike, Duration as CalendarDuration, FixedOffset, LocalResult, NaiveDateTime,
    Offset, TimeZone as ChronoTimeZone, Timelike, Utc,
};
use chrono_tz::{OffsetComponents, OffsetName, Tz};
use std::{collections::HashMap, str::FromStr, sync::OnceLock};

const NANOS_PER_SECOND: i128 = 1_000_000_000;
const MAX_GAP_SEARCH_SECONDS: i64 = 2 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalInstantResult {
    Single(Instant),
    Ambiguous(Instant, Instant),
    None,
}

/// The exact zone-rule database version compiled into this runtime.
pub const IANA_TZDB_VERSION: &str = chrono_tz::IANA_TZDB_VERSION;

/// A point on the Unix timeline, represented as signed nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(i128);

impl Instant {
    pub fn from_unix_nanoseconds(value: i128) -> Result<Self, TemporalError> {
        let instant = Self(value);
        instant.to_utc()?;
        Ok(instant)
    }

    pub fn from_unix_milliseconds(value: i64) -> Result<Self, TemporalError> {
        Self::from_unix_nanoseconds(
            i128::from(value)
                .checked_mul(1_000_000)
                .ok_or(TemporalError::OutOfRange)?,
        )
    }

    pub fn from_unix_seconds(value: i64) -> Result<Self, TemporalError> {
        Self::from_unix_nanoseconds(
            i128::from(value)
                .checked_mul(NANOS_PER_SECOND)
                .ok_or(TemporalError::OutOfRange)?,
        )
    }

    pub fn parse_rfc3339(value: &str) -> Result<Self, TemporalError> {
        let parsed = DateTime::parse_from_rfc3339(value).map_err(|_| TemporalError::Parse)?;
        Self::from_parts(parsed.timestamp(), parsed.timestamp_subsec_nanos())
    }

    pub const fn unix_nanoseconds(self) -> i128 {
        self.0
    }

    /// Canonical UTC ISO 8601 representation with an explicit `Z` offset.
    pub fn to_rfc3339(self) -> Result<String, TemporalError> {
        Ok(self.to_utc()?.to_rfc3339())
    }

    pub fn checked_add_nanoseconds(self, delta: i128) -> Result<Self, TemporalError> {
        Self::from_unix_nanoseconds(self.0.checked_add(delta).ok_or(TemporalError::OutOfRange)?)
    }

    pub fn checked_sub(self, other: Self) -> Result<i128, TemporalError> {
        self.0.checked_sub(other.0).ok_or(TemporalError::OutOfRange)
    }

    fn from_parts(seconds: i64, nanos: u32) -> Result<Self, TemporalError> {
        let value = i128::from(seconds)
            .checked_mul(NANOS_PER_SECOND)
            .and_then(|seconds| seconds.checked_add(i128::from(nanos)))
            .ok_or(TemporalError::OutOfRange)?;
        Self::from_unix_nanoseconds(value)
    }

    fn to_utc(self) -> Result<DateTime<Utc>, TemporalError> {
        let seconds = self.0.div_euclid(NANOS_PER_SECOND);
        let nanos = self.0.rem_euclid(NANOS_PER_SECOND) as u32;
        let seconds = i64::try_from(seconds).map_err(|_| TemporalError::OutOfRange)?;
        DateTime::from_timestamp(seconds, nanos).ok_or(TemporalError::OutOfRange)
    }
}

/// Named IANA rules and fixed offsets are intentionally separate variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeZone(ZoneKind);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ZoneKind {
    Iana(Tz),
    Fixed(FixedOffset),
}

impl TimeZone {
    /// Parses an IANA identifier, `UTC`, or a fixed ISO offset (`±HH:MM` or
    /// `±HH:MM:SS`). Named IANA zones and fixed offsets retain distinct
    /// identities after parsing.
    pub fn parse_identifier(identifier: &str) -> Result<Self, TemporalError> {
        if identifier == "UTC" || identifier == "Z" {
            return Ok(Self::utc());
        }
        if identifier.starts_with('+') || identifier.starts_with('-') {
            return parse_fixed_offset(identifier);
        }
        Self::iana(identifier)
    }

    pub fn iana(identifier: &str) -> Result<Self, TemporalError> {
        let parsed = Tz::from_str(identifier)
            .map_err(|_| TemporalError::UnknownTimeZone(identifier.to_owned()))?;
        let canonical = canonical_iana_name(parsed.name());
        Tz::from_str(canonical)
            .map(|zone| Self(ZoneKind::Iana(zone)))
            .map_err(|_| TemporalError::UnknownTimeZone(identifier.to_owned()))
    }

    pub fn fixed_offset_seconds(seconds: i32) -> Result<Self, TemporalError> {
        FixedOffset::east_opt(seconds)
            .map(|offset| Self(ZoneKind::Fixed(offset)))
            .ok_or(TemporalError::InvalidFixedOffset)
    }

    pub fn utc() -> Self {
        Self(ZoneKind::Fixed(
            FixedOffset::east_opt(0).expect("zero is a valid UTC offset"),
        ))
    }

    pub fn canonical_identifier(self) -> String {
        match self.0 {
            ZoneKind::Iana(zone) => zone.name().to_owned(),
            ZoneKind::Fixed(offset) if offset.local_minus_utc() == 0 => "UTC".to_owned(),
            ZoneKind::Fixed(offset) => {
                let seconds = offset.local_minus_utc();
                let sign = if seconds < 0 { '-' } else { '+' };
                let magnitude = seconds.unsigned_abs();
                if magnitude % 60 == 0 {
                    format!("{sign}{:02}:{:02}", magnitude / 3600, magnitude % 3600 / 60)
                } else {
                    format!(
                        "{sign}{:02}:{:02}:{:02}",
                        magnitude / 3600,
                        magnitude % 3600 / 60,
                        magnitude % 60
                    )
                }
            }
        }
    }

    /// The exact IANA rule-data release compiled into this runtime.
    pub const fn database_version() -> &'static str {
        IANA_TZDB_VERSION
    }

    /// The UTC offset in effect at `instant`, in seconds east of UTC.
    pub fn offset_seconds_at(self, instant: Instant) -> Result<i32, TemporalError> {
        let utc = instant.to_utc()?;
        let offset = match self.0 {
            ZoneKind::Iana(zone) => zone
                .from_utc_datetime(&utc.naive_utc())
                .offset()
                .fix()
                .local_minus_utc(),
            ZoneKind::Fixed(offset) => offset.local_minus_utc(),
        };
        Ok(offset)
    }

    /// Whether daylight-saving adjustment is active at `instant`.
    pub fn is_dst_at(self, instant: Instant) -> Result<bool, TemporalError> {
        let utc = instant.to_utc()?;
        Ok(match self.0 {
            ZoneKind::Iana(zone) => {
                zone.offset_from_utc_datetime(&utc.naive_utc())
                    .dst_offset()
                    .num_seconds()
                    != 0
            }
            ZoneKind::Fixed(_) => false,
        })
    }

    /// The abbreviation in effect at `instant`. Fixed offsets use a stable
    /// `UTC±HH:MM[:SS]` label because they have no IANA abbreviation.
    pub fn abbreviation_at(self, instant: Instant) -> Result<String, TemporalError> {
        let utc = instant.to_utc()?;
        Ok(match self.0 {
            ZoneKind::Iana(zone) => zone
                .offset_from_utc_datetime(&utc.naive_utc())
                .abbreviation()
                .unwrap_or(zone.name())
                .to_owned(),
            ZoneKind::Fixed(offset) => format!(
                "UTC{}",
                Self(ZoneKind::Fixed(offset)).canonical_identifier()
            ),
        })
    }

    fn local_result(self, local: &NaiveDateTime) -> Result<LocalInstantResult, TemporalError> {
        fn convert<T: chrono::TimeZone>(
            result: LocalResult<DateTime<T>>,
        ) -> Result<LocalInstantResult, TemporalError> {
            match result {
                LocalResult::Single(value) => Ok(LocalInstantResult::Single(Instant::from_parts(
                    value.timestamp(),
                    value.timestamp_subsec_nanos(),
                )?)),
                LocalResult::Ambiguous(first, second) => Ok(LocalInstantResult::Ambiguous(
                    Instant::from_parts(first.timestamp(), first.timestamp_subsec_nanos())?,
                    Instant::from_parts(second.timestamp(), second.timestamp_subsec_nanos())?,
                )),
                LocalResult::None => Ok(LocalInstantResult::None),
            }
        }
        match self.0 {
            ZoneKind::Iana(zone) => convert(zone.from_local_datetime(local)),
            ZoneKind::Fixed(offset) => convert(offset.from_local_datetime(local)),
        }
    }

    fn offset_at_local(self, local: &NaiveDateTime) -> Option<i32> {
        fn choose<Tz: chrono::TimeZone>(result: LocalResult<DateTime<Tz>>) -> Option<i32>
        where
            Tz::Offset: Offset,
        {
            match result {
                LocalResult::Single(value) | LocalResult::Ambiguous(value, _) => {
                    Some(value.offset().fix().local_minus_utc())
                }
                LocalResult::None => None,
            }
        }
        match self.0 {
            ZoneKind::Iana(zone) => choose(zone.from_local_datetime(local)),
            ZoneKind::Fixed(offset) => choose(offset.from_local_datetime(local)),
        }
    }

    fn from_instant(self, instant: Instant) -> Result<NaiveDateTime, TemporalError> {
        let utc = instant.to_utc()?;
        Ok(match self.0 {
            ZoneKind::Iana(zone) => zone.from_utc_datetime(&utc.naive_utc()).naive_local(),
            ZoneKind::Fixed(offset) => offset.from_utc_datetime(&utc.naive_utc()).naive_local(),
        })
    }
}

fn parse_fixed_offset(identifier: &str) -> Result<TimeZone, TemporalError> {
    let (sign, components) = match identifier.as_bytes().first() {
        Some(b'+') => (1_i32, &identifier[1..]),
        Some(b'-') => (-1_i32, &identifier[1..]),
        _ => return Err(TemporalError::InvalidFixedOffset),
    };
    let parts = components.split(':').collect::<Vec<_>>();
    if !(parts.len() == 2 || parts.len() == 3)
        || parts
            .iter()
            .any(|part| part.len() != 2 || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(TemporalError::InvalidFixedOffset);
    }
    let hours = parts[0]
        .parse::<i32>()
        .map_err(|_| TemporalError::InvalidFixedOffset)?;
    let minutes = parts[1]
        .parse::<i32>()
        .map_err(|_| TemporalError::InvalidFixedOffset)?;
    let seconds = parts
        .get(2)
        .map(|part| part.parse::<i32>())
        .transpose()
        .map_err(|_| TemporalError::InvalidFixedOffset)?
        .unwrap_or(0);
    if hours > 23 || minutes > 59 || seconds > 59 {
        return Err(TemporalError::InvalidFixedOffset);
    }
    let total = sign * (hours * 3_600 + minutes * 60 + seconds);
    TimeZone::fixed_offset_seconds(total)
}

fn canonical_iana_name(identifier: &str) -> &str {
    static LINKS: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    let links = LINKS.get_or_init(|| {
        include_str!("tzdb_links_2025b.txt")
            .lines()
            .filter_map(|line| {
                let (alias, target) = line.split_once('\t')?;
                if alias.starts_with('#') {
                    return None;
                }
                Some((alias, target))
            })
            .collect()
    });
    let mut current = identifier;
    for _ in 0..32 {
        let Some(target) = links.get(current) else {
            break;
        };
        current = target;
    }
    current
}

/// Explicit choices for resolving repeated or nonexistent local times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DstPolicy {
    pub overlap: OverlapPolicy,
    pub gap: GapPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlapPolicy {
    Reject,
    EarlierInstant,
    LaterInstant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GapPolicy {
    Reject,
    ShiftForwardByGap,
    ShiftBackwardByGap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemporalError {
    Parse,
    UnknownTimeZone(String),
    InvalidFixedOffset,
    AmbiguousLocalTime,
    NonexistentLocalTime,
    InvalidDstPolicy,
    TimeShiftKindMismatch,
    OutOfRange,
}

impl TemporalError {
    /// Stable compiler/runtime boundary code used to preserve the concrete
    /// temporal failure subtype across native and interpreter execution.
    pub const fn stable_code(&self) -> i32 {
        match self {
            Self::Parse => 1,
            Self::UnknownTimeZone(_) => 2,
            Self::InvalidFixedOffset => 3,
            Self::AmbiguousLocalTime => 4,
            Self::NonexistentLocalTime => 5,
            Self::InvalidDstPolicy => 6,
            Self::TimeShiftKindMismatch => 7,
            Self::OutOfRange => 8,
        }
    }
}

impl std::fmt::Display for TemporalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse => formatter.write_str("invalid ISO instant"),
            Self::UnknownTimeZone(identifier) => {
                write!(
                    formatter,
                    "unknown IANA time-zone identifier `{identifier}`"
                )
            }
            Self::InvalidFixedOffset => formatter.write_str("fixed UTC offset is out of range"),
            Self::AmbiguousLocalTime => formatter.write_str("local time is ambiguous"),
            Self::NonexistentLocalTime => formatter.write_str("local time does not exist"),
            Self::InvalidDstPolicy => {
                formatter.write_str("invalid daylight-saving resolution policy")
            }
            Self::TimeShiftKindMismatch => {
                formatter.write_str("clock TimeShift cannot be applied to ZonedDateTime")
            }
            Self::OutOfRange => formatter.write_str("instant is outside the supported range"),
        }
    }
}

impl std::error::Error for TemporalError {}

/// A timeline instant together with its presentation zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ZonedDateTime {
    instant: Instant,
    zone: TimeZone,
}

impl ZonedDateTime {
    /// Resolves a language-packed civil DateTime using explicit DST policy
    /// values (`overlap`: reject/earlier/later; `gap`: reject/forward/backward).
    pub fn from_packed_local_datetime(
        value: i128,
        zone: TimeZone,
        policy: DstPolicy,
    ) -> Result<Self, TemporalError> {
        let days =
            crate::temporal::datetime_days_checked(value).ok_or(TemporalError::OutOfRange)?;
        let (year, month, day) = crate::temporal::civil_from_days(days);
        let nanos = crate::temporal::datetime_nanos(value);
        let second_of_day = nanos / 1_000_000_000;
        let nanosecond = (nanos % 1_000_000_000) as u32;
        let hour = (second_of_day / 3_600) as u32;
        let minute = (second_of_day % 3_600 / 60) as u32;
        let second = (second_of_day % 60) as u32;
        let date = chrono::NaiveDate::from_ymd_opt(year, month as u32, day as u32)
            .ok_or(TemporalError::OutOfRange)?;
        let time = chrono::NaiveTime::from_hms_nano_opt(hour, minute, second, nanosecond)
            .ok_or(TemporalError::OutOfRange)?;
        Self::from_local(NaiveDateTime::new(date, time), zone, policy)
    }

    /// Parses RFC 3339 date-time text with a bracketed zone identifier, such
    /// as `2024-03-10T03:30:00-04:00[America/New_York]`. The explicit offset
    /// must agree with the pinned zone rules at that instant.
    pub fn parse_rfc3339(value: &str) -> Result<Self, TemporalError> {
        let (timestamp, identifier) = value
            .strip_suffix(']')
            .and_then(|value| value.rsplit_once('['))
            .ok_or(TemporalError::Parse)?;
        let parsed = DateTime::parse_from_rfc3339(timestamp).map_err(|_| TemporalError::Parse)?;
        let instant = Instant::from_parts(parsed.timestamp(), parsed.timestamp_subsec_nanos())?;
        let zone = TimeZone::parse_identifier(identifier)?;
        if zone.offset_seconds_at(instant)? != parsed.offset().local_minus_utc() {
            return Err(TemporalError::Parse);
        }
        Self::from_instant(instant, zone)
    }

    /// Compact opaque representation used by the native and interpreted
    /// language runtimes. The zone identifier is canonical, and the numeric
    /// instant is exact epoch nanoseconds.
    pub fn encode_runtime_value(self) -> String {
        format!(
            "{}|{}",
            self.instant.unix_nanoseconds(),
            self.zone.canonical_identifier()
        )
    }

    /// Parses the opaque runtime representation produced by
    /// [`Self::encode_runtime_value`].
    pub fn parse_runtime_value(value: &str) -> Result<Self, TemporalError> {
        let (instant, zone) = value.split_once('|').ok_or(TemporalError::Parse)?;
        let nanos = instant.parse::<i128>().map_err(|_| TemporalError::Parse)?;
        let instant = Instant::from_unix_nanoseconds(nanos)?;
        let zone = TimeZone::parse_identifier(zone)?;
        Self::from_instant(instant, zone)
    }

    pub fn from_instant(instant: Instant, zone: TimeZone) -> Result<Self, TemporalError> {
        zone.from_instant(instant)?;
        Ok(Self { instant, zone })
    }

    pub fn from_local(
        local: NaiveDateTime,
        zone: TimeZone,
        policy: DstPolicy,
    ) -> Result<Self, TemporalError> {
        match zone.local_result(&local)? {
            LocalInstantResult::Single(instant) => Self::from_instant(instant, zone),
            LocalInstantResult::Ambiguous(first, second) => {
                let (earlier, later) = if first <= second {
                    (first, second)
                } else {
                    (second, first)
                };
                let selected = match policy.overlap {
                    OverlapPolicy::Reject => return Err(TemporalError::AmbiguousLocalTime),
                    OverlapPolicy::EarlierInstant => earlier,
                    OverlapPolicy::LaterInstant => later,
                };
                Self::from_instant(selected, zone)
            }
            LocalInstantResult::None => {
                let shift = match policy.gap {
                    GapPolicy::Reject => return Err(TemporalError::NonexistentLocalTime),
                    GapPolicy::ShiftForwardByGap => 1,
                    GapPolicy::ShiftBackwardByGap => -1,
                };
                let gap_seconds = Self::gap_size_seconds(local, zone)?;
                let shifted = local
                    .checked_add_signed(CalendarDuration::seconds(gap_seconds * shift))
                    .ok_or(TemporalError::OutOfRange)?;
                match zone.local_result(&shifted)? {
                    LocalInstantResult::Single(instant) => Self::from_instant(instant, zone),
                    LocalInstantResult::Ambiguous(first, second) => {
                        let selected = match policy.overlap {
                            OverlapPolicy::Reject => return Err(TemporalError::AmbiguousLocalTime),
                            OverlapPolicy::EarlierInstant => first.min(second),
                            OverlapPolicy::LaterInstant => first.max(second),
                        };
                        Self::from_instant(selected, zone)
                    }
                    LocalInstantResult::None => Err(TemporalError::NonexistentLocalTime),
                }
            }
        }
    }

    pub const fn instant(self) -> Instant {
        self.instant
    }

    pub const fn zone(self) -> TimeZone {
        self.zone
    }

    pub fn local_datetime(self) -> Result<NaiveDateTime, TemporalError> {
        self.zone.from_instant(self.instant)
    }

    /// Projects the local civil value into the language's packed DateTime
    /// representation: epoch-relative day count times nanoseconds per day,
    /// plus nanoseconds since local midnight.
    pub fn local_datetime_unix_nanoseconds(self) -> Result<i128, TemporalError> {
        let local = self.local_datetime()?;
        let days = crate::temporal::days_from_civil(
            local.year(),
            local.month() as i32,
            local.day() as i32,
        );
        let nanos = i64::from(local.num_seconds_from_midnight()) * 1_000_000_000
            + i64::from(local.nanosecond());
        Ok(crate::temporal::datetime_pack(days, nanos))
    }

    pub fn with_zone(self, zone: TimeZone) -> Result<Self, TemporalError> {
        Self::from_instant(self.instant, zone)
    }

    fn gap_size_seconds(local: NaiveDateTime, zone: TimeZone) -> Result<i64, TemporalError> {
        let mut before = None;
        let mut after = None;
        for distance in 1..=MAX_GAP_SEARCH_SECONDS {
            if before.is_none() {
                let candidate = local
                    .checked_sub_signed(CalendarDuration::seconds(distance))
                    .ok_or(TemporalError::OutOfRange)?;
                before = zone.offset_at_local(&candidate);
            }
            if after.is_none() {
                let candidate = local
                    .checked_add_signed(CalendarDuration::seconds(distance))
                    .ok_or(TemporalError::OutOfRange)?;
                after = zone.offset_at_local(&candidate);
            }
            if let (Some(before), Some(after)) = (before, after) {
                let gap = after - before;
                if gap > 0 {
                    return Ok(i64::from(gap));
                }
                return Err(TemporalError::NonexistentLocalTime);
            }
        }
        Err(TemporalError::NonexistentLocalTime)
    }
}

/// A calendar-relative shift. Its length is defined only when applied to a
/// civil date; month ends clamp to the final day of the target month.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Period {
    years: i32,
    months: i32,
    weeks: i32,
    days: i32,
}

impl Period {
    pub const fn new(years: i32, months: i32, weeks: i32, days: i32) -> Self {
        Self {
            years,
            months,
            weeks,
            days,
        }
    }

    /// Packs the four signed components into a canonical 128-bit language
    /// scalar. Each component occupies one two's-complement `i32` lane.
    pub const fn encode(self) -> i128 {
        (((self.years as u32 as u128) << 96)
            | ((self.months as u32 as u128) << 64)
            | ((self.weeks as u32 as u128) << 32)
            | self.days as u32 as u128) as i128
    }

    /// Decodes the canonical packed language representation.
    pub const fn decode(value: i128) -> Self {
        let bits = value as u128;
        Self::new(
            ((bits >> 96) as u32) as i32,
            ((bits >> 64) as u32) as i32,
            ((bits >> 32) as u32) as i32,
            (bits as u32) as i32,
        )
    }

    pub fn parse_iso(text: &str) -> Result<Self, TemporalError> {
        let body = text.strip_prefix('P').ok_or(TemporalError::Parse)?;
        if body.is_empty() {
            return Err(TemporalError::Parse);
        }
        let mut values = [0_i32; 4];
        let mut seen = [false; 4];
        let mut position = 0;
        let mut time_section = false;
        while position < body.len() {
            let rest = &body[position..];
            let (number_end, number) = parse_signed_i32_prefix(rest)?;
            if number_end >= rest.len() {
                return Err(TemporalError::Parse);
            }
            let designator = rest.as_bytes()[number_end] as char;
            let index = match designator {
                'Y' if !time_section => 0,
                'M' if !time_section => 1,
                'W' if !time_section => 2,
                'D' if !time_section => 3,
                'T' if number_end == 0 && !time_section => {
                    time_section = true;
                    position += 1;
                    continue;
                }
                // Period has no hour/minute/second fields. Rejecting them
                // avoids silently discarding exact elapsed quantities.
                _ => return Err(TemporalError::Parse),
            };
            if seen[index] {
                return Err(TemporalError::Parse);
            }
            seen[index] = true;
            values[index] = number;
            position += number_end + 1;
        }
        if time_section || seen.iter().all(|value| !value) {
            return Err(TemporalError::Parse);
        }
        Ok(Self::new(values[0], values[1], values[2], values[3]))
    }

    pub fn to_iso_string(self) -> String {
        let mut value = String::from("P");
        for (amount, designator) in [
            (self.years, 'Y'),
            (self.months, 'M'),
            (self.weeks, 'W'),
            (self.days, 'D'),
        ] {
            if amount != 0 {
                value.push_str(&amount.to_string());
                value.push(designator);
            }
        }
        if value == "P" {
            value.push_str("0D");
        }
        value
    }

    pub fn checked_add(self, other: Self) -> Result<Self, TemporalError> {
        Ok(Self::new(
            self.years
                .checked_add(other.years)
                .ok_or(TemporalError::OutOfRange)?,
            self.months
                .checked_add(other.months)
                .ok_or(TemporalError::OutOfRange)?,
            self.weeks
                .checked_add(other.weeks)
                .ok_or(TemporalError::OutOfRange)?,
            self.days
                .checked_add(other.days)
                .ok_or(TemporalError::OutOfRange)?,
        ))
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, TemporalError> {
        Ok(Self::new(
            self.years
                .checked_sub(other.years)
                .ok_or(TemporalError::OutOfRange)?,
            self.months
                .checked_sub(other.months)
                .ok_or(TemporalError::OutOfRange)?,
            self.weeks
                .checked_sub(other.weeks)
                .ok_or(TemporalError::OutOfRange)?,
            self.days
                .checked_sub(other.days)
                .ok_or(TemporalError::OutOfRange)?,
        ))
    }

    pub fn checked_mul(self, factor: i32) -> Result<Self, TemporalError> {
        Ok(Self::new(
            self.years
                .checked_mul(factor)
                .ok_or(TemporalError::OutOfRange)?,
            self.months
                .checked_mul(factor)
                .ok_or(TemporalError::OutOfRange)?,
            self.weeks
                .checked_mul(factor)
                .ok_or(TemporalError::OutOfRange)?,
            self.days
                .checked_mul(factor)
                .ok_or(TemporalError::OutOfRange)?,
        ))
    }

    pub const fn years(self) -> i32 {
        self.years
    }

    pub const fn months(self) -> i32 {
        self.months
    }

    pub const fn weeks(self) -> i32 {
        self.weeks
    }

    pub const fn days(self) -> i32 {
        self.days
    }

    pub fn checked_neg(self) -> Result<Self, TemporalError> {
        Ok(Self::new(
            self.years.checked_neg().ok_or(TemporalError::OutOfRange)?,
            self.months.checked_neg().ok_or(TemporalError::OutOfRange)?,
            self.weeks.checked_neg().ok_or(TemporalError::OutOfRange)?,
            self.days.checked_neg().ok_or(TemporalError::OutOfRange)?,
        ))
    }

    pub fn add_to(self, local: NaiveDateTime) -> Result<NaiveDateTime, TemporalError> {
        let date = local.date();
        let month_index = i64::from(date.year())
            .checked_mul(12)
            .and_then(|year| year.checked_add(i64::from(date.month0())))
            .and_then(|months| months.checked_add(i64::from(self.years).checked_mul(12)?))
            .and_then(|months| months.checked_add(i64::from(self.months)))
            .ok_or(TemporalError::OutOfRange)?;
        let year =
            i32::try_from(month_index.div_euclid(12)).map_err(|_| TemporalError::OutOfRange)?;
        let month =
            u32::try_from(month_index.rem_euclid(12) + 1).map_err(|_| TemporalError::OutOfRange)?;
        let last_day = days_in_month(year, month).ok_or(TemporalError::OutOfRange)?;
        let target_date = chrono::NaiveDate::from_ymd_opt(year, month, date.day().min(last_day))
            .ok_or(TemporalError::OutOfRange)?;
        let week_days = i64::from(self.weeks)
            .checked_mul(7)
            .ok_or(TemporalError::OutOfRange)?;
        let day_delta = week_days
            .checked_add(i64::from(self.days))
            .ok_or(TemporalError::OutOfRange)?;
        let target_date = target_date
            .checked_add_signed(CalendarDuration::days(day_delta))
            .ok_or(TemporalError::OutOfRange)?;
        Ok(target_date.and_time(local.time()))
    }
}

fn parse_signed_i32_prefix(text: &str) -> Result<(usize, i32), TemporalError> {
    let bytes = text.as_bytes();
    let mut end = usize::from(matches!(bytes.first(), Some(b'+' | b'-')));
    let digits_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if end == digits_start {
        return Err(TemporalError::Parse);
    }
    let value = text[..end].parse().map_err(|_| TemporalError::OutOfRange)?;
    Ok((end, value))
}

/// A tagged temporal shift. Clock shifts retain their wrapped time and day
/// carry; calendar shifts preserve calendar and elapsed-time parts separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeShift {
    Clock {
        time_nanoseconds: i64,
        day_offset: i32,
        duration_nanoseconds: i128,
    },
    Calendar {
        period: Period,
        duration_nanoseconds: i128,
        dst_policy: DstPolicy,
    },
}

impl TimeShift {
    pub const fn new(period: Period, duration_nanoseconds: i128, dst_policy: DstPolicy) -> Self {
        Self::Calendar {
            period,
            duration_nanoseconds,
            dst_policy,
        }
    }

    pub fn clock(time_nanoseconds: i64, duration_nanoseconds: i128) -> Result<Self, TemporalError> {
        let total = i128::from(time_nanoseconds)
            .checked_add(duration_nanoseconds)
            .ok_or(TemporalError::OutOfRange)?;
        let day_offset = i32::try_from(total.div_euclid(crate::temporal::NANOS_PER_DAY as i128))
            .map_err(|_| TemporalError::OutOfRange)?;
        let time_nanoseconds =
            i64::try_from(total.rem_euclid(crate::temporal::NANOS_PER_DAY as i128))
                .map_err(|_| TemporalError::OutOfRange)?;
        Ok(Self::Clock {
            time_nanoseconds,
            day_offset,
            duration_nanoseconds,
        })
    }

    pub const fn kind(self) -> &'static str {
        match self {
            Self::Clock { .. } => "clock",
            Self::Calendar { .. } => "calendar",
        }
    }

    pub const fn duration_nanoseconds(self) -> i128 {
        match self {
            Self::Clock {
                duration_nanoseconds,
                ..
            }
            | Self::Calendar {
                duration_nanoseconds,
                ..
            } => duration_nanoseconds,
        }
    }

    pub const fn time_nanoseconds(self) -> Option<i64> {
        match self {
            Self::Clock {
                time_nanoseconds, ..
            } => Some(time_nanoseconds),
            Self::Calendar { .. } => None,
        }
    }

    pub const fn day_offset(self) -> Option<i32> {
        match self {
            Self::Clock { day_offset, .. } => Some(day_offset),
            Self::Calendar { .. } => None,
        }
    }

    pub const fn period(self) -> Option<Period> {
        match self {
            Self::Clock { .. } => None,
            Self::Calendar { period, .. } => Some(period),
        }
    }

    pub const fn dst_policy(self) -> Option<DstPolicy> {
        match self {
            Self::Clock { .. } => None,
            Self::Calendar { dst_policy, .. } => Some(dst_policy),
        }
    }

    /// Stable private wire form used by the native runtime and interpreter.
    /// It is not part of the source-level serialization contract.
    pub fn to_runtime_token(self) -> String {
        match self {
            Self::Clock {
                time_nanoseconds,
                day_offset,
                duration_nanoseconds,
            } => format!("C|{time_nanoseconds}|{day_offset}|{duration_nanoseconds}"),
            Self::Calendar {
                period,
                duration_nanoseconds,
                dst_policy,
            } => format!(
                "K|{}|{duration_nanoseconds}|{}|{}",
                period.to_iso_string(),
                overlap_token(dst_policy.overlap),
                gap_token(dst_policy.gap),
            ),
        }
    }

    /// Stable source-facing representation. The private runtime token is
    /// intentionally kept out of user output and serialized text.
    pub fn to_display_string(self) -> String {
        match self {
            Self::Clock {
                time_nanoseconds,
                day_offset,
                duration_nanoseconds,
            } => {
                let hours = time_nanoseconds / 3_600_000_000_000;
                let minutes = time_nanoseconds / 60_000_000_000 % 60;
                let seconds = time_nanoseconds / 1_000_000_000 % 60;
                let nanos = time_nanoseconds % 1_000_000_000;
                format!(
                    "TimeShift.clock(time={hours:02}:{minutes:02}:{seconds:02}.{nanos:09}, day_offset={day_offset}, duration_ns={duration_nanoseconds})"
                )
            }
            Self::Calendar {
                period,
                duration_nanoseconds,
                dst_policy,
            } => format!(
                "TimeShift.calendar(period={}, duration_ns={}, overlap={}, gap={})",
                period.to_iso_string(),
                duration_nanoseconds,
                overlap_token(dst_policy.overlap),
                gap_token(dst_policy.gap),
            ),
        }
    }

    pub fn from_runtime_token(token: &str) -> Result<Self, TemporalError> {
        let mut fields = token.split('|');
        match fields.next() {
            Some("C") => {
                let time_nanoseconds = parse_token_field::<i64>(&mut fields)?;
                let day_offset = parse_token_field::<i32>(&mut fields)?;
                let duration_nanoseconds = parse_token_field::<i128>(&mut fields)?;
                if fields.next().is_some()
                    || !(0..crate::temporal::NANOS_PER_DAY).contains(&time_nanoseconds)
                {
                    return Err(TemporalError::Parse);
                }
                Ok(Self::Clock {
                    time_nanoseconds,
                    day_offset,
                    duration_nanoseconds,
                })
            }
            Some("K") => {
                let period = Period::parse_iso(fields.next().ok_or(TemporalError::Parse)?)?;
                let duration_nanoseconds = parse_token_field::<i128>(&mut fields)?;
                let overlap = parse_overlap_token(fields.next().ok_or(TemporalError::Parse)?)?;
                let gap = parse_gap_token(fields.next().ok_or(TemporalError::Parse)?)?;
                if fields.next().is_some() {
                    return Err(TemporalError::Parse);
                }
                Ok(Self::new(
                    period,
                    duration_nanoseconds,
                    DstPolicy { overlap, gap },
                ))
            }
            _ => Err(TemporalError::Parse),
        }
    }

    pub fn apply_to(self, value: ZonedDateTime) -> Result<ZonedDateTime, TemporalError> {
        let Self::Calendar {
            period,
            duration_nanoseconds,
            dst_policy,
        } = self
        else {
            return Err(TemporalError::TimeShiftKindMismatch);
        };
        let local = period.add_to(value.local_datetime()?)?;
        let calendar_adjusted = ZonedDateTime::from_local(local, value.zone(), dst_policy)?;
        let instant = calendar_adjusted
            .instant()
            .checked_add_nanoseconds(duration_nanoseconds)?;
        ZonedDateTime::from_instant(instant, value.zone())
    }
}

fn parse_token_field<T: std::str::FromStr>(
    fields: &mut std::str::Split<'_, char>,
) -> Result<T, TemporalError> {
    fields
        .next()
        .ok_or(TemporalError::Parse)?
        .parse()
        .map_err(|_| TemporalError::Parse)
}

fn overlap_token(policy: OverlapPolicy) -> &'static str {
    match policy {
        OverlapPolicy::Reject => "reject",
        OverlapPolicy::EarlierInstant => "earlier",
        OverlapPolicy::LaterInstant => "later",
    }
}

fn gap_token(policy: GapPolicy) -> &'static str {
    match policy {
        GapPolicy::Reject => "reject",
        GapPolicy::ShiftForwardByGap => "forward",
        GapPolicy::ShiftBackwardByGap => "backward",
    }
}

fn parse_overlap_token(token: &str) -> Result<OverlapPolicy, TemporalError> {
    match token {
        "reject" => Ok(OverlapPolicy::Reject),
        "earlier" => Ok(OverlapPolicy::EarlierInstant),
        "later" => Ok(OverlapPolicy::LaterInstant),
        _ => Err(TemporalError::InvalidDstPolicy),
    }
}

fn parse_gap_token(token: &str) -> Result<GapPolicy, TemporalError> {
    match token {
        "reject" => Ok(GapPolicy::Reject),
        "forward" => Ok(GapPolicy::ShiftForwardByGap),
        "backward" => Ok(GapPolicy::ShiftBackwardByGap),
        _ => Err(TemporalError::InvalidDstPolicy),
    }
}

fn days_in_month(year: i32, month: u32) -> Option<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if chrono::NaiveDate::from_ymd_opt(year, 2, 29).is_some() => Some(29),
        2 => Some(28),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporal_error_codes_are_stable_and_distinguish_dst_outcomes() {
        assert_eq!(
            TemporalError::UnknownTimeZone("Mars/Base".into()).stable_code(),
            2
        );
        assert_eq!(TemporalError::InvalidFixedOffset.stable_code(), 3);
        assert_eq!(TemporalError::AmbiguousLocalTime.stable_code(), 4);
        assert_eq!(TemporalError::NonexistentLocalTime.stable_code(), 5);
        assert_eq!(TemporalError::InvalidDstPolicy.stable_code(), 6);
        assert_eq!(TemporalError::OutOfRange.stable_code(), 8);
    }
    use chrono::{NaiveDate, NaiveTime};

    fn local(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_time(NaiveTime::from_hms_opt(hour, minute, 0).unwrap())
    }

    const STRICT: DstPolicy = DstPolicy {
        overlap: OverlapPolicy::Reject,
        gap: GapPolicy::Reject,
    };

    #[test]
    fn iana_database_version_and_zone_identity_are_explicit() {
        assert!(!IANA_TZDB_VERSION.is_empty());
        assert_eq!(TimeZone::database_version(), IANA_TZDB_VERSION);
        let legacy = TimeZone::iana("US/Eastern").unwrap();
        let current = TimeZone::iana("America/New_York").unwrap();
        assert_eq!(legacy.canonical_identifier(), "America/New_York");
        assert_eq!(legacy, current);
        assert_ne!(TimeZone::utc(), TimeZone::iana("Etc/UTC").unwrap());
        assert_eq!(
            TimeZone::fixed_offset_seconds(-18_000)
                .unwrap()
                .canonical_identifier(),
            "-05:00"
        );
        assert_eq!(
            TimeZone::fixed_offset_seconds(0)
                .unwrap()
                .canonical_identifier(),
            "UTC"
        );
        assert_eq!(
            TimeZone::parse_identifier("+05:30").unwrap(),
            TimeZone::fixed_offset_seconds(19_800).unwrap()
        );
        assert_eq!(TimeZone::parse_identifier("Z").unwrap(), TimeZone::utc());
        assert!(TimeZone::parse_identifier("+24:00").is_err());
    }

    #[test]
    fn zone_queries_report_offsets_dst_and_abbreviations_for_each_instant() {
        let new_york = TimeZone::iana("America/New_York").unwrap();
        let winter = Instant::parse_rfc3339("2024-01-15T12:00:00Z").unwrap();
        let summer = Instant::parse_rfc3339("2024-07-15T12:00:00Z").unwrap();

        assert_eq!(new_york.offset_seconds_at(winter).unwrap(), -18_000);
        assert!(!new_york.is_dst_at(winter).unwrap());
        assert_eq!(new_york.abbreviation_at(winter).unwrap(), "EST");
        assert_eq!(new_york.offset_seconds_at(summer).unwrap(), -14_400);
        assert!(new_york.is_dst_at(summer).unwrap());
        assert_eq!(new_york.abbreviation_at(summer).unwrap(), "EDT");

        let fixed = TimeZone::fixed_offset_seconds(19_800).unwrap();
        assert_eq!(fixed.offset_seconds_at(summer).unwrap(), 19_800);
        assert!(!fixed.is_dst_at(summer).unwrap());
        assert_eq!(fixed.abbreviation_at(summer).unwrap(), "UTC+05:30");

        let invalid = Instant(i128::MAX);
        assert_eq!(
            new_york.offset_seconds_at(invalid),
            Err(TemporalError::OutOfRange)
        );
        assert_eq!(new_york.is_dst_at(invalid), Err(TemporalError::OutOfRange));
        assert_eq!(
            new_york.abbreviation_at(invalid),
            Err(TemporalError::OutOfRange)
        );
    }

    #[test]
    fn every_pinned_iana_link_resolves_to_its_canonical_zone() {
        for line in include_str!("tzdb_links_2025b.txt").lines() {
            let Some((alias, target)) = line.split_once('\t') else {
                continue;
            };
            let alias = TimeZone::iana(alias).unwrap();
            let canonical_target = TimeZone::iana(target).unwrap();
            assert_eq!(
                alias,
                canonical_target,
                "alias `{}` -> `{target}`",
                alias.canonical_identifier()
            );
            assert_eq!(alias.canonical_identifier(), canonical_iana_name(target));
        }
    }

    #[test]
    fn instant_parsing_and_zone_projection_preserve_the_timeline_point() {
        let instant = Instant::parse_rfc3339("2024-03-10T07:30:00Z").unwrap();
        let utc = ZonedDateTime::from_instant(instant, TimeZone::utc()).unwrap();
        let new_york = utc
            .with_zone(TimeZone::iana("America/New_York").unwrap())
            .unwrap();
        assert_eq!(new_york.instant(), instant);
        assert_eq!(
            new_york.local_datetime().unwrap(),
            local(2024, 3, 10, 3, 30)
        );
        assert_eq!(
            instant
                .checked_sub(Instant::from_unix_seconds(0).unwrap())
                .unwrap(),
            1_710_055_800_000_000_000
        );
    }

    #[test]
    fn zoned_rfc3339_parser_checks_the_offset_against_pinned_zone_rules() {
        let value =
            ZonedDateTime::parse_rfc3339("2024-03-10T03:30:00-04:00[America/New_York]").unwrap();
        assert_eq!(
            value.instant().to_rfc3339().unwrap(),
            "2024-03-10T07:30:00+00:00"
        );
        assert_eq!(value.local_datetime().unwrap(), local(2024, 3, 10, 3, 30));
        assert!(
            ZonedDateTime::parse_rfc3339("2024-03-10T03:30:00-05:00[America/New_York]").is_err()
        );
        assert!(ZonedDateTime::parse_rfc3339("not a zoned value").is_err());
    }

    #[test]
    fn daylight_saving_overlap_requires_and_obeys_policy() {
        let zone = TimeZone::iana("America/New_York").unwrap();
        let wall = local(2024, 11, 3, 1, 30);
        assert_eq!(
            ZonedDateTime::from_local(wall, zone, STRICT),
            Err(TemporalError::AmbiguousLocalTime)
        );
        let earlier = ZonedDateTime::from_local(
            wall,
            zone,
            DstPolicy {
                overlap: OverlapPolicy::EarlierInstant,
                gap: GapPolicy::Reject,
            },
        )
        .unwrap();
        let later = ZonedDateTime::from_local(
            wall,
            zone,
            DstPolicy {
                overlap: OverlapPolicy::LaterInstant,
                gap: GapPolicy::Reject,
            },
        )
        .unwrap();
        assert_eq!(
            later.instant().unix_nanoseconds() - earlier.instant().unix_nanoseconds(),
            3_600_000_000_000
        );
        assert_eq!(earlier.local_datetime().unwrap(), wall);
        assert_eq!(later.local_datetime().unwrap(), wall);
    }

    #[test]
    fn packed_local_datetime_resolves_dst_with_explicit_policies() {
        let zone = TimeZone::iana("America/New_York").unwrap();
        let packed = crate::temporal::datetime_pack(
            crate::temporal::days_from_civil(2024, 11, 3),
            90 * 60 * 1_000_000_000,
        );
        assert_eq!(
            ZonedDateTime::from_packed_local_datetime(packed, zone, STRICT),
            Err(TemporalError::AmbiguousLocalTime)
        );
        let earlier = ZonedDateTime::from_packed_local_datetime(
            packed,
            zone,
            DstPolicy {
                overlap: OverlapPolicy::EarlierInstant,
                gap: GapPolicy::Reject,
            },
        )
        .unwrap();
        let later = ZonedDateTime::from_packed_local_datetime(
            packed,
            zone,
            DstPolicy {
                overlap: OverlapPolicy::LaterInstant,
                gap: GapPolicy::Reject,
            },
        )
        .unwrap();
        assert_eq!(
            earlier.zone().offset_seconds_at(earlier.instant()),
            Ok(-14_400)
        );
        assert_eq!(later.zone().offset_seconds_at(later.instant()), Ok(-18_000));
    }

    #[test]
    fn daylight_saving_gap_rejects_or_shifts_by_the_transition_size() {
        let zone = TimeZone::iana("America/New_York").unwrap();
        let wall = local(2024, 3, 10, 2, 30);
        assert_eq!(
            ZonedDateTime::from_local(wall, zone, STRICT),
            Err(TemporalError::NonexistentLocalTime)
        );
        let forward = ZonedDateTime::from_local(
            wall,
            zone,
            DstPolicy {
                overlap: OverlapPolicy::Reject,
                gap: GapPolicy::ShiftForwardByGap,
            },
        )
        .unwrap();
        let backward = ZonedDateTime::from_local(
            wall,
            zone,
            DstPolicy {
                overlap: OverlapPolicy::Reject,
                gap: GapPolicy::ShiftBackwardByGap,
            },
        )
        .unwrap();
        assert_eq!(forward.local_datetime().unwrap(), local(2024, 3, 10, 3, 30));
        assert_eq!(
            backward.local_datetime().unwrap(),
            local(2024, 3, 10, 1, 30)
        );
    }

    #[test]
    fn period_month_arithmetic_clamps_month_ends_and_handles_negative_shifts() {
        let january_end = local(2024, 1, 31, 12, 15);
        assert_eq!(
            Period::new(0, 1, 0, 0).add_to(january_end).unwrap(),
            local(2024, 2, 29, 12, 15)
        );
        assert_eq!(
            Period::new(0, -1, 0, 0)
                .add_to(local(2024, 3, 31, 12, 15))
                .unwrap(),
            local(2024, 2, 29, 12, 15)
        );
        assert_eq!(
            Period::new(0, 0, 0, -1).add_to(january_end).unwrap(),
            local(2024, 1, 30, 12, 15)
        );
        assert_eq!(Period::default().checked_neg().unwrap(), Period::default());
    }

    #[test]
    fn period_scalar_encoding_round_trips_every_signed_component() {
        for period in [
            Period::new(0, 0, 0, 0),
            Period::new(1, -2, 3, -4),
            Period::new(i32::MIN, i32::MAX, -1, 1),
        ] {
            assert_eq!(Period::decode(period.encode()), period);
        }
    }

    #[test]
    fn time_shift_applies_calendar_dst_resolution_before_exact_duration() {
        let zone = TimeZone::iana("America/New_York").unwrap();
        let start = ZonedDateTime::from_local(local(2024, 3, 9, 2, 30), zone, STRICT).unwrap();
        let shift = TimeShift::new(
            Period::new(0, 0, 0, 1),
            3_600_000_000_000,
            DstPolicy {
                overlap: OverlapPolicy::Reject,
                gap: GapPolicy::ShiftForwardByGap,
            },
        );
        let shifted = shift.apply_to(start).unwrap();
        // One calendar day lands at 03:30 after the spring gap. The exact hour
        // is then applied on the timeline, producing 04:30 local.
        assert_eq!(shifted.local_datetime().unwrap(), local(2024, 3, 10, 4, 30));
        assert_eq!(shift.period(), Some(Period::new(0, 0, 0, 1)));
        assert_eq!(shift.duration_nanoseconds(), 3_600_000_000_000);
        assert_eq!(shift.kind(), "calendar");
        assert_eq!(shift.time_nanoseconds(), None);
        assert_eq!(shift.day_offset(), None);
    }

    #[test]
    fn calendar_time_shift_resolves_dst_overlap_with_its_selected_policy() {
        let zone = TimeZone::iana("America/New_York").unwrap();
        let start = ZonedDateTime::from_local(local(2024, 11, 2, 1, 30), zone, STRICT).unwrap();
        let rejected = TimeShift::new(Period::new(0, 0, 0, 1), 0, STRICT);
        assert_eq!(
            rejected.apply_to(start),
            Err(TemporalError::AmbiguousLocalTime)
        );

        let earlier = TimeShift::new(
            Period::new(0, 0, 0, 1),
            0,
            DstPolicy {
                overlap: OverlapPolicy::EarlierInstant,
                gap: GapPolicy::Reject,
            },
        )
        .apply_to(start)
        .unwrap();
        let later = TimeShift::new(
            Period::new(0, 0, 0, 1),
            0,
            DstPolicy {
                overlap: OverlapPolicy::LaterInstant,
                gap: GapPolicy::Reject,
            },
        )
        .apply_to(start)
        .unwrap();
        assert_eq!(
            later.instant().unix_nanoseconds() - earlier.instant().unix_nanoseconds(),
            3_600_000_000_000
        );
    }

    #[test]
    fn clock_time_shift_preserves_wrapped_time_duration_and_signed_day_carry() {
        let shift = TimeShift::clock(23 * 3_600_000_000_000, 2 * 3_600_000_000_000).unwrap();
        assert_eq!(shift.kind(), "clock");
        assert_eq!(shift.time_nanoseconds(), Some(3_600_000_000_000));
        assert_eq!(shift.day_offset(), Some(1));
        assert_eq!(shift.duration_nanoseconds(), 7_200_000_000_000);
        assert_eq!(shift.period(), None);

        let negative = TimeShift::clock(0, -1).unwrap();
        assert_eq!(negative.time_nanoseconds(), Some(86_399_999_999_999));
        assert_eq!(negative.day_offset(), Some(-1));
        let zone = TimeZone::utc();
        let start =
            ZonedDateTime::from_instant(Instant::from_unix_seconds(0).unwrap(), zone).unwrap();
        assert_eq!(
            negative.apply_to(start),
            Err(TemporalError::TimeShiftKindMismatch)
        );
    }

    #[test]
    fn time_shift_private_wire_form_round_trips_both_variants() {
        let shifts = [
            TimeShift::clock(0, -1).unwrap(),
            TimeShift::new(
                Period::new(1, -2, 0, 3),
                17,
                DstPolicy {
                    overlap: OverlapPolicy::LaterInstant,
                    gap: GapPolicy::ShiftBackwardByGap,
                },
            ),
        ];
        for shift in shifts {
            assert_eq!(
                TimeShift::from_runtime_token(&shift.to_runtime_token()),
                Ok(shift)
            );
        }
        assert_eq!(
            TimeShift::from_runtime_token("C|86400000000000|0|0"),
            Err(TemporalError::Parse)
        );
        assert_eq!(
            TimeShift::clock(23 * 3_600_000_000_000, 2 * 3_600_000_000_000)
                .unwrap()
                .to_display_string(),
            "TimeShift.clock(time=01:00:00.000000000, day_offset=1, duration_ns=7200000000000)"
        );
    }

    #[test]
    fn period_iso_zero_and_repeated_zero_fields_are_unambiguous() {
        assert_eq!(Period::parse_iso("P0D"), Ok(Period::default()));
        assert_eq!(Period::parse_iso("P0Y0D"), Ok(Period::new(0, 0, 0, 0)));
        assert_eq!(Period::parse_iso("P0D0D"), Err(TemporalError::Parse));
        assert_eq!(Period::parse_iso("P1Y1Y"), Err(TemporalError::Parse));
    }

    #[test]
    fn period_and_time_shift_report_calendar_and_instant_overflow() {
        assert_eq!(
            Period::new(10_000, 0, 0, 0).add_to(local(262_000, 12, 31, 23, 59)),
            Err(TemporalError::OutOfRange)
        );
        let start =
            ZonedDateTime::from_instant(Instant::from_unix_seconds(0).unwrap(), TimeZone::utc())
                .unwrap();
        let shift = TimeShift::new(Period::default(), i128::MAX, STRICT);
        assert_eq!(shift.apply_to(start), Err(TemporalError::OutOfRange));
    }

    #[test]
    fn unknown_zone_and_out_of_range_instants_are_typed_errors() {
        assert_eq!(
            TimeZone::iana("Mars/Olympus_Mons"),
            Err(TemporalError::UnknownTimeZone("Mars/Olympus_Mons".into()))
        );
        assert_eq!(
            TimeZone::fixed_offset_seconds(86_400),
            Err(TemporalError::InvalidFixedOffset)
        );
        assert_eq!(
            Instant::from_unix_nanoseconds(i128::MAX),
            Err(TemporalError::OutOfRange)
        );
        assert_eq!(
            Instant::parse_rfc3339("2024-02-30T07:30:00Z"),
            Err(TemporalError::Parse)
        );
    }
}
