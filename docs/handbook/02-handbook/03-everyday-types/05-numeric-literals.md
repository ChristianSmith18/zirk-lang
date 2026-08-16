# Numeric Literals

Whole literals default to `Int32` and fractional/scientific literals to
`Float64` when no context chooses another representable type.

```zirk
inmut count = 42;          // Int32
inmut ratio = 0.5;        // Float64
inmut scale = 1e3;        // Float64
inmut mask: UInt16 = 65_535;
```

Use constructors rather than suffixes to request widths:

```zirk
Int8(10)
UInt64(1_000)
Float32(3.14)
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
