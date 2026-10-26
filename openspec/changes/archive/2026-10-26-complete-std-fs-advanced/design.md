## Context

See `proposal.md` for motivation and prerequisite sequencing. This change builds on the core `std.fs`/`std.path` host boundary from `deliver-std-fs-core`; it must not create another path resolver, permission vocabulary, error model, or resource ownership system. The runtime already has structured cooperative tasks and a bounded managed blocking pool, while the current networking APIs establish the expected contract that waits suspend Zirk branches rather than the executor thread.

## Goals / Non-Goals

**Goals:**
- Add everyday directory and temporary-resource operations with explicit bounds and conservative symlink defaults.
- Add scoped locks and watchers with explicit ownership, typed outcomes, and predictable cancellation cleanup.
- Provide async operations for blocking filesystem work without blocking the executor or multiplying unbounded worker threads.
- Extend path-scoped authorization to directory subtrees and advanced operations without granting ambient filesystem authority.
- Make native support and browser exclusion explicit and testable across all declared native targets.
- Sequence the remaining work in two blocks: complete and verify advanced synchronous filesystem APIs first, then add async filesystem integration and cancellation.

**Non-Goals:**
- Reimplement the core `File`, `Path`, `FileError`, or whole-file APIs from the prerequisite change.
- Claim portable guarantees stronger than the host filesystem provides for locks, change events, replacement, or durability.
- Provide a transaction across several files or a complete event history from native watchers.
- Expose raw OS handles or permit filesystem authority to cross package/requester boundaries.

## Decisions

### 1. Sequence this change after the core filesystem change

`deliver-std-fs-core` must be implemented, verified, documented, and archived before this proposal is applied. Its filesystem spec becomes the base contract extended by this capability; its `Path`, `Result`, error, resource, effect, permission, and runtime types remain canonical.

**Alternative considered:** Develop two branches concurrently against provisional signatures. Rejected because the advanced API would otherwise duplicate or repeatedly rewrite core types and authorization semantics.

### 2. Keep one public module surface and separate capability ownership in specifications

Advanced source declarations remain in the official `std.fs` package. The new advanced capability spec owns the additional behavioral guarantees while the standard-library index points to both filesystem contracts. Host binding identity remains fully qualified to the official source package.

**Alternative considered:** Add `std.directory`, `std.watch`, or a second `std.fs` package. Rejected because directory, lock, and watch operations share path, permissions, errors, and resource ownership with `File`.

### 3. Make destructive and recursive intent visible and bounded

Single-entry operations and recursive operations have distinct names. `remove_all` requires an explicit maximum-entry/depth bound. `walk`, recursive size/copy, and similar operations are lazy or incremental and enforce entry, depth, byte, and cancellation limits. Symbolic links are entries, not traversal instructions, by default. Any follow-links option is explicit and stays within a previously approved subtree.

**Alternative considered:** Follow links by default for familiar filesystem behavior. Rejected because it can leave the approved root, revisit cycles, and make resource limits ineffective.

### 4. Treat directory subtree approval as an explicit scope, not a prefix rule

A direct file operation is authorized against that file. A directory-list operation requests the concrete directory. Recursive operations must identify that descendants are involved before consent is collected; only explicit approval of that subtree scope covers descendant entries. The approval record binds operation, canonical directory, requester graph, and project identity. No path string prefix, symlink, package declaration, watcher, or lock can widen scope.

**Alternative considered:** Let a grant for `/project` automatically cover every descendant and operation. Rejected because list/read/write/delete/watch have different impact, and packages should not receive broader authority from a path spelling.

### 5. Tie temporary cleanup to native object identity

Temporary creation uses exclusive creation and unpredictable names in a controlled parent. Cleanup compares the still-open handle or native identity to the object originally created; it must not unlink a later replacement solely because it occupies the same path. Retaining a temporary file requires a visible ownership-release operation.

**Alternative considered:** Always remove by stored path on scope exit. Rejected because a hostile or concurrent process could replace that path and cause unrelated data deletion.

### 6. Treat locks as advisory coordination resources

Locks are scoped over a validated open file identity, not a string path alone. Shared locks require read authority; exclusive locks require write authority. Lock success grants no content access. `try_lock` is non-suspending; synchronous lock may block its caller; `lock_async` suspends its branch. Guarantees and unsupported filesystem behavior are documented per target.

**Alternative considered:** Expose mandatory locking or imply that a lock makes multi-call writes transactional. Rejected because most native file locks are advisory and do not protect against non-cooperating programs.

### 7. Treat watchers as bounded, lossy event streams

