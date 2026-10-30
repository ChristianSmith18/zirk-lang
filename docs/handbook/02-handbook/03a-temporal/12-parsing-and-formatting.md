# Temporal Parsing and Formatting

Implemented temporal parsers accept canonical ISO forms where the type's
meaning is unambiguous. Civil `Date`/`Time`/`DateTime` also provide localized
`parse_locale(text, locale, style)` and `format_locale(locale, style)` with
explicit BCP 47 locales and bundled ICU4X 2.4.0 / CLDR 49 data. The localized
parser uses the selected locale's exact CLDR style; it does not guess a style
or a century for two-digit years.

```zirk
import { stdout } from std.io;
import { Date, Instant, Period, Time } from std.time;

fn main(): Void {
    match Date.parse("2026-08-15") {
        Ok(value) => stdout.println(value.to_iso_string());
        Error(_) => stdout.println("invalid date");
    };
    match Date.parse_locale("May 17, 2026", "en-US", "long") {
        Ok(value) => stdout.println(value.to_iso_string());
        Error(error) => stdout.println(error.message());
    };
    match Time.parse_locale("4:12:20 PM", "en-US", "medium") {
        Ok(value) => stdout.println(value.to_iso_string());
        Error(error) => stdout.println(error.message());
    };
    match Instant.parse("2026-08-15T18:30:00Z") {
        Ok(value) => stdout.println(value.to_iso_string());
        Error(_) => stdout.println("invalid instant");
    };
    stdout.println(2h + 30m);
    match Period.parse("P1Y2M3D") {
        Ok(value) => stdout.println(value.to_iso_string());
        Error(_) => stdout.println("invalid period");
    };
}
main();
```

Parsing validates complete input, ranges, leap days, offsets and zone/offset
consistency. It never repairs invalid values or uses locale heuristics without
an explicit locale. Localized input accepts the numbering system selected by
the BCP 47 tag, including Unicode decimal digits. A two-digit year returns
`AmbiguousLocalizedInputError`; malformed text returns
`InvalidLocalizedInputError`; malformed/unsupported locale tags and styles
remain `LocaleError` outcomes. Existing ISO `parse` methods keep their
locale-neutral behavior.

`to_iso_string()` emits the canonical machine form. Implemented
`format(pattern)` methods are for presentation; their pattern symbols are
shared consistently across types but only meaningful components are accepted.
Formatting a Date with an hour token is an error rather than an empty
substitution.

```zirk
import { stdout } from std.io;
import { Date, Duration } from std.time;

fn main(): Void {
    inmut date = Date(2026, 8, 15);
    inmut duration = 90m;
    stdout.println(date.format("DD/MM/YYYY"));
    stdout.println(duration.humanize(locale: "es", max_units: 2)); // English labels currently
}
main();
```

`DateTime.format_locale_in_zone(zone, locale, style, overlap, gap)` is
available for localized rendering with an explicit IANA zone or fixed offset.
The overlap choices are `reject`, `earlier`, and `later`; gap choices are
`reject`, `forward`, and `backward`. It reports ambiguous/nonexistent local
times as errors. `ZonedDateTime.parse` is available for RFC 3339 timestamps
with bracketed zone identifiers and validates the supplied offset against
pinned rules. Localized parse APIs cover civil date, time, and date-time; they
do not parse a time-zone name into a zoned value. `Duration.humanize` accepts
`locale:` for forward compatibility, but currently ignores it.

The API tables in the temporal chapters label each member `implemented` or
`specified`; a `specified` row describes the target contract and is not
available in the current compiler/runtime.

Serialization should prefer ISO forms plus explicit zone IDs/version context
where replay depends on zone rules. Humanized strings are not stable machine
formats.

---

**Previous:** [← Temporal Arithmetic](11-temporal-arithmetic.md) · **Next:** [ Zones and DST](13-zones-and-dst.md)
