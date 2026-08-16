# Boolean

`Boolean` is an inline primitive with exactly `true` and `false`. Conditions
require it directly; Zirk has no numeric, text, collection, null or object
truthiness.

```zirk
inmut ready: Boolean = items.length > 0;
if ready {
    process(items);
}

if items.length { } // error: expected Boolean, received Int32
```

## Operators

Boolean supports `!`, equality, and short-circuit `&&`/`||`. The right operand
of `&&` executes only when the left is true; the right operand of `||` executes
only when the left is false.

```zirk
if user != null && user.is_active { }
```

It has no arithmetic or ordering:

```zirk
true + false; // error
true < false; // error
```

`Boolean?` also admits `null` and must be narrowed or coalesced before use as a
condition. There is no implicit conversion from `0`, `1`, `"true"`, or an
empty/non-empty value. The intentionally small API contains `to_string()`,
`type`, equality and formatting inherited through common contracts.

---

**Previous:** [← Overflow and Arithmetic Safety](06-overflow-and-arithmetic-safety.md) · **Next:** [ Char](08-char.md)
