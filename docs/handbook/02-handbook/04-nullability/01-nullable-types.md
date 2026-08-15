# Nullable Types

Append `?` when absence is a valid state:

```zirk
mut selected_name: String? = null;
inmut required_name: String = "Zirk";
```

`String?` is equivalent to `String | Null`. `String` cannot hold `null`, and Zirk has no `undefined`.

This is invalid:

```zirk
inmut name: String = null;
```

The compiler should explain that `Null` is not assignable to `String` and suggest either a real string initializer or an explicitly nullable type. Do not make a type nullable merely to postpone deciding what absence means.

---

**Previous:** [← Nullability](./README.md) · **Next:** [Safe Access →](./02-safe-access.md)
