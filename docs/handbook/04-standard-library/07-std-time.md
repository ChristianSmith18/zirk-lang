# `std.time`

`std.time` separates civil values, timeline instants, exact durations, calendar
periods and clock sources. Every value is immutable; transformations return new
values. The detailed arithmetic contract belongs to the temporal-type chapters.

> **Implementation status:** `Duration` literals, arithmetic, comparison,
> `Timer`, `Timer.sleep`, `Timer.after`, and `Timer.every` are delivered. Civil
> and zone-aware types, IANA data, virtual clocks, and `Ticker` remain planned.

## Getting the current time

Temporal types provide the friendly API; `Clock` supplies an injectable source:

```zirk
inmut instant = Instant.now();                 // Clock.system
inmut date = Date.now(zone:);
inmut time = Time.now(zone:);
inmut current = ZonedDateTime.now(zone:);
```

`Time` is only a time-of-day value, so it does not silently guess a zone.
`Time.now_local()` and corresponding local helpers return `Result` because
system-zone discovery can fail. Tests pass `clock: virtual_clock` without
changing global or system time.

`Clock.monotonic` produces monotonic instants for elapsed measurement, timeout
and benchmarks; wall-clock corrections cannot make it run backward.

```zirk
inmut measurement: Measurement<Report> = Clock.monotonic.measure(fn() {
    build_report()
});
println(measurement.elapsed);
```

## Zones and parsing

`TimeZone` uses versioned IANA identities. Zirk projects carry a reproducible
timezone-data version rather than silently changing with the host. Fixed offsets
remain distinct from zones. Parsing is strict: invalid dates are errors, never
automatically repaired. ISO and explicit deterministic formats are standard;
localized calendars/names belong to an official internationalization package.

## Duration and calendar arithmetic

`Duration` is signed exact elapsed time; `Period` is calendar-relative years,
months, weeks and days. They do not convert implicitly. Differences may produce
negative durations, while timer sleep and interval construction require a
non-negative duration. Overflow, ambiguous local time and nonexistent DST time
are typed failures resolved through explicit policies.

## Timers

```zirk
concurrent {
    Timer.after(5s, (): Void => report_timeout());
    Timer.every(1s, (): Void => refresh_status());
    Timer.sleep(500ms);
}
```

`Timer.sleep` suspends the current branch and is a cancellation safe point.
`Timer.after` and `Timer.every` return scope-owned `Job<T>` handles; the nearest
`concurrent` scope cancels ambient timers when it closes. Calendar/cron
scheduling is intentionally an official package rather than `std.time`.

---

**Previous:** [← std.collections](06-std-collections.md) · **Next:** [ std.thread](09-std-thread.md)
