# Safe References

Safe references are non-null and cannot outlive the value or resource on which they depend. The compiler tracks escape and alias constraints internally without exposing Rust-style lifetime syntax in Zirk 1.x.

A reference derived inside `match with` cannot escape after the resource closes. Sharing mutable referenced state across concurrent contexts requires synchronization.

`Weak<T>` observes without keeping the referent alive:

```zirk
mut weak = Weak.from(cache);

match weak.upgrade() {
    Some(live) => live.refresh(),
    None => rebuild_cache(),
}
```

`upgrade(): Option<T>` is required before use. `is_alive` is only an immediate
observation and cannot replace upgrading. Native views, borrowed iterators,
resource-derived handles, and internal-storage views are dependent references;
the compiler rejects any escape beyond their owner.

Validated native borrowing pins managed storage automatically for the bounded
borrow. Zirk 1.x does not expose a general `Pin<T>` or lifetime annotation.

---

**Previous:** [← Automatic Memory Management](03-automatic-memory-management.md) · **Next:** [ Pointers](05-pointers.md)
