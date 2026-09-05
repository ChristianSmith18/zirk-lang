# DateTime

`DateTime` combines a Date and Time but deliberately has no zone. It represents
“2026-08-15 at 14:30” as entered locally, not one universal instant.

```zirk
inmut local = DateTime(2026, 8, 15, 14, 30);
inmut composed = Date(2026, 8, 15) + Time(14, 30);
```

It exposes `date`, `time` and their component properties through nanoseconds.
`with_date()`, `with_time()`, component replacement, parsing, formatting,
`start_of()` and `end_of()` return new DateTime values.

## Arithmetic

DateTime accepts exact Duration and calendar Period arithmetic. The former
moves local components by a fixed quantity; the latter applies calendar rules.
Subtracting two DateTime values yields a local component Duration, not a
globally meaningful elapsed time across zones.

## Acquiring a zone

```zirk
inmut zoned = local.in_zone(TimeZone("America/Santiago"));
```

Zone assignment can fail if the local time is skipped or repeated by DST. Zirk
rejects without an explicit policy rather than guessing. Once resolved, the
result is ZonedDateTime and has an absolute `instant`.

Do not record globally ordered events as bare DateTime. Use Instant or
ZonedDateTime once the zone is known.

## API

A `Date` + `Time` pair, deliberately without a zone.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `date` | `Date` | Date part | specified |
| `time` | `Time` | Time part | specified |
| `year` … `nanosecond` | `Int32` | All `Date`/`Time` component properties | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `DateTime(y, m, d, h, min, s?, ns?)` | `DateTime` | Component construction | specified |
| `DateTime(date, time)` / `date + time` | `DateTime` | Composition | specified |
| `DateTime.parse(text, format?)` | `Result<DateTime, ParseError>` | Explicit parsing | specified |
| `dt.with_date(date)` / `dt.with_time(time)` | `DateTime` | Part replacement | specified |
| `dt.with_*(component)` | `DateTime` | Component replacement | specified |
| `dt.start_of(unit)` / `dt.end_of(unit)` | `DateTime` | Boundary of the unit | specified |
| `dt ± Duration` | `DateTime` | Exact local-component shift | specified |
| `dt ± Period` | `DateTime` | Calendar arithmetic preserving local clock | specified |
| `dt - other` | `Duration` | Local component difference — not a global elapsed time | specified |
| `dt.in_zone(zone, policy?)` | `Result<ZonedDateTime, TemporalError>` | Localization; fails without an explicit DST policy on gap/overlap | specified |
| `dt.format(pattern, locale?)` | `String` | Presentation formatting | specified |
| `dt.to_iso_string()` / `dt.to_string()` | `String` | Machine/default rendering | specified |

### Examples

```zirk
inmut local = DateTime(2026, 8, 15, 14, 30);
inmut composed = Date(2026, 8, 15) + Time(14, 30);
inmut zoned = local.in_zone(TimeZone("America/Santiago"));
```

---

**Previous:** [← Time](03-time.md) · **Next:** [ Instant](05-instant.md)
