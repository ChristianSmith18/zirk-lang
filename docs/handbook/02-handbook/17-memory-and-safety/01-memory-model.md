# Memory Model

The public model defines values, complete references, projected values, places,
identity, mutation, concurrency safety, and observable lifetime. Stack, heap,
regions, moves, reference counting, and garbage collection are internal
strategies that may be combined.

Representation cannot change equality, identity, or observable lifetime.
Objects retain identity even if the runtime moves their storage. `==` compares
structure; `is` observes stable reference identity.

Assigning a complete reference shares it. Reading an attribute, index, slice,
destructured component, iterator item, or projected capture creates an
independent logical value. Using the same expression as an assignment place
writes original storage. These are language rules, not GC side effects.

---

**Previous:** [← Memory and Safety](README.md) · **Next:** [ Stack and Heap](02-stack-and-heap.md)
