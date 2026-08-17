# Undefined Behavior

Safe Zirk has no undefined behavior. Bounds, division by zero, null, invalid state, and overflow produce compile-time diagnostics or controlled runtime failures.

`unsafe` moves responsibility for specific invariants to the programmer, but does not grant a general license to violate the language. Safe APIs built above unsafe code must restore all public guarantees.

Transactional rollback covers controlled `Error`, exceptions, checked traps,
and pre-commit cancellation. It cannot restore a process after arbitrary memory
corruption, an invalid instruction, damage inside unknown native code, or abrupt
termination. Development sanitizers should turn detectable precondition
violations into early controlled traps, but program correctness cannot depend on
detecting true undefined behavior afterward.

Zirk 1.x exposes neither ownership nor reference counting as source semantics; internal strategies cannot weaken this rule.

---

**Previous:** [← Use-After-Free Prevention](10-use-after-free.md) · **Next:** [ Transactional Unsafe and Commit](12-transactional-unsafe-and-commit.md)