A watcher holds a managed OS registration and a bounded queue. Events may be coalesced, duplicated, or reordered where the platform cannot guarantee more. Overflow is explicit and requires a rescan. The API does not promise an audit log or exactly-once event delivery. Recursive watchers never follow symlinks by default and their event paths stay relative to or safely rooted in the approved directory.

**Alternative considered:** Promise a complete ordered filesystem history. Rejected because native watcher APIs cannot uphold that contract under queue overflow or platform coalescing.

### 8. Add explicit asynchronous counterparts using the existing managed blocking ceiling

Use clear `_async` methods for operations that may wait, such as open/read/write, directory iteration, and lock acquisition; watcher `next()` is inherently task-aware. They return the same typed outcomes as synchronous methods after suspending the current task; they do not return a new `Future` or add an `async fn` language feature. Native blocking syscalls run as owned runtime jobs using the existing process-wide managed worker admission, not on the cooperative executor. Queue saturation parks callers with bounded FIFO admission, and queued cancelled work is marked/removable before execution.

**Alternative considered:** Run synchronous filesystem calls on the executor. Rejected because a slow disk or network filesystem would stall every ready branch. **Alternative considered:** Create a filesystem-specific unbounded worker pool. Rejected because multiple host pools could exceed the runtime's worker ceiling and exhaust process resources.

### 9. Preserve roots and ownership through cancellation races

A worker job retains all path, byte, and native-handle state until completion. Cancellation before a job begins prevents the effect. A syscall already in progress may not be interruptible; the task remains in structured cleanup until its completion is safe. Any handle that cannot be delivered is closed by the runtime; lock races are resolved by cleanup ownership; watcher waits unregister before task termination. Non-atomic writes may have completed partially and cancellation must not claim rollback. Atomic replacement retains the core old-or-new publication guarantee.

**Alternative considered:** Immediately discard the caller frame on cancellation while a worker still holds pointers or handles. Rejected because that creates use-after-free and leaks or misreports effects.

### 10. Share admission capacity without starving unrelated managed work

Filesystem work uses the process-wide worker ceiling already used by managed blocking operations. Admission is bounded and FIFO; queued jobs retain their roots but not extra OS threads. The implementation must measure whether one class can monopolize the shared queue and provide fair admission between filesystem jobs and `Thread.run` before shipping. Queue capacity and worker ceilings remain runtime policy and are surfaced as bounded backpressure, not configurable ambient authority.

**Alternative considered:** Hard-code a second arbitrary filesystem pool/limit. Rejected until runtime-level measurements show a need; resource ceilings should be owned centrally.

### 11. Fail closed on unsupported hosts

Native operations retain equivalent observable results on supported native targets where the platform permits. Optional facilities return typed unsupported/degraded-guarantee errors. Browser preflight rejects filesystem, watcher, and lock effects before execution; no fake in-memory or browser-local backend is substituted.

**Alternative considered:** Degrade unsupported native features to success-shaped empty results. Rejected because empty results hide missing support and could trigger unsafe application decisions.

## Risks / Trade-offs

- [Subtree grants can be broader than an individual file grant] → The consent UI and `permissions inspect` must show operation and descendant extent explicitly; default file access remains exact and separate.
- [Native watchers can lose or coalesce events] → Bound queues, expose `Overflow`, document best-effort semantics, and test forced overflow/rescan.
- [Long-running native calls cannot always be cancelled] → Keep branch roots and handles alive until safe completion; never promise rollback after the syscall begins.
- [Filesystem operations can saturate the shared managed pool] → Bound admission, measure fairness against `Thread.run`, and report backpressure without adding threads.
- [Locks and atomic replacement differ on network or unusual filesystems] → Report unsupported guarantees and typed failures rather than claiming stronger semantics.
- [Platform-specific path and link races] → Use native handle-relative operations and test adversarial replacement/symlink cases on all declared targets.

## Migration Plan

This is additive and requires the archived core filesystem change. Existing code and core `std.fs` signatures remain valid. Advanced APIs become available through the same bundled module only after native target conformance. Directory subtree grants are separately stored and visible in permission inspection; prior exact-file grants do not widen. Async methods are additive; existing synchronous methods retain their blocking behavior. Browser builds receive explicit capability diagnostics for newly reachable APIs. User approvals remain outside the toolchain version directory and survive upgrade/rollback.

## Open Questions

- Platform conformance may identify individual watcher or lock guarantees that must be marked unsupported on a filesystem; the public outcome shape is already constrained to typed capability/unsupported outcomes, so only the support matrix remains to be finalized before release.
