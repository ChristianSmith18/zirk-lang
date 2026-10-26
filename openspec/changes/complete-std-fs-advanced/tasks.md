## 1. Confirm the core prerequisite and extend the public package

- [x] 1.1 Confirm `deliver-std-fs-core` is implemented, verified, documented, and archived; verify its `Path`, `File`, `FileError`, permission, and resource contracts before starting this change.
- [x] 1.2 Extend the official `std.fs` declarations without creating a second package identity; verify imports resolve through the same bundled package and existing core signatures remain unchanged.
- [x] 1.3 Define advanced event/value types and the target support matrix; verify every unavailable guarantee maps to a typed unsupported/degraded outcome rather than empty success.

## 2. Extend operation-local authorization for advanced scopes

- [x] 2.1 Extend approval records for directory-object and explicit descendant-subtree scopes, retaining project, requester, operation, and canonical identity; verify existing exact-file approvals do not widen.
- [x] 2.2 Update first-use consent and permission inspection to show operation and descendant extent; verify list consent does not authorize recursive reads, writes, deletion, links, or watchers.
- [x] 2.3 Check authorization before every directory, link, lock, watch, and async side effect; verify denial prevents the effect and authorization is repeated after suspension or path re-resolution.

## 3. Remaining block 1: bounded directory operations

- [x] 3.1 Implement single and recursive directory creation, lazy entries, bounded walk, emptiness, and bounded size; verify entry/depth/byte limits and typed absence, denial, and host errors.
- [x] 3.2 Implement bounded copy and same-filesystem move with explicit replacement behavior; verify source and destination scopes are independent, collisions preserve the destination, and cross-device renames map to a typed error.
- [x] 3.3 Implement single-entry removal and explicitly bounded recursive removal; verify destructive intent and bounds are required before any entry is removed.
- [x] 3.4 Prevent default link-following in walk, recursive size/copy/removal; verify links are surfaced or rejected as typed entries and their targets remain unchanged.

## 4. Remaining block 1: links, metadata, and temporary resources

- [x] 4.1 Implement supported symbolic-link inspection, target reading, and creation; verify target reading does not open the target and creation is denied outside destination write scope.
- [x] 4.2 Implement metadata snapshots and supported read-only permission changes; verify metadata access grants no content access and mutations require write scope.
- [x] 4.3 Implement exclusive unpredictable temporary file/directory creation beneath a validated parent; verify successive creations are unique and consent names the effective parent/scope.
- [x] 4.4 Implement temporary-resource cleanup by original native object identity and explicit ownership release; verify a substituted replacement is never deleted during cleanup.

## 5. Remaining block 1: locks, watchers, and synchronous conformance gate

- [x] 5.1 Implement shared/exclusive advisory locks, non-blocking try-lock, synchronous acquisition, and managed release on Unix; verify separate operation scopes, contention, and release. Async acquisition is delivered for supported Unix targets under task 6.2; Windows lock operations remain explicitly unsupported.
- [x] 5.2 Implement managed directory watchers with bounded event queues; verify create/modify/remove/rename/closed/error outcomes and documented coalescing behavior.
- [x] 5.3 Implement overflow reporting, rescan guidance, recursive scope containment, and watcher cleanup; verify forced overflow does not claim a complete history and links cannot reveal external events.
- [x] 5.4 Verify platform-specific lock and watcher guarantees; verify unsupported or weaker guarantees are represented in the published support matrix and typed outcomes.
- [x] 5.5 Gate async work on the synchronous block; verify directory/link/temp/lock/watch permission, race, cleanup, limit, and target checks pass before adding async entry points.

## 6. Remaining block 2: asynchronous filesystem and structured cancellation

- [x] 6.1 Extend the existing managed blocking admission path for owned filesystem jobs under the process-wide worker ceiling; verify it retains native/Zirk state safely and creates no second unbounded pool.
- [x] 6.2 Add explicit async counterparts for blocking file open/read/write, directory iteration, and lock acquisition, with watcher waits suspending directly; verify signatures and `Result` outcomes match synchronous behavior.
- [x] 6.3 Implement bounded FIFO admission and backpressure without blocking the executor; verify ready branches continue under slow I/O and saturation stays within worker and queue bounds.
- [x] 6.4 Implement cancellation before dispatch, during non-interruptible operations, async-open completion, lock races, watcher waits, and shutdown; verify no dangling roots/handles and no false rollback claim for non-atomic effects.
- [x] 6.5 Propagate async filesystem effects through direct calls, closures, higher-order APIs, and `Concurrent.each`; verify one branch's `Result.Error` does not cancel siblings.

## 7. Validate, document, and synchronize

- [x] 7.1 Add adversarial coverage for directory escape, link replacement, traversal cycles, temporary substitution, lock cleanup, watcher overflow, cancellation, and concurrent races; verify each invariant on native runtime tests.
- [x] 7.2 Run conformance on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64; verify each target meets published behavior or returns its documented typed unsupported/degraded outcome. macOS ARM64 and Linux ARM64 container runs pass; native Linux x86_64 run [40259785377](https://github.com/ChristianSmith18/zirk-lang/actions/runs/40259785377) passes 25 filesystem runtime tests and 20 CLI E2Es; focused native Windows run [40256201191](https://github.com/ChristianSmith18/zirk-lang/actions/runs/40256201191) passes 10 filesystem CLI E2Es and 12 `windows_` runtime tests. The full Windows workspace suite was skipped for this focused conformance run.
- [x] 7.3 Add browser capability preflight for all advanced filesystem effects; verify no host filesystem operation begins in browser mode.
- [x] 7.4 Update standard-library specs, handbook pages, API indexes, examples, status, limitations, and roadmap; verify each distinguishes delivered core, advanced synchronous APIs, and async behavior.
- [x] 7.5 Review and commit the complete `zirk-lang` implementation and documentation before synchronization; verify the source revision contains the approved scope and no unrelated changes.
- [x] 7.6 Run `./scripts/sync-website-content.sh` from the committed clean source state, review `../zirk-lang-site` and its status catalog with an explicit audit date, then verify and report both repository revisions.
