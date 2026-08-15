# Unsigned Integers

`UInt8`, `UInt16`, `UInt32`, `UInt64`, and `UInt128` represent non-negative fixed-width integers.

```zirk
inmut request_id: UInt64 = 42;
inmut byte: UInt8 = 255;
```

Unsigned types are appropriate for bit patterns, protocol fields, sizes with explicit contracts, and native APIs. They are not automatically the best type for every count: subtracting from zero and mixing signed values still require careful semantics.

A negative literal cannot initialize an unsigned value. Cross-sign conversions that might lose information must be explicit and checked where necessary.

---

**Previous:** [← Signed Integers](./02-signed-integers.md) · **Next:** [Decimals →](./04-decimals.md)
