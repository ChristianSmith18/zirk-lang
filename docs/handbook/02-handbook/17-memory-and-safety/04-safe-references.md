# Safe References

Safe references are non-null and cannot outlive the value or resource on which they depend. The compiler tracks escape and alias constraints internally without exposing Rust-style lifetime syntax in Zirk 1.x.

A reference derived inside `match with` cannot escape after the resource closes. Sharing mutable referenced state across concurrent contexts requires synchronization.

---

**Previous:** [← Automatic Memory Management](./03-automatic-memory-management.md) · **Next:** [Pointers →](./05-pointers.md)
