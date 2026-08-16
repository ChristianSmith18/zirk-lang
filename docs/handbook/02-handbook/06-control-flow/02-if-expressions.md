# `if` Expressions

An `if` can produce a value when every reachable branch produces compatible types.

```zirk
inmut label: String = if enabled {
    "enabled"
} else {
    "disabled"
};
```

The assignment terminates with a semicolon because the complete `if` is an expression. Omitting a value-producing `else` is invalid when the context requires a result, unless the missing path has type `Never`.

Use an `if` expression for one conceptual choice. If branches primarily perform effects, use statement form and keep produced values explicit.

For a short value choice, prefer the ternary form:

```zirk
inmut label = enabled ? "enabled" : "disabled";
```

An effect-only `if` may govern the immediately following statement without braces:

```zirk
if connection.closed return;

if needs_refresh
    refresh();
```

Only that one statement is conditional. Use braces whenever a branch contains
more than one statement.

---

**Previous:** [← if and else](01-if-and-else.md) · **Next:** [ match](03-match.md)
