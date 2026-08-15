# `map`

`map` transforms each input element into one output element.

```zirk
inmut names = users.map((user: User): String => user.name);
```

The output element type is the lambda's result type. Whether the returned operation is lazy or eager belongs to the collection or iterator API; callers should not assume allocation behavior from the method name alone.

Errors, permissions, and cancellation inside the transform remain visible in its callable contract.

---

**Previous:** [← Generators](./03-generators.md) · **Next:** [`filter` →](./05-filter.md)
