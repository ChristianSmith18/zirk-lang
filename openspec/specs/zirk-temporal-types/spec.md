# zirk-temporal-types Specification

## Purpose
Defines the sealed temporal family, exact and calendar arithmetic, time zones,
comparison domains, parsing, formatting, and temporal errors.

## Requirements

### Requirement: Current-time APIs separate values from clock sources
`Instant`, `Date`, `Time`, `DateTime`, and `ZonedDateTime` SHALL expose friendly
current-value construction with an optional injectable clock; civil values
SHALL require an explicit zone or a fallible explicitly local helper.
`Clock.system`, `Clock.monotonic`, and virtual test clocks SHALL remain distinct,
and virtual clocks SHALL NOT mutate host-global time.

#### Scenario: Current local time is unavailable
- **WHEN** `Time.now_local()` cannot discover a valid host time zone
- **THEN** it returns a typed timezone error rather than silently using UTC

### Requirement: Timer resources report scheduling behavior
One-shot timers and repeating tickers SHALL suspend tasks, observe structured
cancellation, require non-negative durations, and support explicit fixed-rate
and fixed-delay policies. Fixed-rate ticks SHALL report missed intervals rather
than enqueueing an unbounded backlog.

#### Scenario: Fixed-rate consumer misses ticks
- **WHEN** a ticker consumer resumes after multiple scheduled intervals
- **THEN** the next tick reports its scheduled and observed instants plus the
  missed count

### Requirement: Distinct temporal value types
Zirk SHALL define `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`, and `Period` as distinct immutable value types in a sealed `Temporal` capability family. Membership in that family SHALL NOT make unsupported cross-type operations valid.

#### Scenario: Compose a date and time
- **WHEN** a `Date` is combined with a `Time`
- **THEN** the result is a `DateTime` with those calendar and clock components

#### Scenario: Reject meaningless composition
- **WHEN** application code attempts to add a `TimeZone` and a `Duration`
- **THEN** type checking rejects the operation

### Requirement: Calendar, local, instant, and zoned distinctions
`Date` SHALL contain only a calendar date, `Time` only a clock time, `DateTime` a date and time without a zone, `Instant` an absolute timeline point, and `ZonedDateTime` an instant plus an IANA time zone and its local representation.

#### Scenario: Local date-time needs a zone
- **WHEN** a `DateTime` is converted with `in_zone(TimeZone("America/Santiago"))`
- **THEN** the result is a `ZonedDateTime` or a controlled ambiguity/nonexistence error

#### Scenario: Same instant in two zones
- **WHEN** one `Instant` is represented in Santiago and Tokyo
- **THEN** both zoned values compare equal by instant while exposing different local components and zones

### Requirement: Exact Duration and calendar Period
`Duration` SHALL be a signed exact nanosecond-precision timeline quantity with conceptual `Int128` range. `Period` SHALL represent calendar years, months, weeks, and days and SHALL require calendar context before conversion to exact elapsed time.

#### Scenario: Negative elapsed difference
- **WHEN** an earlier instant is subtracted from a later operand in reverse order
- **THEN** the resulting `Duration` is negative and preserves `a - b == -(b - a)`

#### Scenario: Month is not an exact duration
- **WHEN** code requests total seconds from `Period.months(1)` without a starting date and zone
- **THEN** type checking rejects the operation

### Requirement: Temporal arithmetic and month adjustment
Temporal arithmetic SHALL accept only the documented composition matrix. Adding a month to a date SHALL clamp to the last valid day by default, while `add_strict` SHALL diagnose a missing target day.

#### Scenario: Comfortable month addition
- **WHEN** one month is added to `Date(2026, 1, 31)`
- **THEN** the result is `Date(2026, 2, 28)`

#### Scenario: Strict month addition
- **WHEN** `add_strict(Period.months(1))` is applied to `Date(2026, 1, 31)`
- **THEN** a controlled invalid-calendar-operation error is produced

### Requirement: Time zones and daylight-saving ambiguity
`TimeZone` SHALL use canonical IANA zone identities and rules rather than treating an offset as a zone. Converting a nonexistent or ambiguous local time SHALL reject by default and SHALL accept explicit earlier/later or previous/next-valid policies where applicable.

#### Scenario: Unknown zone
- **WHEN** `TimeZone("Mars/Olympus")` is constructed
- **THEN** a controlled invalid-time-zone error is produced

