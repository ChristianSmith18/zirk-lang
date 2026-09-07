# Records

A record groups named fields into structural data.

```zirk
record Point {
    x: Float;
    y: Float;
}
```

Records suit messages, configuration, coordinates, and other data whose
attributes are the primary contract. Construction is named-only; an omitted
attribute receives its declared type's default.

```zirk
inmut first = Point(x: 10.0, y: 20.0);
inmut second = Point(x: 10.0, y: 20.0);

first == second; // true: equal field values
```

A record is immutable after construction. Updating one field means constructing
a new record, commonly with destructuring:

```zirk
inmut { x, y } = first;
inmut moved = Point(x: x + 1.0, y: y);
```

Records have a nominal declared type but value-oriented, structural semantics:
two records of different declared types are not interchangeable merely because
their fields happen to look alike.

Records may define non-mutating methods that compute from their contents. They
cannot declare a custom `construct`, mutate an attribute, inherit from a class,
or acquire reference identity. Use an external factory returning `Result` when
creation requires validation beyond the built-in type checks.

Derived equality compares every field. Derived hashing is available only when every field is hashable and must agree with equality: equal records always hash equally. A record containing an unhashable field can still exist, but cannot derive hashing or serve as a hashed collection key.

Prefer a class when observable identity, lifecycle, inheritance, or substantial encapsulated behavior is central. Prefer an algebraic enum when a value is one of several variants rather than one fixed product of fields.

## API

A nominal type with structural value semantics: immutable after construction,
named-only construction, derived equality (and hashing when every field is
hashable).

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `type` | `Type` | Universal member | specified |
| declared fields | declared types | Read-only projections | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Point(x:, y:)` | `Point` | Named-only construction; omitted fields take the declared default | implemented |
| `r.to_string()` | `String` | Universal member | implemented |
| `r.clone()` | `Point` | Derived `Clone` when every field is `Clone` | implemented |
| user methods | varies | Non-mutating computed methods only | implemented |

Records cannot declare a custom `construct`, mutate an attribute, inherit from
a class, or acquire reference identity. `==` compares every field (delivered by
`fase-3-structural-equality`). Use an external factory returning `Result` for
validation beyond built-in checks.

### Examples

```zirk
record Point { x: Float; y: Float; }

inmut first = Point(x: 10.0, y: 20.0);
first == Point(x: 10.0, y: 20.0);   // true

inmut { x, y } = first;
inmut moved = Point(x: x + 1.0, y: y);
```

---

**Previous:** [← Tuples](00-tuples.md) · **Next:** [Traditional Enums →](03-traditional-enums.md)
