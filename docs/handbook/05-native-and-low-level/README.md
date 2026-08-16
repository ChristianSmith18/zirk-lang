# Native and Low-Level Programming

Zirk uses the C ABI as its stable native boundary. Raw pointers and ABI-sensitive operations remain inside `unsafe`; native dependencies declare targets and permissions explicitly. Portable intrinsics and SIMD provide controlled performance without textual inline assembly.

---

**Previous:** [← std.system](../04-standard-library/17-std-system.md) · **Next:** [ C ABI](01-c-abi.md)