#### Scenario: Ambiguous local time
- **WHEN** a repeated local time is assigned to a zone without an ambiguity policy
- **THEN** the operation reports `AmbiguousLocalTime` rather than selecting silently

### Requirement: Temporal parsing, formatting, and errors
Every temporal type SHALL provide appropriate ISO serialization, explicit-pattern parsing/formatting, typed properties, and controlled errors for invalid values, parsing, formatting, overflow, zones, and DST resolution.

#### Scenario: Invalid leap day
- **WHEN** `Date(2026, 2, 29)` is constructed
- **THEN** an `InvalidDate` error is produced instead of normalizing the date

#### Scenario: ISO duration
- **WHEN** `Duration("PT2H30M")` is parsed
- **THEN** the result equals two hours and thirty minutes

### Requirement: Duration sign and magnitude members

A `Duration` SHALL expose `abs()`, `sign()`, `is_zero()`, `is_positive()`,
and `is_negative()` in addition to its existing literal/arithmetic surface.

#### Scenario: Magnitude

- **WHEN** `(-5s).abs()` is evaluated
- **THEN** the result is `5s`

#### Scenario: Sign tests

- **WHEN** `(-1s).is_negative()`, `(0s).is_zero()`, and
  `(2s).is_positive()` run
- **THEN** all produce `true`

#### Scenario: Sign value

- **WHEN** `(-3s).sign()` is evaluated
- **THEN** the result is `-1` as `Int32`

### Requirement: Duration primitive

`Duration` SHALL be a compiler primitive representing an exact span of time with nanosecond precision. It SHALL support literal suffixes, arithmetic, comparison, and `to_string()`. `Duration` SHALL be distinct from integer counts of time and SHALL avoid calendar ambiguity.

#### Scenario: Duration literals
- **WHEN** `inmut d: Duration = 250ms;` is declared
- **THEN** `d` represents 250 milliseconds

- **WHEN** `inmut d: Duration = 1.5s;` is declared
- **THEN** `d` represents 1500 milliseconds

- **WHEN** `inmut d: Duration = 2h;` is declared
- **THEN** `d` represents two hours in nanoseconds

### Requirement: Duration arithmetic and comparison

`Duration` SHALL support addition, subtraction, negation, multiplication by a scalar, and division by a scalar and by another `Duration`. Division of two `Duration` values SHALL yield a `Float64`. Comparisons SHALL be total.

#### Scenario: Duration arithmetic
- **WHEN** `1s + 500ms` is evaluated
- **THEN** the result is `1500ms`

#### Scenario: Duration ratio
- **WHEN** `1s / 250ms` is evaluated
- **THEN** the result is `4.0` as `Float64`

### Requirement: Duration formatting

`Duration.to_string()` SHALL produce a human-readable representation. It SHALL be lossless for exact nanosecond values and SHALL normalize larger units when possible.

#### Scenario: Duration to_string
- **WHEN** `(1500ms).to_string()` is evaluated
- **THEN** the result is `"1.5s"` or an equivalent normalized form


### Requirement: Civil temporal types are implemented

`Date`, `Time`, and `DateTime` SHALL be available as immutable value types: construction validates calendar and clock fields, components are exposed as read-only properties, values compare totally, and `to_string()` produces ISO 8601 text. `Date(2026, 2, 29)` SHALL be a controlled `InvalidDate` error, and `Time(25, 0)` a controlled `InvalidTime` error — neither may silently normalize.

#### Scenario: Construct and read a date
- **WHEN** `Date(2026, 9, 6)` is evaluated
- **THEN** `.year` is `2026`, `.month` is `9`, `.day` is `6`, and `to_string()` is `"2026-09-06"`

#### Scenario: Reject an invalid leap day
- **WHEN** `Date(2026, 2, 29)` is constructed
- **THEN** a controlled `InvalidDate` error is produced

#### Scenario: Construct and read a time
- **WHEN** `Time(14, 30, 45)` is evaluated
- **THEN** `.hour` is `14`, `.minute` is `30`, `.second` is `45`, and `to_string()` is `"14:30:45"`

#### Scenario: Reject an out-of-range hour
- **WHEN** `Time(25, 0)` is constructed
- **THEN** a controlled `InvalidTime` error is produced

#### Scenario: Compose a date and time
- **WHEN** `Date(2026, 9, 6)` is combined with `Time(14, 30)`
- **THEN** the result is a `DateTime` whose `to_string()` is `"2026-09-06T14:30:00"`

