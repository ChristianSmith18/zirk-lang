# Value and Reference Behavior

A value assignment produces an independent semantic value. A reference
assignment produces another path to the same instance.

```zirk
mut first: Int32 = 3;
mut second = first;
second += 1;
// first is still 3
```

```zirk
mut first = "hello";
mut second = first;
second[0] = 'H';
// both bindings observe "Hello"
```

## Equality and identity

`==` asks whether values are equal according to `Equatable`. `is` asks whether
two references identify the same observable instance.

```zirk
mut alias = first;
mut copy = first.clone();

first == alias; // true
first is alias; // true
first == copy; // true
first is copy; // false
```

Tuples and enums have value semantics, so `is` is invalid for them. Classes,
`Object`, and `String` have reference identity. Enum equality compares the
case and, for algebraic enums, equal associated values.

## Nominal and structural questions

Classes and enums are nominal: two declarations do not become the same type
merely because their members match. Tuple types are structural value shapes.
“Structural equality” describes how two values of one compatible type compare;
it does not erase a nominal type's name.

## Cloning

`clone()` is not universal. A type implements `Clone` only when it can
produce an independent logical copy. A String clone owns independent mutable
content; a native handle or resource may deliberately be non-cloneable.

See [Bindings and Values](../02-bindings-and-values/README.md) for the binding
permissions layered over these behaviors.

---

**Previous:** [← Type Categories](01a-type-categories.md) · **Next:** [ Contracts and Capabilities](01c-contracts-and-capabilities.md)
