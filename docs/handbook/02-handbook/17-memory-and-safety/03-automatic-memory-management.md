# Automatic Memory Management

The runtime reclaims unreachable memory, including cycles and concurrent structures, without observable general-purpose destructors. Pause time, throughput, and memory consumption are implementation metrics rather than new semantics.

Memory reclamation does not close files, sockets, locks, or processes. Those external resources use `Resource<E>` and deterministic scopes.

---

**Previous:** [← Stack and Heap](./02-stack-and-heap.md) · **Next:** [Safe References →](./04-safe-references.md)
