# Resources

Memory lifetime and external-resource lifetime are different. Files, sockets, and handles implement `Resource<E>` and use `match with` for deterministic closure across ordinary exit, error, exception, return, and cooperative cancellation. Grouped acquisition, transfer, use-after-transfer checking, and body/close failure composition are delivered; cancellation can still wait on non-cooperative work.

Resource responsibility is compiler-tracked without public ownership syntax.
Transfer, duplication, dependent lifetime, container extraction, and combined
body/close failures are explicit so automatic memory reclamation can never be
mistaken for external cleanup.

---

**Previous:** [← Assertions](../15-errors/08-assertions.md) · **Next:** [ The Resource Contract](01-resource-contract.md)
