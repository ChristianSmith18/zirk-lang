# Union Patterns

A union value can be narrowed by matching each member type or value form.

```zirk
fn render(value: String | Int32): String {
    return match value {
        String(text) => text;
        Int32(number) => "{number}";
    };
}
```

The expression is exhaustive because both union members are covered. If a union later widens, the compiler identifies matches that lack the new alternative.

Avoid a wildcard when each union member deserves distinct domain behavior; explicit branches preserve the reason the union exists.

---

**Previous:** [← Enum Patterns](04-enum-patterns.md) · **Next:** [ Destructuring Patterns](06-destructuring-patterns.md)
