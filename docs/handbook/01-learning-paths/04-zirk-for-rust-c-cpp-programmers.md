# Zirk for Rust, C, and C++ Programmers

Zirk can produce native libraries and applications, expose a C-compatible boundary, use raw pointers inside `unsafe`, and request explicit multicore work. Its safe everyday model, however, is intentionally higher level than C, C++, or Rust.

## Memory is automatic

Stack placement, heap allocation, escape analysis, moves, and reference counting are implementation strategies. Ownership and borrowing are not public Zirk 1.x semantics. Safe code is designed to exclude use-after-free, uncontrolled null dereference, and undefined behavior. Concurrency safety is enforced at implemented boundaries; it is not a general data-race guarantee for every shared value.

Do not translate lifetimes or smart pointers mechanically. Begin with values and safe references. Cross into `unsafe {}` only when a native or low-level contract cannot be expressed safely.

## Resource lifetime is distinct from memory lifetime

Files, sockets, and other external handles implement `Resource<E>`. `match with` acquires and closes a resource on ordinary exit, return, exception, error, and cooperative cancellation; grouped acquisition and explicit transfer are also supported. A non-cooperative operation can delay cancellation cleanup. Zirk 1.x deliberately avoids general `defer` and destructor-based cleanup as the primary public contract.

## Concurrency communicates intent

Use `concurrent { ... }`, `spawn`, and `Job<T>` for structured concurrent work; use `parallel` for CPU parallelism. Typed channels are available for communication. `Thread.run`, scalar `Atomic<T>`, and scoped `Mutex<T>.with` are delivered source APIs; the general `Thread` lifecycle API and other synchronizers remain unavailable. The former `task` syntax was removed. Do not assume a general compiler check for unsynchronized mutable global access.

## Native boundaries

The initial stable interoperability boundary is the C ABI. Native dependencies constrain target compatibility, and packages must declare the permissions and artifacts involved. Zirk provides portable intrinsics and SIMD rather than textual inline assembly.

## Recommended route

Read the object and value model before memory and unsafe code. Then study resources, concurrency, project targets, native interoperability, portable IR, ABI compatibility, and why package implementations are compiled for the final target.

---

**Previous:** [← Zirk for Python Programmers](03-zirk-for-python-programmers.md) · **Next:** [ Zirk for Java and C# Programmers](05-zirk-for-java-csharp-programmers.md)
