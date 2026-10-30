# `std.time`

`std.time` separates civil values, timeline instants, exact durations, calendar
periods and clock sources. Every value is immutable; transformations return new
values. The detailed arithmetic contract belongs to the temporal-type chapters.

> **Implementation status:** `Duration` literals, arithmetic, comparison,
> `Timer`, `Timer.sleep`, `Timer.after`, `Timer.every`, and the civil `Date`,
> `Time`, and `DateTime` types are delivered. `Instant` is partially delivered:
> RFC 3339 parsing, Unix seconds/milliseconds/nanoseconds factories, UTC
> rendering, exact Unix-unit properties, ordering, exact `Duration` arithmetic,
> and native `Instant.now()`
> work in native execution. The browser interpreter denies ambient clock
> access. `TimeZone` parsing, canonical identifiers, database version, local
> projection, and instant-specific offset/DST/abbreviation queries are available
> for pinned IANA names and fixed offsets. `ZonedDateTime` supports checked zoned
> parsing, conversion from `Instant`, instant-preserving zone changes, local
> projection, and instant comparison. `Date`, `Time`, and `DateTime` expose
> explicit-locale formatting with bundled ICU4X 2.4.0 / CLDR 49 data; `DateTime`
> can resolve local time into a zone with explicit overlap/gap policies and
> format that local time. Both local-time APIs return `LocaleError` for invalid
> policies, ambiguous times, or nonexistent times. `Period` exposes constructors,
> component access, ISO parsing, and formatting. `TimeShift` exposes clock and
> calendar forms, nullable form-specific fields, and ordered application to a
> `ZonedDateTime`; native/interpreter behavior has local parity evidence. The
> supported-host temporal matrix and localized parsing are still open. The examples below describe the
> target contract wherever the listed API is not marked as delivered.

## Getting the current time

Temporal types provide the civil and timeline APIs:

```zirk
inmut instant = Instant.now(); // Clock.system
inmut date = Date.today();
inmut time = Time.now_local();
inmut current = DateTime.now_local();
```

`Time` is only a time-of-day value, so it does not silently guess a zone.
`Instant.now()` is available in native execution. Browser interpretation
denies host-clock access. `Date.today()`, `Time.now_local()`, and
`DateTime.now_local()` use the native host clock and local zone.

`Clock.monotonic` produces monotonic instants for elapsed measurement, timeout
and benchmarks; wall-clock corrections cannot make it run backward.

```zirk
inmut measurement: Measurement<Report> = Clock.monotonic.measure(fn() {
    build_report()
});
println(measurement.elapsed);
```

## Instants, zones and parsing

The delivered `Instant` factories take signed Unix seconds, milliseconds, or
nanoseconds and return `Result<Instant, ArithmeticOverflowError>`. Equivalent
timestamps in different units identify the same instant; nanoseconds retain
their exact precision.

```zirk
import { stdout } from std.io;

fn main(): Void {
    match Instant.from_unix_seconds(0) {
        Ok(epoch) => stdout.println(epoch);
        Error(_) => stdout.println("timestamp out of range");
    };
    match Instant.from_unix_nanoseconds(1500000000) {
        Ok(value) => stdout.println(value);
        Error(_) => stdout.println("timestamp out of range");
    };
}
main();
```

`DateTime.format_locale_in_zone(zone, locale, style, overlap, gap)` uses a
versioned IANA zone or an explicit fixed offset, a BCP 47 locale, and one of
`short`, `medium`, `long`, or `full`. The overlap policy is `reject`, `earlier`,
or `later`; the gap policy is `reject`, `forward`, or `backward`. Invalid,
ambiguous, and nonexistent local times return a `LocaleError` result instead
of silently choosing a value. Names and offset text use bundled ICU4X 2.4.0 /
CLDR 49 data, independent of the host locale. Localized parsing remains
unavailable. `TimeZone.parse` accepts IANA identifiers, `UTC`/`Z`, and fixed
offsets in `±HH:MM[:SS]` form. It canonicalizes IANA aliases and exposes
`canonical_identifier`, `offset_seconds_at(instant)`, `is_dst_at(instant)`,
`abbreviation_at(instant)`, and `local_datetime_at(instant)`. Its
`database_version()` method reports the pinned IANA rules. `ZonedDateTime.parse`
checks an RFC 3339 offset against its bracketed zone. `ZonedDateTime.from_instant`
and `Instant.in_zone` project an instant; `with_zone` preserves it while
changing the view. Its properties expose the instant, zone, local `DateTime`,
offset, DST state, and abbreviation. `Period` exposes calendar component
construction, ISO parsing, and ISO output. `TimeShift` is one tagged type:
clock shifts from `Time ± Duration` retain the wrapped time and signed day
offset, while calendar shifts apply a `Period`, resolve DST with explicit
policies, and then apply exact elapsed time. The inactive form's fields are
null. A local `DateTime.in_zone` requires
`reject`, `earlier`, or `later` for overlap handling and `reject`, `forward`,
or `backward` for gap handling.

## Duration and calendar arithmetic

`Duration` is signed exact elapsed time; `Period` is calendar-relative years,
months, weeks and days. They do not convert implicitly. Differences may produce
negative durations, while timer sleep and interval construction require a
non-negative duration. Overflow, ambiguous local time and nonexistent DST time
are typed failures resolved through explicit policies.

`std.time` exports the canonical aliases `Instant`, `Duration`, `Period`,
`TimeShift`, `TimeZone`, and `ZonedDateTime`. Existing unqualified built-in
spellings remain supported.

## Timers

```zirk
import { stdout } from std.io;
import { Timer } from std.time;

fn report_timeout(): Void { stdout.println("timeout"); }
fn refresh_status(): Void { stdout.println("refresh"); }

fn main(): Void {
    concurrent {
        Timer.after(5s,(): Void => report_timeout());
        Timer.every(1s,(): Void => refresh_status());
        Timer.sleep(500ms);
    }
}
main();
```

`Timer.sleep` suspends the current branch and is a cancellation safe point.
`Timer.after` and `Timer.every` return scope-owned `Job<T>` handles; the nearest
`concurrent` scope cancels ambient timers when it closes. Calendar/cron
scheduling is intentionally an official package rather than `std.time`.

---

**Previous:** [← std.collections](06-std-collections.md) · **Next:** [ std.thread](09-std-thread.md)
