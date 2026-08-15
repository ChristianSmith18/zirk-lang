# Numeric Literals

Numeric literals may use `_` separators for readability and scientific notation for scale:

```zirk
inmut population = 1_000_000;
inmut tiny = 1e-6;
```

Separators do not change value. Their placement must remain lexically valid; they cannot be used to join otherwise separate tokens.

Context can select a concrete width:

```zirk
inmut mask: UInt16 = 65_535;
```

When no context exists, the language uses its default signed or decimal type. If the value cannot fit, compilation should report the range and suggest a suitable type rather than truncate silently.

---

**Previous:** [← Decimals](./04-decimals.md) · **Next:** [Overflow and Arithmetic Safety →](./06-overflow-and-arithmetic-safety.md)