### Requirement: Civil temporal arithmetic

`Date` SHALL support `+`/`-` by a whole-day count and `Date - Date` SHALL yield the day difference as `Int64`. `Time` SHALL support `+`/`-` by `Duration`, wrapping modulo 24 hours. `DateTime` SHALL support `+`/`-` by `Duration` as an exact nanosecond shift of its local components, and `DateTime - DateTime` SHALL yield the exact difference as `Duration`. `Date ± Duration` SHALL promote to `DateTime`: the date is read at local midnight and the duration shifts it — `Date + 5h` is `DateTime` at 05:00 the same day, never a silently truncated `Date`. Comparisons between same-type civil values SHALL be total and calendar-correct.

#### Scenario: Add days across a month boundary
- **WHEN** `Date(2026, 1, 31) + 2` is evaluated
- **THEN** the result is `Date(2026, 2, 2)`

#### Scenario: Difference between dates
- **WHEN** `Date(2026, 9, 6) - Date(2026, 9, 1)` is evaluated
- **THEN** the result is `5` as `Int64`

#### Scenario: Time addition wraps the day
- **WHEN** `Time(23, 30) + 2h` is evaluated
- **THEN** the result is `Time(1, 30)`

#### Scenario: Date plus duration promotes to DateTime
- **WHEN** `Date(2026, 9, 6) + 5h` is evaluated
- **THEN** the result is `DateTime` equal to `DateTime(Date(2026, 9, 6), Time(5, 0))`

#### Scenario: DateTime shifts across midnight
- **WHEN** `DateTime(Date(2026, 9, 6), Time(23, 0)) + 2h` is evaluated
- **THEN** the result is `DateTime` equal to `DateTime(Date(2026, 9, 7), Time(1, 0))`

#### Scenario: DateTime difference is a Duration
- **WHEN** `DateTime(Date(2026, 9, 7), Time(0, 0)) - DateTime(Date(2026, 9, 6), Time(0, 0))` is evaluated
- **THEN** the result is a `Duration` of exactly 24 hours

### Requirement: Host-clock civil constructors

`Date.today()` and `Time.now_local()` SHALL return the host's current civil date and time through the runtime, without interpreting or validating a named zone.

#### Scenario: Today is a valid date
- **WHEN** `Date.today()` is evaluated
- **THEN** the result is a `Date` whose fields form a valid calendar day

### Requirement: Civil temporal arithmetic uses checked wide intermediates

Civil date and time operations SHALL detect unrepresentable intermediate or result values before narrowing. `Time ± Duration` SHALL apply modulo-day semantics using an intermediate wide enough to avoid overflow. Date projections SHALL reject years outside the public representation instead of truncating them.

#### Scenario: Time wraps with an extreme duration
- **WHEN** a representable time is combined with an extreme duration
- **THEN** the documented modulo-day result is computed without integer overflow

#### Scenario: Date arithmetic exceeds the public year range
- **WHEN** date arithmetic or projection produces a year outside the supported year representation
- **THEN** the operation returns a controlled range failure and does not wrap or truncate

#### Scenario: Ordinary temporal operation is representable
- **WHEN** a date/time operation remains within its supported range
- **THEN** native and interpreted execution return the same result

### Requirement: Zone-aware timeline values are delivered

`Instant` SHALL represent an absolute timeline point independently of a zone. `TimeZone` SHALL represent a canonical IANA zone identity backed by a versioned rule database; a fixed offset SHALL remain distinguishable from a named zone. `ZonedDateTime` SHALL combine an `Instant`, its named zone, the rule-data version used for conversion, and the corresponding local components. Conversion SHALL preserve the instant and report ambiguous or nonexistent local times explicitly. Absolute-time comparison SHALL compare instants, while zone identity and local fields remain available for inspection.

`Instant` SHALL expose `from_unix_seconds(Int64)`, `from_unix_milliseconds(Int64)`, and `from_unix_nanoseconds(Int128)` factories. Each factory SHALL return `Result<Instant, ArithmeticOverflowError>` and SHALL reject a value outside the timeline range supported by the pinned temporal implementation. The nanosecond factory SHALL preserve the supplied integer exactly; second and millisecond factories SHALL scale using checked arithmetic.

