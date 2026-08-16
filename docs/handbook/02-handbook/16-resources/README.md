# Resources

Memory lifetime and external-resource lifetime are different. Files, sockets, and handles implement `Resource<E>` and use `match with` so closure occurs exactly once across success, error, exception, return, and cancellation.

---

**Previous:** [← Assertions](../15-errors/08-assertions.md) · **Next:** [ The Resource Contract](01-resource-contract.md)
