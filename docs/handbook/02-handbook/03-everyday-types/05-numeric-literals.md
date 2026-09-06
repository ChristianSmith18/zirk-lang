# Numeric Literals

Whole literals default to `Int32` and a suffix-less fractional/scientific
literal is an exact `Decimal`. An `f` suffix makes it a `Float`; a `d` suffix
is an explicit `Decimal`.

```zirk
inmut count = 42;          // Int32
inmut ratio = 0.5;         // Decimal (exact base-ten)
inmut scale = 1e3;         // Decimal
inmut exact = 1.5d;        // Decimal
inmut fast = 1.5f;         // Float64
inmut faster = 1.5f;       // Float64
inmut compact = 1.5f32;    // Float32
inmut compact2 = 1.5f32;   // Float32
inmut mask: UInt16 = 65_535;
```

Use constructors rather than suffixes to request integer widths:

```zirk
Int8(10)
UInt64(1_000)
Float32(3.14)   // or the literal 3.14f32
Decimal(3.14)   // exact base-ten
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
