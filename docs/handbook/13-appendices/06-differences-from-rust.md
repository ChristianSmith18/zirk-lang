# Differences from Rust

Zirk safe code shares goals such as preventing use-after-free and data races, but Zirk 1.x does not expose ownership, borrowing, or lifetime syntax. Memory strategy is automatic and may combine several techniques.

Resources use `match with`; concurrency distinguishes tasks, parallel CPU work, and OS threads; native interoperability uses C ABI wrappers.

---

**Previous:** [← Differences from Python](./05-differences-from-python.md) · **Next:** [Current Limitations →](./07-current-limitations.md)
