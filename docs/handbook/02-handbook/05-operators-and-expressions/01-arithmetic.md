# Arithmetic

Zirk supports `+`, `-`, `*`, `/`, `%`, and `**` for types whose contracts define those operations. `**` raises its left operand to the power supplied on the right.

```zirk
inmut subtotal = price * quantity;
inmut remainder = items.count % page_size;
inmut cube = 2 ** 3; // 8
```

Operands must have compatible types. Zirk does not silently choose a lossy numeric conversion. Division by zero and overflow follow controlled error contracts rather than undefined behavior.

Unary `-` applies only when the type supports a negative value; it is invalid for unsigned integers without an explicit conversion or operation.

Exponentiation binds more tightly than multiplication. Its compound form is
`**=` and requires a mutable left operand.

---

**Previous:** [← Operators and Expressions](README.md) · **Next:** [ Comparison](02-comparison.md)
