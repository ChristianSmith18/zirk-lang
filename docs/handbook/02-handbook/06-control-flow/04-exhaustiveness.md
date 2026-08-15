# Exhaustiveness

An expression-valued `match` must account for every value permitted by the scrutinee's static type.

```zirk
inmut text = match result {
    Ok(value) => "Value: {value}";
    Error(error) => "Error: {error}";
};
```

If another variant is possible, the compiler rejects the expression and identifies the missing cases. Exhaustiveness turns model evolution into a useful compile-time signal: adding an enum variant reveals every decision that needs reconsideration.

A broad fallback can be appropriate for open domains, but avoid it for closed enums when listing variants preserves intent.

---

**Previous:** [← `match`](./03-match.md) · **Next:** [`for` →](./05-for.md)
