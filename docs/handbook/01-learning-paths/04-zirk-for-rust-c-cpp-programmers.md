# Zirk for Rust, C, and C++ Programmers

Zirk can produce native libraries and applications, expose a C-compatible boundary, use raw pointers inside `unsafe`, and request explicit multicore work. Its safe everyday model, however, is intentionally higher level than C, C++, or Rust.

## Memory is automatic

Stack placement, heap allocation, escape analysis, moves, and reference counting are implementation strategies. Ownership and borrowing are not public Zirk 1.x semantics. Safe code is nevertheless required to exclude use-after-free, null dereference, data races, and undefined behavior.

Do not translate lifetimes or smart pointers mechanically. Begin with values and safe references. Cross into `unsafe {}` only when a native or low-level contract cannot be expressed safely.

## Resource lifetime is distinct from memory lifetime

Files, sockets, and other external handles implement `Resource<E>`. `match with` acquires a resource and guarantees closure on every path, including return, exception, error, and cancellation. Zirk 1.x deliberately avoids general `defer` and destructor-based cleanup as the primary public contract.

## Concurrency communicates intent

Use `task` for structured concurrent work, `parallel` for CPU parallelism, and `thread` when OS-thread identity matters. Channels, `sync`, mutexes, and atomics coordinate shared state. The compiler must reject unsynchronized mutable global access from parallel or thread contexts.

## Native boundaries

The initial stable interoperability boundary is the C ABI. Native dependencies constrain target compatibility, and packages must declare the permissions and artifacts involved. Zirk provides portable intrinsics and SIMD rather than textual inline assembly.

## Recommended route

Read the object and value model before memory and unsafe code. Then study resources, concurrency, project targets, native interoperability, portable IR, ABI compatibility, and why package implementations are compiled for the final target.

---

**Previous:** [← Zirk for Python Programmers](./03-zirk-for-python-programmers.md) · **Next:** [Zirk for Java and C# Programmers →](./05-zirk-for-java-csharp-programmers.md)
