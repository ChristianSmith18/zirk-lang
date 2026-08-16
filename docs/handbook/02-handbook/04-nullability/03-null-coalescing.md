# Null Coalescing

`??` evaluates to its left operand when that value is non-null and otherwise evaluates a fallback.

```zirk
inmut label: String = configured_label ?? "Unnamed";
```

The resulting type reflects the alternatives. A non-null fallback can remove nullability; a nullable fallback cannot.

The fallback should be evaluated only when needed so expensive or effectful recovery work is not performed unconditionally.

Choose a fallback only when it represents valid domain behavior. If missing configuration is an error, return a `Result` or diagnose it instead of silently inventing a default.

---

**Previous:** [← Safe Access](02-safe-access.md) · **Next:** [ Flow Analysis](04-flow-analysis.md)
