# TimeZone

`TimeZone` names versioned IANA rules such as `America/Santiago`; it is not just
an offset like `-04:00`.

```zirk
inmut chile = TimeZone("America/Santiago");
inmut utc = TimeZone.UTC;
inmut system = TimeZone.system();
```

An IANA zone can change offset across seasons and history. A fixed offset has no
DST transition rules and is represented explicitly as an offset, not silently
promoted to TimeZone.

## API

A versioned IANA rule set (`"America/Santiago"`), not a mere offset.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `TimeZone.UTC` | `TimeZone` | The UTC zone | specified |
| `id` | `String` | Canonical IANA identifier | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `TimeZone("Area/Location")` | `TimeZone` | Validated construction; unknown IDs raise `InvalidTimeZone` | specified |
| `TimeZone.system()` | `TimeZone` | Host system zone | specified |
| `TimeZone.database_version()` | `String` | IANA database version — required where reproducibility matters | specified |
| `z.offset_at(instant)` | `Duration` | Offset from UTC at that instant | specified |
| `z.is_dst_at(instant)` | `Boolean` | Whether DST applies at that instant | specified |
| `z.abbreviation_at(instant)` | `String` | Abbreviation (e.g. `"CLT"`, `"CLST"`) | specified |
| `z.to_string()` | `String` | The canonical `id` | specified |

Equality compares canonical identity. Zones have no arithmetic or natural
order. A fixed offset is a distinct explicit concept, never silently promoted
to `TimeZone`.

### Examples

```zirk
inmut chile = TimeZone("America/Santiago");
inmut utc = TimeZone.UTC;
inmut system = TimeZone.system();
TimeZone("Mars/Olympus");   // InvalidTimeZone
```

---

**Previous:** [← Instant](05-instant.md) · **Next:** [ ZonedDateTime](07-zoned-date-time.md)
