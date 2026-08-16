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

`id` is the canonical identity. `offset_at(instant)`, `is_dst_at(instant)` and
`abbreviation_at(instant)` query rules at a real point. Equality compares the
canonical identity; zones have no arithmetic or natural order.

```zirk
TimeZone("Mars/Olympus"); // InvalidTimeZone
```

Database packaging and update delivery are runtime/stdlib concerns. Public
behavior must expose the database version when reproducibility matters and
must never reinterpret a stored zone ID as a fixed offset.

---

**Previous:** [← Instant](05-instant.md) · **Next:** [ ZonedDateTime](07-zoned-date-time.md)
