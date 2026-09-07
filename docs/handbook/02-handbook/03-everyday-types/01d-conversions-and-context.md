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

## Checked and nullable casts

`as` performs a checked cast between related types (a prefix `<Type>`
form exists as well). It verifies the runtime type and fails the operation
on a mismatch:

```zirk
mut a: Animal = Dog();
mut d = a as Dog; // verifies a is a Dog
```

When failure is an ordinary outcome, `as?` yields a nullable result instead:
`x as? T` has type `T?` and evaluates to `null` when the runtime type does not
match, so the result composes with `match`, `?.`, and `??`:

```zirk
mut maybeDog: Dog? = a as? Dog;
match maybeDog {
    null => stdout.println("no dog"),
    d => stdout.println(d.speak()),
}
```

A cast between unrelated types is rejected at compile time for both forms;
`as?` is not a way to attempt impossible conversions.

---

**Previous:** [← Contracts and Capabilities](01c-contracts-and-capabilities.md) · **Next:** [ Native Operators](01e-native-operators.md)
