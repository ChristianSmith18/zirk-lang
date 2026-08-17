# Automatic Memory Management

The runtime reclaims unreachable memory, including cycles and concurrent structures, without observable general-purpose destructors. Pause time, throughput, and memory consumption are implementation metrics rather than new semantics.

Memory reclamation does not close files, sockets, locks, or processes. Those external resources use `Resource<E>` and deterministic scopes.

The implementation may combine tracing collection, generations, regions,
escape analysis, moves, or reference counting. It must reclaim cycles and may
not expose collection timing as a correctness mechanism. `Resource<E>` and
`match with` are the only general deterministic-cleanup model.

---

**Previous:** [← Stack and Heap](02-stack-and-heap.md) · **Next:** [ Safe References](04-safe-references.md)
