# Comparison

`<`, `<=`, `>`, and `>=` use the ordering contract of compatible types.

```zirk
if current_version >= minimum_version {
    enable_feature();
}
```

Comparison is a contract: each operator resolves to a reserved method on the
receiver's type. `<`, `<=`, `>`, and `>=` call `_less`, `_less_equal`,
`_greater`, and `_greater_equal` respectively, and `!=` is the negation of
`_equals`:

```zirk
class Money {
    amount: Int32;
    construct(amount: Int32) { this.amount = amount; }
    fn _less(other: Money): Boolean { return this.amount < other.amount; }
}

Money(3) < Money(5); // true
```

Not every type has a meaningful total order. Comparing operands whose type
declares no corresponding reserved method is a compile-time error — the
compiler rejects the comparison rather than deriving an arbitrary order from
memory layout or identity.

For text, ordering belongs to the documented string or locale API; do not assume that user-facing collation equals raw code-point order.

---

**Previous:** [← Arithmetic](01-arithmetic.md) · **Next:** [ Structural Equality](03-structural-equality.md)
