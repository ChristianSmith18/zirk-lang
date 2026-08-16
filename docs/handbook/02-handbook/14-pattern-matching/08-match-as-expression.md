# `match` as an Expression

Expression-form `match` produces the value of the selected branch. Every possible input must be covered, and branch results must have compatible types.

```zirk
inmut message: String = match result {
    Ok(value) => "Value: {value}";
    Error(error) => "Error: {error}";
};
```

There is no special `yield` or `capture`; the branch's expression is its result. The terminating semicolon belongs to the surrounding binding declaration.

When a branch cannot return normally, its `Never` type can coexist with the value-producing branches without fabricating a value.

---

**Previous:** [← match as a Statement](07-match-as-statement.md) · **Next:** [ Classes and Objects](../08-classes-and-objects/README.md)
