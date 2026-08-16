# Temporal Parsing and Formatting

Constructors accept canonical ISO forms where the type's meaning is
unambiguous; custom input requires an explicit pattern.

```zirk
Date("2026-08-15")
Date.parse("15/08/2026", "DD/MM/YYYY")
Instant.parse("2026-08-15T18:30:00Z")
Duration("PT2H30M")
Period("P1Y2M3D")
```

Parsing validates complete input, ranges, leap days, offsets and zone/offset
consistency. It never repairs invalid values or uses locale heuristics without
an explicit locale.

`to_iso_string()` emits the canonical machine form. `format(pattern, locale?)`
is for presentation; its pattern symbols are shared consistently across types
but only meaningful components are accepted. Formatting a Date with an hour
token is an error rather than an empty substitution.

```zirk
date.format("DD/MM/YYYY")
zoned.format("YYYY-MM-DD HH:mm z", locale: "es-CL")
duration.humanize(locale: "es", max_units: 2)
```

Serialization should prefer ISO forms plus explicit zone IDs/version context
where replay depends on zone rules. Humanized strings are not stable machine
formats.

---

**Previous:** [← Temporal Arithmetic](11-temporal-arithmetic.md) · **Next:** [ Zones and DST](13-zones-and-dst.md)
