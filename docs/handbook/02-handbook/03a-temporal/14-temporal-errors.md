# Temporal Errors

Temporal failures are controlled and typed. They never normalize invalid input,
wrap an Instant, or guess a DST policy silently.

| Error | Meaning |
| --- | --- |
| `InvalidDate` | impossible/out-of-range calendar components |
| `InvalidTime` | impossible/out-of-range clock components |
| `InvalidDateTime` | invalid combined local components |
| `InvalidTimeZone` | unknown/malformed IANA identity |
| `AmbiguousLocalTime` | local value maps to two instants |
| `NonexistentLocalTime` | local value maps to no instant |
| `DurationOverflow` | exact quantity exceeds supported range |
| `TemporalParseError` | input does not match required form |
| `TemporalFormatError` | pattern asks for invalid/unknown fields |

Examples:

```zirk
Date(2026, 2, 29);       // InvalidDate
Time(25, 0);             // InvalidTime
TimeZone("Mars/Olympus"); // InvalidTimeZone
task.sleep(-2s);         // non-negative API contract error
```

Parsing APIs return typed expected failures; programmer-constructed invalid
constants can be diagnosed at compile time, and data-dependent failures occur
in a controlled runtime path. Application code should preserve the cause and
offending input in diagnostics without exposing sensitive context.

---

**Previous:** [← Zones and DST](13-zones-and-dst.md) · **Next:** [ Nullability](../04-nullability/README.md)
