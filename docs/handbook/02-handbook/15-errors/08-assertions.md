# Assertions

Assertions verify programmer assumptions, especially in tests. They are not input validation and must not be the only protection for a runtime safety contract.

An assertion failure reports the expression, source location, and useful actual values without leaking secrets. Release optimization cannot turn a required bounds, null, permission, or type check into undefined behavior.

Use `Result` or a typed exception when callers can recover.

---

**Previous:** [← `fatalError`](./07-fatal-error.md) · **Next:** [Resources →](../16-resources/README.md)
