# Comparison

`<`, `<=`, `>`, and `>=` use the ordering contract of compatible types.

```zirk
if current_version >= minimum_version {
    enable_feature();
}
```

Not every type has a meaningful total order. A compiler should reject comparison when no contract exists rather than derive an arbitrary order from memory layout or identity.

For text, ordering belongs to the documented string or locale API; do not assume that user-facing collation equals raw code-point order.

---

**Previous:** [← Arithmetic](01-arithmetic.md) · **Next:** [ Structural Equality](03-structural-equality.md)
