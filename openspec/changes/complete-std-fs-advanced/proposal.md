## Why

After both milestones in `deliver-std-fs-core` are complete, Zirk will have bounded synchronous file operations but still lack directory workflows and non-blocking filesystem waits. This follow-on completes the remaining filesystem surface in two coordinated blocks while preserving the same path-scoped consent, bounded resources, and structured cleanup.

## What Changes

- **Prerequisite:** complete, verify, document, and archive `deliver-std-fs-core` first. This change extends its `Path`, `File`, `FileError`, permission, and resource contracts; it does not redefine them.
- **Remaining block 1 — advanced synchronous filesystem:** add bounded directory creation/listing/walking/sizing/copy/move/removal; explicit symbolic-link inspection and creation; metadata/permission extensions; managed temporary files/directories; advisory file locks; and bounded directory watchers with overflow signaling.
- Extend path authorization only where these APIs require directory objects or explicit descendant-subtree operations. Prompts and permission inspection identify the operation and descendant extent. Core exact-file grants do not widen, and read, write, metadata, links, locks, and watches remain distinct effects.
- **Remaining block 2 — asynchronous filesystem:** add explicit async counterparts for blocking file/directory/lock operations and task-aware watcher waits. They suspend the caller, use the existing process-wide bounded managed-work admission, and integrate with structured cancellation without blocking the cooperative executor or creating an unbounded worker pool.
- Make platform guarantees explicit. Do not follow links during recursive traversal by default; do not promise complete watcher history, mandatory locks, or rollback for a non-interruptible operation. Unsupported guarantees return typed outcomes.
- Reject reachable native filesystem operations during browser preflight; do not substitute a browser-local or in-memory filesystem.
- Update standard-library docs, examples, support evidence, status/roadmap, then synchronize and review `../zirk-lang-site` from a committed source revision.

## Capabilities

### New Capabilities
- `zirk-filesystem-advanced`: Directory and link operations, temporary resources, metadata/permissions, locks, watchers, async filesystem waits, scoped authorization, cancellation, limits, and target behavior.

### Modified Capabilities
- `zirk-permissions`: Add explicit directory-object and descendant-subtree authorization for the operations that request it.
- `async-runtime-core`: Define bounded non-blocking admission, completion, shutdown, and cancellation for filesystem host work.
- `zirk-structured-concurrency`: Define ownership and cancellation cleanup for async filesystem work, locks, and watchers.

## Impact

- Depends on both milestones of `deliver-std-fs-core`, including its secure path backend and managed `File` resources.
- Extends the compiler/runtime permission manager, directory and link backends, task scheduler, managed blocking admission, and resource cleanup.
- Affects normative standard-library behavior, handbook/API pages, examples, implementation status, limitations, and roadmap; the companion `../zirk-lang-site` is synchronized only after source commit and reviewed separately.
- Does not add a new `.zkinit` permission name or grant authority through package declarations, directory names, links, locks, or watchers.
