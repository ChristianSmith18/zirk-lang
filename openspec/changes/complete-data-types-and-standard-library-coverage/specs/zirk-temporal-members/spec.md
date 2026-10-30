## ADDED Requirements

### Requirement: Temporal formatting and parsing accept explicit locales

Temporal formatting and locale-aware parsing SHALL accept an explicit validated BCP 47 locale and SHALL use a versioned CLDR data set. Locale-sensitive operations SHALL NOT read or mutate an implicit process-global locale. The public temporal locale surface SHALL cover localized date, time, and date-time styles, month/day names, locale numbering preferences, and time-zone display names where applicable; ISO serialization and the existing locale-neutral parse/format APIs SHALL remain stable. Localized parsing SHALL report ambiguous, invalid, or unsupported input with typed errors.

Malformed tags SHALL return `LocaleError.InvalidTag`. A valid tag absent from the pinned CLDR data SHALL return `LocaleError.UnsupportedLocale`; unavailable versioned locale data SHALL return `LocaleError.DataUnavailable`. The API SHALL NOT silently fall back to a different locale or process-global settings.

#### Scenario: Format with an explicit locale

- **WHEN** the same temporal value is formatted with two supported locales
- **THEN** output follows each locale's documented date/time conventions without changing the underlying temporal value

#### Scenario: Reject an invalid or unsupported locale tag

- **WHEN** a locale-sensitive temporal API receives a malformed or unsupported BCP 47 tag
- **THEN** it returns `LocaleError.InvalidTag` for malformed syntax or `LocaleError.UnsupportedLocale` when valid data is absent

#### Scenario: Parse ambiguous localized input

- **WHEN** localized text can represent more than one valid date or time under the selected locale and parse options
- **THEN** parsing reports ambiguity rather than selecting a value silently

#### Scenario: Locale data version is observable

- **WHEN** application code queries the locale data version used for formatting
- **THEN** the bundled CLDR version is available so output changes can be audited across toolchain upgrades

#### Scenario: Report unavailable pinned locale data

- **WHEN** the requested locale-data version is unavailable
- **THEN** the operation returns `LocaleError.DataUnavailable` without consulting a host locale or network source
