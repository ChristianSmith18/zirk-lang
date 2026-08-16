# `match with`

`match with` handles acquisition and scopes the acquired resource.

```zirk
inmut line = match File.open("data.txt") with file {
    Ok(file) => file.read_line();
    Error(error) => "";
};
```

The file closes before the expression delivers its value. Closure occurs on
normal completion, `return`, exception, cancellation, and a `break` or
`continue` that leaves the scope.

Grouped acquisition opens left-to-right and closes right-to-left. If a later
open fails, earlier resources close before the error branch executes. Nested
`match with` remains equivalent and available.

---

**Previous:** [← The Resource Contract](01-resource-contract.md) · **Next:** [ Resource Transfer](03-resource-transfer.md)
