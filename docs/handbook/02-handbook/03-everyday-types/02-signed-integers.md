# Signed Integers

Zirk provides `Int8`, `Int16`, `Int32`, `Int64`, and `Int128`. `Int` and `Integer` alias the default signed integer type selected by the language contract.

```zirk
inmut temperature: Int32 = -12;
inmut distance: Int64 = 4_000_000_000;
```

Choose an explicit width for serialization, ABI boundaries, persistent formats, or arithmetic whose range is part of the contract. Use the default type for ordinary local values when its range is sufficient.

Conversions that may lose range or sign are not implicit. Ordinary overflow produces a controlled error; wrapping, saturating, and checked behavior require explicit operations.

---

**Previous:** [← Object and Type Hierarchy](./01-object-and-type-hierarchy.md) · **Next:** [Unsigned Integers →](./03-unsigned-integers.md)
