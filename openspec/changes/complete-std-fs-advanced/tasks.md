## 1. Confirm the core prerequisite and extend the public package

- [ ] 1.1 Confirm `deliver-std-fs-core` is implemented, verified, documented, and archived; verify its `Path`, `File`, `FileError`, permission, and resource contracts before starting this change.
- [ ] 1.2 Extend the official `std.fs` declarations without creating a second package identity; verify imports resolve through the same bundled package and existing core signatures remain unchanged.
- [ ] 1.3 Define advanced event/value types and the target support matrix; verify every unavailable guarantee maps to a typed unsupported/degraded outcome rather than empty success.

## 2. Extend operation-local authorization for advanced scopes

- [ ] 2.1 Extend approval records for directory-object and explicit descendant-subtree scopes, retaining project, requester, operation, and canonical identity; verify existing exact-file approvals do not widen.
- [ ] 2.2 Update first-use consent and permission inspection to show operation and descendant extent; verify list consent does not authorize recursive reads, writes, deletion, links, or watchers.
- [ ] 2.3 Check authorization before every directory, link, lock, watch, and async side effect; verify denial prevents the effect and authorization is repeated after suspension or path re-resolution.

## 3. Remaining block 1: bounded directory operations

- [ ] 3.1 Implement single and recursive directory creation, lazy entries, bounded walk, emptiness, and bounded size; verify entry/depth/byte limits and typed absence, denial, and host errors.
- [ ] 3.2 Implement copy and move with explicit replacement behavior and same-filesystem outcomes; verify source read and destination write scopes are checked independently.
- [ ] 3.3 Implement single-entry removal and explicitly bounded recursive removal; verify destructive intent and bounds are required before any entry is removed.
- [ ] 3.4 Prevent default link-following in walk, recursive size/copy/removal; verify links are surfaced as entries and their targets remain unchanged.

## 4. Remaining block 1: links, metadata, and temporary resources

- [ ] 4.1 Implement supported symbolic-link inspection, target reading, and creation; verify target reading does not open the target and creation is denied outside destination write scope.
- [ ] 4.2 Implement metadata extensions and supported permission reads/changes; verify metadata access grants no content access or mutation and unsupported fields are reported explicitly.
- [ ] 4.3 Implement exclusive unpredictable temporary file/directory creation beneath a validated parent; verify concurrent creation never reuses an existing entry and consent names the effective parent/scope.
- [ ] 4.4 Implement temporary-resource cleanup by original native object identity and explicit ownership release; verify replacement entries and symlink substitutions are never deleted during cleanup.

## 5. Remaining block 1: locks, watchers, and synchronous conformance gate

- [ ] 5.1 Implement shared/exclusive advisory locks, non-blocking try-lock, synchronous acquisition, and managed release; verify separate read/write authority and exactly-once cleanup.
- [ ] 5.2 Implement managed directory watchers with bounded event queues; verify create/modify/remove/rename/closed/error outcomes and documented coalescing behavior.
- [ ] 5.3 Implement overflow reporting, rescan guidance, recursive scope containment, and watcher cleanup; verify forced overflow does not claim a complete history and links cannot reveal external events.
- [ ] 5.4 Verify platform-specific lock and watcher guarantees; verify unsupported or weaker guarantees are represented in the published support matrix and typed outcomes.
- [ ] 5.5 Gate async work on the synchronous block; verify directory/link/temp/lock/watch permission, race, cleanup, limit, and target checks pass before adding async entry points.

## 6. Remaining block 2: asynchronous filesystem and structured cancellation

- [ ] 6.1 Extend the existing managed blocking admission path for owned filesystem jobs under the process-wide worker ceiling; verify it retains native/Zirk state safely and creates no second unbounded pool.
- [ ] 6.2 Add explicit async counterparts for blocking file open/read/write, directory iteration, and lock acquisition, with watcher waits suspending directly; verify signatures and `Result` outcomes match synchronous behavior.
- [ ] 6.3 Implement bounded FIFO admission and backpressure without blocking the executor; verify ready branches continue under slow I/O and saturation stays within worker and queue bounds.
- [ ] 6.4 Implement cancellation before dispatch, during non-interruptible operations, async-open completion, lock races, watcher waits, and shutdown; verify no dangling roots/handles and no false rollback claim for non-atomic effects.
- [ ] 6.5 Propagate async filesystem effects through direct calls, closures, higher-order APIs, and `Concurrent.each`; verify one branch's `Result.Error` does not cancel siblings.

## 7. Validate, document, and synchronize

- [ ] 7.1 Add adversarial coverage for directory escape, link replacement, traversal cycles, temporary substitution, lock cleanup, watcher overflow, cancellation, and concurrent races; verify each invariant on native runtime tests.
- [ ] 7.2 Run conformance on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64; verify each target meets published behavior or returns its documented typed unsupported/degraded outcome.
- [ ] 7.3 Add browser capability preflight for all advanced filesystem effects; verify no host filesystem operation begins in browser mode.
- [ ] 7.4 Update standard-library specs, handbook pages, API indexes, examples, status, limitations, and roadmap; verify each distinguishes delivered core, advanced synchronous APIs, and async behavior.
- [ ] 7.5 Review and commit the complete `zirk-lang` implementation and documentation before synchronization; verify the source revision contains the approved scope and no unrelated changes.
- [ ] 7.6 Run `./scripts/sync-website-content.sh` from the committed clean source state, review `../zirk-lang-site` and its status catalog with an explicit audit date, then verify and report both repository revisions.