The constructor SHALL resolve IANA links to a canonical identifier and expose that identifier. An unknown identifier SHALL return `TimeZoneError.UnknownIdentifier`; unavailable pinned rule data SHALL return `TimeZoneError.DataUnavailable`. Neither condition SHALL consult the host database or silently become a fixed offset. Constructing a zoned value from a local wall time SHALL require explicit overlap and gap policies; the default policy for each is `Reject`. Overlaps support `EarlierInstant` and `LaterInstant`; gaps support shifting forward or backward by the transition gap. Invalid civil values and arithmetic outside the supported instant range SHALL return typed errors.

#### Scenario: Convert one instant without changing its identity

- **WHEN** an `Instant` is represented in two different IANA zones
- **THEN** both `ZonedDateTime` values retain the same instant while exposing the appropriate local components and zone identities

#### Scenario: Construct an instant from Unix units

- **WHEN** a caller constructs an `Instant` from seconds, milliseconds, or nanoseconds since the Unix epoch
- **THEN** the checked factory returns the same absolute timeline point for equivalent inputs and preserves nanosecond precision

#### Scenario: Reject a Unix timestamp outside the supported timeline

- **WHEN** a Unix-unit factory receives a value outside the pinned implementation's supported instant range
- **THEN** it returns `ArithmeticOverflowError` in the `Result` without wrapping or truncating the value

#### Scenario: Resolve a daylight-saving gap explicitly

- **WHEN** a local `DateTime` falls in a nonexistent or repeated wall-clock interval
- **THEN** conversion returns a typed ambiguity/nonexistence outcome unless the caller supplies a documented resolution policy

#### Scenario: Reproducible zone rules

- **WHEN** a serialized or compared zoned value depends on time-zone rules
- **THEN** the applicable zone database version is available to the caller and the result does not silently use a fixed offset as an IANA zone

#### Scenario: Reject unknown or unavailable zone data

- **WHEN** a caller requests an unknown IANA identifier or the pinned zone data is unavailable
- **THEN** the operation returns the corresponding typed `TimeZoneError` without consulting host zone data or substituting a fixed offset

#### Scenario: Resolve a repeated local time with an explicit policy

- **WHEN** a local time occurs twice during an overlap
- **THEN** the default rejects it and an explicit `EarlierInstant` or `LaterInstant` policy selects the corresponding instant

### Requirement: Period and TimeShift distinguish calendar changes from elapsed time

`Period` SHALL represent calendar-relative years, months, weeks, and days and SHALL require a calendar and zone context before conversion to elapsed time. `TimeShift` SHALL be one immutable tagged type with two forms: a clock shift produced by `Time ± Duration`, which retains the wrapped `Time` and signed day offset; and a calendar shift constructed by `TimeShift.calendar(period, duration, overlap, gap)` from a `Period`, exact elapsed `Duration`, and explicit overlap/gap policy strings. It SHALL expose `kind`, `duration`, nullable clock fields (`time`, `day_offset`), and nullable calendar fields (`period`, `overlap_policy`, `gap_policy`); fields from the inactive form SHALL be null. Applying a calendar shift to a `ZonedDateTime` SHALL NOT silently equate a calendar period with an exact duration, and applying a clock shift to a `ZonedDateTime` SHALL return a typed form-mismatch error.

Month arithmetic SHALL clamp to the final valid day of the target month. Applying a calendar `TimeShift` SHALL resolve the calendar-adjusted local time using its overlap/gap policies before adding the exact `Duration` to the resolved instant. A clock `TimeShift` SHALL expose its wrapped `time` and `day_offset`; a calendar `TimeShift` SHALL expose its `period`, `duration`, and policies; inactive-form fields SHALL be null. Overflow SHALL return a typed temporal arithmetic error.

#### Scenario: Apply a calendar month in a zone

- **WHEN** a `Period` containing a month is applied to a `ZonedDateTime`
- **THEN** documented month-end and daylight-saving policies determine the local result and any unresolved ambiguity is reported

#### Scenario: TimeShift applies its components in order

- **WHEN** a `TimeShift` containing both a `Period` and `Duration` is applied across a daylight-saving transition
- **THEN** it applies the declared calendar/exact order and resolution policy and produces a reproducible result

#### Scenario: Reject context-free elapsed conversion

- **WHEN** code requests an exact elapsed duration from a calendar `Period` without a starting date and zone
- **THEN** the operation is rejected or returns its typed missing-context error

#### Scenario: Clamp a month-end period

- **WHEN** a period advances a date whose day does not exist in the target month
- **THEN** the result uses the final valid day of that month, and an out-of-range result returns a typed overflow error
