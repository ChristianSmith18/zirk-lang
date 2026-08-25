# Contracts and Capabilities

Zirk derives operations from contracts rather than assuming that a broad base
class provides every method. Typical capabilities include:

```text
Equatable<Self>          Comparable<Self>
Hashable                 Clone
Addable<Self, Output>    Subtractable<Self, Output>
Multipliable<Self, Output>
Dividable<Self, Output>  Remainder<Self, Output>
Exponentiable<Self, Output>
Iterable<Element>        Iterator<Element>
Temporal
```

The exact generic signature states both accepted operands and the result. A
Duration divided by another Duration yields `Float`, while a Duration divided
by a scalar yields Duration; sharing `/` does not require sharing a result type.

## Reserved operator methods

User-defined types participate through language-reserved methods such as
`_add`, `_subtract`, `_multiply`, and `_divide`. Safe code may implement them on
its own types. It may not reopen `String`, `Int32`, or another native type to
replace fundamental behavior.

```zirk
class Vector implements Addable<Vector, Vector> {
    fn _add(other: Vector): Vector {
        return Vector(x + other.x, y + other.y);
    }
}
```

An implementation cannot change an operator's spelling, precedence,
associativity, arity, short-circuit behavior, or evaluation category.

## Universal surface

Every value exposes `type` and `to_string()`. Equality, hashing, cloning,
ordering, arithmetic, and iteration require their corresponding capabilities.
This is why a resource handle need not clone and why an enum has no implicit
order.

---

**Previous:** [← Value and Reference Behavior](01b-value-and-reference-behavior.md) · **Next:** [ Conversions and Context](01d-conversions-and-context.md)
