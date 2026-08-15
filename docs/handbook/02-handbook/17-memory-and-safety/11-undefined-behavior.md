# Undefined Behavior

Safe Zirk has no undefined behavior. Bounds, division by zero, null, invalid state, and overflow produce compile-time diagnostics or controlled runtime failures.

`unsafe` moves responsibility for specific invariants to the programmer, but does not grant a general license to violate the language. Safe APIs built above unsafe code must restore all public guarantees.

Zirk 1.x exposes neither ownership nor reference counting as source semantics; internal strategies cannot weaken this rule.

---

**Previous:** [← Use-After-Free Prevention](./10-use-after-free.md) · **Next:** Concurrency *(next unit)*
