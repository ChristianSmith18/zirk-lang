# Numeric Literals

Whole literals default to `Int32` and a suffix-less fractional/scientific
literal is an exact `Float`. A `b` suffix makes it a `BinaryFloat`.

```zirk
inmut count = 42;          // Int32
inmut ratio = 0.5;         // Float (exact base-ten)
inmut scale = 1e3;         // Float
inmut fast = 1.5b;         // BinaryFloat64
inmut compact = 1.5b32;    // BinaryFloat32
inmut mask: UInt16 = 65_535;
```

Use constructors rather than suffixes to request integer widths:

```zirk
Int8(10)
UInt64(1_000)
BinaryFloat32(3.14)   // or the literal 3.14b32
```

`_` may group digits without changing the value. It cannot appear first, last,
twice consecutively, beside the decimal point, or beside the exponent marker
or sign.

```zirk
1_000_000 // valid
0xFF_FF   // valid
1__000    // lexical error
1_.5      // lexical error
```

Scientific `e` denotes a power of ten; the formatter normalizes uppercase `E`
to `e`. A compile-time literal outside the selected type's range is diagnosed
before execution rather than truncated.

Duration literals (`500ms`, `2h`) are typed temporal values, not numeric
suffixes. See [Duration](../03a-temporal/08-duration.md).

---

**Previous:** [← Floats](04-decimals.md) · **Next:** [ Overflow and Arithmetic Safety](06-overflow-and-arithmetic-safety.md)
