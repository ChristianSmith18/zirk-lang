# Type Patterns

Type patterns select and narrow a value according to its runtime-observable type contract.

```zirk
match value {
    String(text) => stdout.println(text);
    Int32(number) => stdout.println("{number}");
    _ { stdout.println("unsupported"); }
}
```

Inside a branch, the bound value has the narrowed type. The compiler must reject impossible or shadowed patterns and preserve safety across generics and native boundaries.

Use algebraic enums instead when the set of alternatives is closed and owned by the model; they provide stronger exhaustiveness.

---

**Previous:** [← Multiple Patterns](02-multiple-patterns.md) · **Next:** [ Enum Patterns](04-enum-patterns.md)
