# Operator Overloading

Zirk permits overloading only through contracts defined by the language. A type cannot invent arbitrary symbolic operators or change an operator's precedence, arity, or evaluation category.

Classes implement the reserved method associated with an operator. `_add`
implements `+` and `_subtract` implements `-`:

```zirk
class Vector2 {
    x: Float64;
    y: Float64;

    fn _add(other: Vector2): Vector2 {
        return Vector2(x + other.x, y + other.y);
    }
}

inmut combined = left + right;
```

User-defined types may implement language-defined operator contracts in safe
code. Native types are owned by the language and cannot be reopened or have
their fundamental operator behavior replaced by application code. Low-level
native extensions remain behind `unsafe`; ordinary operator implementation is
not itself unsafe.

An overload must preserve expectations important to generic code. For example, equality should remain coherent and arithmetic should document overflow and error behavior.

Traditional function overloading is not part of Zirk 1.x. Operators are a narrow, contract-governed exception, not a route to name-based overload sets.

When a named method communicates domain behavior better than a symbol, prefer the method. Operator syntax is most useful for established mathematical or value relationships.

---

**Previous:** [← Ranges](10-ranges.md) · **Next:** [ Control Flow](../06-control-flow/README.md)
