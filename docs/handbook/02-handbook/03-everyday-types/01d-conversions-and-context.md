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

Mixed integer/Decimal arithmetic evaluates in the Decimal domain:

```zirk
3 / 4;    // 0.75: Decimal
3 / 4.0;  // 0.75: Decimal
```

## Final-result conversion

An explicit `T(expression)` conversion evaluates the complete expression first
and converts only its final result:

```zirk
Int32(3 / 4);    // Decimal 0.75, then Int32 0
Float64(3 / 4);  // Decimal 0.75, then Float64 0.75
String(42);      // "42"
```

The target never propagates into operands. For example:

```zirk
Int64(2_000_000_000 + 2_000_000_000);
```

The addition still executes as `Int32` and may overflow before `Int64(...)`
runs. Likewise, `String("value=" + 42)` is invalid because the mixed
concatenation must be valid before its result can be converted. Use
interpolation (`"value={42}"`) or pass separate values to `stdout.print`.

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
