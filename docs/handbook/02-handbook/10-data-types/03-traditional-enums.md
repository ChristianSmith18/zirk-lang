# Traditional Enums

A traditional enum defines a closed set of named cases without associated payloads. The name after `enum` is the type:

```zirk
enum Direction {
    North;
    South;
    East;
    West;
}
```

`Direction` is now a nominal type. Its cases are accessed through the type:

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

Every case exposes `.name` and `.value`. The type itself exposes built-in static
members for reflection and conversion. Enums cannot declare user-defined
methods; put domain behavior in an external function or `class` and select cases
with exhaustive `match`.

```zirk
Direction.keys();       // ["North", "South", "East", "West"]
Direction.values();     // [Direction.North, Direction.South, Direction.East, Direction.West]
Direction.count;        // 4
Direction.from_name("North");  // Ok(Direction.North) or Err
ExitCode.from_value(0);        // Ok(ExitCode.Success)

// Generic helper when the enum type is not known statically.
Enums.keys(Direction);
Enums.values(ExitCode);
Enums.count(CompassCode);
```

## API

A closed set of named cases without payloads; optional `->` mapping to a
string or number representation. The enum type itself is a nominal type with
built-in static members; user-defined methods are not allowed.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `case.name` | `String` | Declared case name | implemented |
| `case.value` | `Int32` (or mapped type) | Discriminant / explicit mapping value | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `case.to_string()` | `String` | Case name | implemented |
| `Direction.keys()` | `List<String>` | All case names in declaration order | implemented |
| `Direction.values()` | `List<Direction>` | All case values in declaration order | implemented |
| `Direction.count` | `Int32` | Number of cases | implemented |
| `Direction.from_name(name)` | `Result<Direction, LookupError>` | Name lookup; controlled failure | implemented |
| `Direction.from_value(value)` | `Result<Direction, LookupError>` | Mapped-value lookup; mappings must be unique | implemented |
| `Direction.to_string()` | `String` | Renders the enum type name | specified |
| `Enums.keys(enum_type)` | `List<String>` | Generic helper, equivalent to `T.keys()` | implemented |
| `Enums.values(enum_type)` | `List<T>` | Generic helper, equivalent to `T.values()` | implemented |
| `Enums.count(enum_type)` | `Int32` | Generic helper, equivalent to `T.count` | implemented |

Cases support equality within the same enum; ordering exists only through an
explicit ordering contract. `match` is exhaustive.

### Examples

```zirk
enum Direction { North; South; East; West; }
enum ExitCode { Success -> 0; Failure -> 1; }

Direction.North.to_string();    // "North"
ExitCode.Success.value;         // 0
Direction.from_name("North");   // Ok(Direction.North) or Err
Direction.keys();               // ["North", "South", "East", "West"]
Direction.values();             // [North, South, East, West]
Direction.count;                  // 4

Enums.keys(Direction);
Enums.values(ExitCode);
```

---

**Previous:** [← Records](01-records.md) · **Next:** [ Algebraic Enums](04-algebraic-enums.md)
