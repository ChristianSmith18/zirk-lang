# Traditional Enums

A traditional enum defines a closed set of named cases without associated payloads.

```zirk
enum Direction {
    North;
    South;
    East;
    West;
}
```

With no explicit mapping, each case exposes its own name as its default string
value and has no implicit numeric index:

```zirk
Direction.North.to_string(); // "North"
```

A case may map explicitly to a string or number:

```zirk
enum CompassCode {
    North -> "N";
    South -> "S";
}

enum ExitCode {
    Success -> 0;
    Failure -> 1;
}
```

`CompassCode.North` remains a `CompassCode`; its associated representation is
`"N"`. It does not become an untyped string. Every mapping in one traditional
enum must use a compatible value type.

Closed cases enable exhaustive matching. Names communicate domain meaning more safely than unrelated constants.

The compiler may choose an efficient internal layout, but source mappings are
observable values and must be preserved. Adding a case can require downstream
exhaustive matches to change.

Cases support equality within the same enum. Declaration or mapped-value order does not silently create `<` or `>`; ordering exists only through an explicit ordering contract. Matching is exhaustive, and an explicit mapping is data rather than an implicit conversion.

Every case exposes `.name` and `.value`. The type supplies `to_string()`,
`from_name()`, and `from_value()`; lookup reports controlled failure and
mappings must be unique. Enums are data-only and cannot declare user methods.
Put domain behavior in an external function and select cases with exhaustive
`match`.

---

**Previous:** [← Value Classes](02-value-classes.md) · **Next:** [ Algebraic Enums](04-algebraic-enums.md)
