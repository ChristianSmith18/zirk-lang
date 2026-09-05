# `std.time`

`std.time` separates civil values, timeline instants, exact durations, calendar
periods and clock sources. Every value is immutable; transformations return new
values. The detailed arithmetic contract belongs to the temporal-type chapters.

> **Implementation status:** accepted Zirk 1.x contract. `Duration` literals,
> arithmetic, comparison and `to_string()` are delivered by
> `array-list-tuple-duration-regex`; the civil/zone temporal types, IANA data,
> virtual clocks and timers remain ahead of the current runtime.

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
negative durations, while sleep, timeout and interval construction require a
non-negative duration. Overflow, ambiguous local time and nonexistent DST time
are typed failures resolved through explicit policies.

## Timers

```zirk
await Timer.after(5s);

match Ticker.every(1s, policy: TickerPolicy.FixedRate) with ticker {
    Ok(ticker) => consume_ticks(ticker),
    Error(error) => report(error),
}
```

`Timer` and `Ticker` are task-aware resources. `FixedRate` follows the original
schedule and reports missed intervals; `FixedDelay` waits after each completed
consumption. A tick records scheduled/observed instants and `missed`. Waiting is
cancelable and never reserves a thread. Calendar/cron scheduling is intentionally
an official package rather than `std.time`.

---

**Previous:** [← std.collections](06-std-collections.md) · **Next:** [ std.task](08-std-task.md)
