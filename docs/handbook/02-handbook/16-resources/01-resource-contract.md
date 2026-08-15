# The Resource Contract

`Resource<E>` describes acquisition, use, and typed closure failure for an external resource. Opening can fail before a resource exists; closing can fail after useful work has completed.

The contract guarantees exactly-once closure when managed by `match with`. A garbage collector or object finalizer cannot provide the same deterministic external effect.

---

**Previous:** [← Resources](./README.md) · **Next:** [`match with` →](./02-match-with.md)
