# Conversions and Context

Zirk does not silently perform conversions that may lose range, sign,
precision, identity, or domain meaning. Guaranteed widening may be inferred
when there is one unambiguous target; signed/unsigned and lossy conversions are
explicit and checked.

```zirk
inmut small: Int8 = 10;
inmut wide: Int32 = small;       // safe widening
inmut byte = UInt8(wide);        // explicit, range checked
inmut whole = Int32(3.8);        // explicit, documented truncation/check
```

Mixed integer/Float arithmetic evaluates in the Float domain:

```zirk
3 / 4;    // 0: Int32
3 / 4.0;  // 0.75: Float
```

## Deep contextual conversion

An explicit `Float(...)` constructor changes the compatible arithmetic tree
inside it before operations execute:

```zirk
Float(3 / 4);                 // 0.75
Float((a + 1) / (b * 2));
```

The second expression behaves conceptually like:

```zirk
(Float(a) + Float(1)) / (Float(b) * Float(2));
```

Likewise, `String("value=" + 42)` converts the concatenation operands before
joining them. Outside that explicit context, `"value=" + 42` is an error.

Context does not mutate operands, escape the constructor, or enter a called
function's body. `Float(calculate() / 4)` converts the returned value and the
literal for the division; it does not re-type arithmetic performed inside
`calculate()`.

Parsing text that can fail returns a typed result rather than relying on a cast:

```zirk
mut parsed: Result<Int32, ParseError> = Int32.parse(text);
```

---

**Previous:** [← Contracts and Capabilities](01c-contracts-and-capabilities.md) · **Next:** [ Native Operators](01e-native-operators.md)
