# Errors

Zirk has four deliberately separate failure channels: `Result<T,E>` for
expected operational failure, checked `throws` for explicit extraordinary
recovery, implicit typed `RuntimeError` for safe runtime checks, and
`fatalError` for irreparable state. There is no implicit conversion between
them. This unit defines handling, propagation, provenance and boundaries.

---

**Previous:** [← Interface, Trait, or Class?](../09-interfaces-and-traits/05-interface-vs-trait-vs-class.md) · **Next:** [ Result](01-result.md)
