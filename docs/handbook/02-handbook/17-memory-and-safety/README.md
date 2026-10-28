# Memory and Safety

Zirk manages memory automatically. Implemented safe-language paths aim to
prevent use-after-free, double-free, uncontrolled null dereference, and
undefined behavior; this is not a blanket guarantee for unsafe or native code.
The guarantees implemented today are scoped to the language and
compiler boundaries that are available; they do not amount to a general
data-race guarantee. For example, `parallel` rejects mutable reference captures,
but the compiler does not prove race freedom for every shared value or global.
Managed `Thread.run` and scoped `Mutex<T>.with` are delivered. General
`Thread<T>` handles and language-level rwlock, semaphore, barrier, and once
APIs remain unavailable. The compiler provides scalar `Atomic<T>` for
booleans and signed/unsigned integers through 64 bits when the target supports
the width. Its payload is inline in the GC-managed object; pointer atomics are
unsupported, and no lock-free promise is made. Atomic operations do not amount
to a general data-race proof.

Transfer/Share properties are derived for represented concrete type graphs
and checked at implemented boundaries such as `spawn`, channels, and `parallel`.
Unknown/open or opaque native types, function values without capture
provenance, task handles, pointers, weak references, and native views fail the
proof conservatively. This is not a general race-freedom proof or a guarantee
about arbitrary foreign code.

The collector traces compiler-emitted shadow-stack roots for live executor
tasks, including suspended tasks, retains task results until consumed, and
traces managed references in queued channel values from their generated layout.
It does not scan arbitrary native stacks or unregistered foreign roots. The
private blocking pool accepts Rust closures only and is not connected to the
managed heap's mutator registration.

Cancellation is cooperative: it is delivered at runtime/compiler checkpoints,
not by preempting arbitrary instructions. A tight loop or synchronous native
call can delay it. Cleanup is attempted under a cancellation shield, but
`Job.cancel()` only requests cancellation. Executor shutdown cancels tracked
background work and waits for a grace period; expiry reports unfinished jobs
without forcibly stopping them, and active `parallel` regions must still finish
before the managed heap is released. Effect summaries propagate selected
suspend/block/spawn effects through checked direct calls; currently known
language operations seed suspension and spawning, while unresolved callable
and `extern` calls are conservatively treated as possibly suspending, blocking,
and spawning. They guard `parallel` and reject effectful cleanup for the
`Concurrent.protect` cleanup, which runs under a nesting-safe cancellation
shield and checks pending cancellation after cleanup returns. They do not
classify all I/O or prove general nonblocking behavior; cleanup must terminate.

This unit separates ordinary managed references, dependent native views,
reversible unsafe mutation, and irreversible native effects.

Read it in order. The full normative contract lives in
[Memory and Unsafe Semantics](../../../MEMORY_AND_UNSAFE_SEMANTICS.md).

---

**Previous:** [← Why No General defer or Destructors?](../16-resources/06-why-no-defer-or-destructors.md) · **Next:** [ Memory Model](01-memory-model.md)
