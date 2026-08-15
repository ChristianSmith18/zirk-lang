# Decimals

Zirk provides `Decimal16`, `Decimal32`, `Decimal64`, and `Decimal128`, with `Dec` and `Decimal` as aliases for the default decimal type.

```zirk
inmut ratio: Decimal64 = 0.625;
inmut threshold: Decimal128 = 1e-20;
```

Choose a width based on range, precision, interoperability, and performance. Do not assume that a decimal value exactly represents every human base-ten quantity; the specialized numeric contract determines representation and rounding.

Potentially lossy conversions between decimal widths or from decimals to integers are explicit. Comparisons and equality follow the type's defined semantics rather than textual spelling.

---

**Previous:** [← Unsigned Integers](./03-unsigned-integers.md) · **Next:** [Numeric Literals →](./05-numeric-literals.md)
