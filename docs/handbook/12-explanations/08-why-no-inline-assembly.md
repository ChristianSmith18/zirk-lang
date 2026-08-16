# Why No Inline Assembly?

Textual assembly ties source to an assembler, target, registers, ABI, and optimizer contract. Typed intrinsics and portable SIMD permit validation and fallback. Truly unavoidable assembly belongs in an audited native library behind the C ABI.

---

**Previous:** [← Why No General defer?](07-why-no-defer.md) · **Next:** [ Why the C ABI?](09-why-c-abi.md)
