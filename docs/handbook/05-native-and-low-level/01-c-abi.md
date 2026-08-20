# C ABI

The C ABI is Zirk 1.x's stable interoperability surface because operating systems and libraries expose it broadly. A binding must specify calling convention, symbol, integer widths, pointer validity, layout, ownership, and error behavior. C++ and Rust APIs require an `extern "C"` wrapper rather than compiler-specific ABI assumptions.

## What crosses the boundary

Use fixed-width integers, explicitly represented floats, C-compatible records,
opaque handles, pointers with documented extent, and callback function pointers.
Zirk `String`, classes, algebraic enums, generics, exceptions, tasks, and private
object layouts do not cross directly. A wrapper converts them to bytes,
tag/value records, handles, status codes, or another declared C shape.

Every declaration states symbol name, calling convention, nullability,
alignment, buffer length relationship, ownership/borrowing, thread safety,
callback lifetime, and error convention. Platform-dependent C types such as
`long` are translated according to the selected target rather than assumed to
have one width.

## Safety boundary

Calling an imported function is unsafe unless a generated or handwritten safe
wrapper proves all preconditions. The wrapper validates inputs, bounds native
views, pins managed storage only for the call's documented lifetime, translates
status/`errno` into `Result`, and turns owned handles into `Resource` types.
Unknown external effects belong in `commit` and still require their ordinary
permissions.

ABI conformance tests compile a small C companion for every supported target
and compare size, alignment, offsets, calling convention, symbols, ownership,
and failure behavior.

---

**Previous:** [← Native and Low-Level Programming](README.md) · **Next:** [ Importing C](02-importing-c.md)
