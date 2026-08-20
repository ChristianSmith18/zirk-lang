# Multiple Bindings and Simultaneous Assignment

Comma-grouped syntax handles several bindings without turning them into a
tuple. One declaration can apply the same permission and type to several names:

```zirk
mut first, second, third: String;
```

Each binding is independent and starts with `String`'s default value. An
initializer list must contain exactly one expression for every name:

```zirk
mut width, height: Int32 = 1280, 720;
inmut host, scheme: String = "example.com", "https";
```

`mut width, height: Int32 = 1280;` is invalid. Zirk neither repeats the last
initializer nor leaves one binding accidentally uninitialized. When no
initializer is present, the declared type is required so the compiler knows
which defaults to use.

## Simultaneous assignment

Use the same comma shape to replace several mutable destinations:

```zirk
mut left, right: Int32 = 3, 4;
left, right = right, left;
```

The result is `left == 4` and `right == 3`. The runtime does not perform the
first write and then read the changed value for the second source. Instead, the
language guarantees this order:

1. evaluate every right-hand expression exactly once, from left to right;
2. verify their values against corresponding destinations;
3. write the destinations from left to right.

Source and destination counts must match exactly, so this is invalid:

```zirk
left, right = right, left, 5;
```

The compiler reports both counts and points at the unmatched expression. A
destination may not appear twice because two writes to the same place would
make the result depend on commit order.

## Binding and referent permissions

Every destination must already be writable. A whole `inmut` reference cannot
be rebound, and neither can an immutable primitive:

```zirk
inmut name = "Ada";
mut other = "Grace";
name, other = other, name; // compile-time error: name cannot be rebound
```

An `inmut` reference may still permit mutation of its referent, so writable
projected places can participate when their ordinary assignment would be
legal. `inmut::strict` freezes the reachable graph and rejects those writes:

```zirk
inmut::strict values = [1, 2];
values[0], values[1] = values[1], values[0]; // compile-time error
```

Projection reads on the right keep the normal Zirk rule: they produce
independent values, requiring `Clone` for reference-backed results. Projected
destinations on the left remain places in original storage.

This feature is not destructuring. Destructuring introduces new names by
matching a product shape; simultaneous assignment updates existing places with
an exact positional list.

> **Implementation status:** this chapter defines final Zirk 1.x behavior.
> Compiler delivery is owned by the callable-and-binding completion roadmap
> slice and must include grammar, type, flow, IR, diagnostic, and backend tests.

---

**Previous:** [← Destructuring](07-destructuring.md) · **Next:** [ Shadowing](08-shadowing.md)
