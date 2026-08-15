# C ABI

The C ABI is Zirk 1.x's stable interoperability surface because operating systems and libraries expose it broadly. A binding must specify calling convention, symbol, integer widths, pointer validity, layout, ownership, and error behavior. C++ and Rust APIs require an `extern "C"` wrapper rather than compiler-specific ABI assumptions.

---

**Previous:** [← Native and Low-Level Programming](./README.md) · **Next:** [Importing C →](./02-importing-c.md)
