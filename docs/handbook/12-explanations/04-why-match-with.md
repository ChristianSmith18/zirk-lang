# Why `match with`?

External resources have acquisition errors, scoped use, exactly-once close, possible close errors, and cancellation behavior. `match with` gives that lifecycle one compiler-verifiable construct and prevents direct or indirect escape.

It also defines grouped acquisition order, reverse unwind, explicit transfer,
non-clonability and lossless body/close failure composition. Automatic memory
management cannot replace this observable external lifecycle.

Garbage collection cannot guarantee timely close; `finally` does not itself model typed acquisition and transfer. This is why resources receive a dedicated contract.

---

**Previous:** [← Why Result and Exceptions?](03-why-result-and-exceptions.md) · **Next:** [ Why No new?](05-why-no-new.md)
