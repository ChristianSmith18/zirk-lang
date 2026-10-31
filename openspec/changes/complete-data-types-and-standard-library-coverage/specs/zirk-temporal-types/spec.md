## REMOVED Requirements

### Requirement: Out-of-scope temporal types

**Reason**: This change brings the zone-aware temporal family into the delivered contract; keeping these types declared out of scope directly contradicts the normative family and current feature roadmap.

**Migration**: Use the temporal requirements added by this change. Until implementation completes, status documentation SHALL continue to mark the types as pending and SHALL not imply that the specification itself proves delivery.

## ADDED Requirements

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
