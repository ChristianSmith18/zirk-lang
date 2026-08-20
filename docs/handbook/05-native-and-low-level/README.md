# Native and Low-Level Programming

Zirk uses the C ABI as its stable native boundary. Raw pointers and
ABI-sensitive operations remain inside `unsafe`; reversible managed and bounded
native writes are transactional, while external or unbounded effects require
`commit`. Native dependencies still declare targets and permissions explicitly.
Portable intrinsics and SIMD provide controlled performance without textual
inline assembly.

---

**Previous:** [← Standard Library Indexes](../04-standard-library/19-standard-library-indexes.md) · **Next:** [ C ABI](01-c-abi.md)
