# Safe Access

The `?.` operator accesses a member only when the receiver is non-null. If the receiver is `null`, the entire access evaluates to `null`.

```zirk
inmut display_name: String? = user?.profile?.name;
```

Safe access propagates nullability. It does not assert that a value exists, throw implicitly, or produce a non-null member type.

Use it when absence should flow through the expression. If absence requires different behavior, narrow with control flow, match the nullable value, or add `??` with a meaningful fallback.

Avoid long chains when different missing links require different diagnostics; explicit branches communicate those cases better.

---

**Previous:** [← Nullable Types](01-nullable-types.md) · **Next:** [ Null Coalescing](03-null-coalescing.md)
