# `match with`

`match with` handles acquisition and scopes the acquired resource.

```zirk
inmut line = match with File.open("data.txt") {
    Ok(file) => file.read_line();
    Error(error) => "";
};
```

The file closes before the expression delivers its value. Closure occurs on every exit path, including exception, `return`, and cancellation.

---

**Previous:** [← The Resource Contract](./01-resource-contract.md) · **Next:** [Resource Transfer →](./03-resource-transfer.md)
