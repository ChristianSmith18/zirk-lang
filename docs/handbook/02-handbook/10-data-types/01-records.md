# Records

A record groups named fields into structural data.

```zirk
record Point {
    x: Float64;
    y: Float64;
}
```

Records suit messages, configuration, coordinates, and other data whose fields are the primary contract. Their construction must provide required fields with compatible types. Structural equality compares the fields defined by the record contract.

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

Derived equality compares every field. Derived hashing is available only when every field is hashable and must agree with equality: equal records always hash equally. A record containing an unhashable field can still exist, but cannot derive hashing or serve as a hashed collection key.

Prefer a class when observable identity, lifecycle, inheritance, or substantial encapsulated behavior is central. Prefer an algebraic enum when a value is one of several variants rather than one fixed product of fields.

---

**Previous:** [← Data Types](README.md) · **Next:** [ Value Classes](02-value-classes.md)
