# Duration

`Duration` is a signed, exact elapsed-time value with nanosecond precision. It is
part of Zirk's temporal family; the full chapter is
[Duration](../03a-temporal/08-duration.md).

## Literals

```zirk
inmut a = 10ns;
inmut b = 500ms;
inmut c = 5s;
inmut d = 2h;
inmut e = 1d;
inmut f = 2w;
```

Supported suffixes: `ns`, `us`, `ms`, `s`, `m`, `h`, `d`, `w`.

## Arithmetic and printing

```zirk
inmut total = 2h + 30m;      // 2h30m
inmut scaled = 3 * 90s;      // 270s
inmut ratio = 3h / 30m;      // 6.0 (Float)

stdout.println(total);       // prints a human-readable form
stdout.println("wait {total}");
```

Negative durations are allowed; `task.sleep` and similar wait APIs reject them.

See the [temporal chapter](../03a-temporal/08-duration.md) for components,
totals, rounding, formatting and `std.time` integration.

## API

`Duration` is a signed, exact elapsed-time value with nanosecond precision.
Literals: `10ns`, `10us`, `500ms`, `5s`, `5m`, `2h`, `1d`, `2w`. The full
component, rounding, formatting, and `std.time` surface is documented in
[Duration](../03a-temporal/08-duration.md).

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `Duration.ZERO` | `Duration` | The zero duration | specified |

### Methods (everyday subset)

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `d.abs()` | `Duration` | Magnitude | specified |
| `d.is_zero()` / `d.is_negative()` / `d.is_positive()` | `Boolean` | Sign tests | specified |
| `d.to_string()` | `String` | Human-readable rendering used by `stdout.println` | implemented |
| `Duration.parse(text)` | `Result<Duration, ParseError>` | Compact and ISO 8601 input | specified |

### Examples

```zirk
inmut total = 2h + 30m;         // 2h30m
inmut scaled = 3 * 90s;         // 270s
inmut ratio = 3h / 30m;         // 6.0 (Float64)
stdout.println("wait {total}");
```

> Literals, arithmetic, and printing are delivered. Component accessors,
> `total_*`/`whole_*` methods, rounding, `format()`, `humanize()`, and
> `to_iso_string()` are specified; see the temporal `Duration` page.

---

**Previous:** [← Void, Never, Null, and Object](10-void-never-null-object.md) · **Next:** [ Regex](12-regex.md)
