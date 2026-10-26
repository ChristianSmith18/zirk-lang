## Purpose

Defines the advanced filesystem services built on Zirk's core `std.fs` and `std.path` APIs, including bounded directory operations, links, temporary resources, advisory coordination, change notifications, and asynchronous host operations.

## ADDED Requirements

### Requirement: Advanced filesystem APIs extend the delivered core package

This capability SHALL be implemented only after `deliver-std-fs-core` has been applied and archived. It SHALL extend the existing official `std.fs` and `std.path` packages and reuse their `Path`, `FileError`, `Result`, managed-resource, path-resolution, and operation-local permission contracts. It SHALL NOT add a second filesystem module identity, duplicate error model, or separate permission vocabulary.

#### Scenario: Advanced operation uses the core package identity
- **WHEN** a project imports a directory or watcher API from `std.fs`
- **THEN** it resolves through the same bundled package root as `File` and `Path`
- **AND** only official standard-library declarations receive compiler-owned host bindings

#### Scenario: Core prerequisite is absent
- **WHEN** this change is considered for implementation before `deliver-std-fs-core` is archived
- **THEN** implementation is blocked until the core types and permission boundary are available

### Requirement: Directory operations are bounded and distinguish absence from denial

`std.fs` SHALL provide typed operations for creating one directory, creating parent directories, listing entries lazily, bounded recursive walking, checking emptiness, obtaining bounded recursive size, copying, moving, removing one entry, and explicitly destructive recursive removal. Operations SHALL accept `Path`, return typed `Result` outcomes, and enforce documented entry-count, depth, byte, and cancellation limits. Absence SHALL be represented separately from permission denial and other host errors. `remove_all` SHALL be visibly named and SHALL require an explicit caller limit or equivalent safety bound.

#### Scenario: Create nested directories
- **WHEN** a program calls `Directory.create_all(path)` within its approved write scope
- **THEN** missing parent directories are created and the result identifies whether the operation succeeded or failed

#### Scenario: List a directory lazily
- **WHEN** a program iterates `Directory.entries(path)` inside its approved read scope
- **THEN** entries are produced incrementally without materializing an unbounded directory listing
- **AND** iteration reports a typed error if enumeration fails

#### Scenario: Bound recursive walking
- **WHEN** a walk reaches its configured depth or entry ceiling
- **THEN** it returns a typed limit outcome and does not continue allocating or traversing beyond the ceiling

#### Scenario: Recursive removal requires explicit bounded intent
- **WHEN** a caller requests recursive removal without the required explicit bound or destructive operation form
- **THEN** the operation is rejected before removing any entry

#### Scenario: Distinguish inaccessible from absent
- **WHEN** an entry cannot be inspected because its directory scope is denied
- **THEN** the operation returns permission denial rather than reporting the entry as absent

### Requirement: Directory traversal does not follow symbolic links by default

Directory walking, recursive size, recursive copy, and recursive removal SHALL treat symbolic links as entries and SHALL NOT traverse their targets by default. An operation that explicitly follows links SHALL enforce depth and cycle limits, remain within the approved effective scope, and revalidate every resolved target. Recursive removal SHALL remove a link itself rather than its target unless an explicit separately authorized operation says otherwise.

#### Scenario: Walk encounters a symbolic link
- **WHEN** a default recursive walk encounters a symbolic link to another directory
- **THEN** it reports the link entry and does not enumerate the linked directory

#### Scenario: Explicit link following escapes scope
- **WHEN** a caller explicitly follows a symbolic link whose target resolves outside the approved directory subtree
- **THEN** traversal stops with a typed permission error before accessing the target

#### Scenario: Recursive removal encounters a link
- **WHEN** bounded recursive removal encounters a symbolic link
- **THEN** it removes only the link entry and leaves its target untouched

### Requirement: Symbolic-link operations preserve target boundaries

The API SHALL provide explicit, typed link inspection, read-target, and link creation operations where supported. Link targets SHALL be represented as `Path` values without implicit traversal. Creating or replacing a link SHALL require write consent for the destination and the exact target scope needed by the operation. Canonicalization and link inspection SHALL use `filesystem.metadata` consent or full `filesystem.read` consent for the same exact path. Unsupported link kinds or target behavior SHALL return an explicit typed platform error.

#### Scenario: Read a link without opening its target
- **WHEN** a program reads the target of an authorized symbolic link
- **THEN** it receives the stored target path without implicitly reading the target contents

#### Scenario: Create a link outside destination authority
- **WHEN** a program attempts to create a link at a destination outside its write scope
- **THEN** the operation is denied before creating the link

#### Scenario: Link kind is unsupported
- **WHEN** a requested link kind is unavailable on the target platform
- **THEN** the API returns an explicit unsupported-operation error rather than silently creating a different link kind

### Requirement: Filesystem metadata and permissions are explicit snapshots and mutations

Filesystem metadata SHALL expose a documented immutable snapshot of file type, size, timestamps, and supported attributes. Reading metadata SHALL require `filesystem.metadata` scope for the target; full `filesystem.read` scope SHALL also satisfy that check. Metadata-only consent SHALL NOT grant content access. Reading permissions SHALL be distinct from changing them; permission changes SHALL require write scope and SHALL report unsupported or partially supported platform fields explicitly. A metadata snapshot SHALL NOT be represented as a live authorization token.

#### Scenario: Inspect metadata without reading contents
- **WHEN** a program has `filesystem.metadata` or full `filesystem.read` authority but no content-read grant
- **THEN** metadata succeeds while a separate content read is still denied

#### Scenario: Change filesystem permissions
- **WHEN** a caller changes supported mode attributes within its approved write scope
- **THEN** the result reports the fields changed and any unsupported requested field

#### Scenario: Use stale metadata after path replacement
- **WHEN** a path is replaced after metadata was read and the caller later mutates it
- **THEN** the operation revalidates current identity and authority instead of trusting the old snapshot

### Requirement: Temporary files and directories have managed cleanup

The API SHALL provide unpredictable, collision-resistant temporary files and directories under an explicit or documented system temporary parent. Creation SHALL use atomic exclusive creation and require an operation-local write grant for the effective parent/scope. A returned temporary handle SHALL be a non-copyable managed resource with explicit path access and close/removal behavior. Cleanup SHALL be idempotent, shall not follow substituted symlinks, and SHALL report cleanup failure without deleting an unrelated replacement. Persistent temporary paths SHALL require an explicit ownership-release operation.

#### Scenario: Create and automatically clean a temporary file
- **WHEN** a temporary file resource leaves its managed `match with` scope
- **THEN** the owned file is removed exactly once if the resource still identifies the originally created object

#### Scenario: Preserve a substituted entry during cleanup
- **WHEN** another process replaces the temporary path before cleanup
- **THEN** cleanup does not follow the replacement or delete an unrelated object and returns a typed cleanup failure

#### Scenario: Retain a temporary path explicitly
- **WHEN** a caller explicitly releases temporary ownership
- **THEN** the file remains at its path and the caller becomes responsible for its later cleanup

### Requirement: Advisory file locks are managed and operation-scoped

`File.lock` SHALL provide shared and exclusive advisory locks over a validated file identity; `try_lock` SHALL return without waiting, and asynchronous lock acquisition SHALL suspend the caller. A read/shared lock SHALL require read scope and an exclusive lock SHALL require write scope. Locks SHALL be resources, SHALL release exactly once on close or managed-scope exit, and SHALL NOT imply consent to read or write file contents. The contract SHALL state that non-cooperating processes may ignore advisory locks and that lock guarantees vary by filesystem.

#### Scenario: Acquire a shared lock
- **WHEN** a caller acquires a shared lock with read scope
- **THEN** it receives a managed lock resource without gaining write authority

#### Scenario: Exclusive lock is denied without write scope
- **WHEN** a caller has read consent only and attempts an exclusive lock
- **THEN** acquisition is denied before changing lock state

#### Scenario: Lock cleanup on early exit
- **WHEN** control leaves the managed lock scope by return, exception, or cancellation
- **THEN** the lock is released before the scope completes

#### Scenario: Non-blocking acquisition finds a held lock
- **WHEN** `try_lock` finds an incompatible cooperating lock
- **THEN** it returns a typed unavailable outcome immediately without suspending

### Requirement: Filesystem watchers are bounded streams with explicit overflow

`Directory.watch(path, options)` SHALL return a managed watcher resource that observes only the explicitly authorized directory scope. Events SHALL identify create, modify, remove, rename, close, overflow, and typed host error outcomes. The event queue SHALL be bounded; duplicate events MAY be coalesced, and an overflow SHALL require a rescan rather than claim complete history. Recursive watching SHALL remain within the approved subtree and enforce a documented watch/resource ceiling. Watchers SHALL close on explicit close, managed-scope exit, cancellation, or application shutdown.

#### Scenario: Observe a change in the authorized directory
- **WHEN** an authorized watcher observes a file modification
- **THEN** it emits a modification event carrying a path relative to or safely rooted in the watched scope

#### Scenario: Event queue overflows
- **WHEN** the watcher cannot retain every host event within its configured bound
- **THEN** it emits `Overflow`, stops claiming a complete change history, and allows the caller to rescan

#### Scenario: Recursive watcher sees an external link target
- **WHEN** a watched tree contains a link to a path outside its approved subtree
- **THEN** the watcher does not follow the link or disclose events from the target

#### Scenario: Watcher is cancelled
- **WHEN** its owning task or scope is cancelled while waiting for the next event
- **THEN** the wait resumes with a typed cancellation outcome and watcher resources are closed before scope completion

### Requirement: Asynchronous filesystem operations suspend cooperatively

Operations that may block SHALL expose explicit asynchronous counterparts or documented task-aware methods. Waiting SHALL suspend the calling Zirk branch and SHALL NOT block the cooperative executor thread. Native work SHALL use bounded managed blocking admission shared with the runtime's existing blocking-work ceiling; worker and queue saturation SHALL produce a typed capacity outcome or documented backpressure without unbounded thread or queue creation. Permission checks SHALL occur at the host side-effect boundary for every asynchronous retry or newly selected path. The API SHALL preserve the same `Result`, size, resource, and target semantics as its synchronous counterpart.

#### Scenario: Async operation leaves the executor available
- **WHEN** an asynchronous filesystem operation waits on a slow native filesystem
- **THEN** the caller's branch is suspended and another ready branch continues to run
- **AND** no blocking filesystem call occupies the executor thread

#### Scenario: Managed blocking admission is exhausted
- **WHEN** the bounded filesystem work admission is full
- **THEN** the call applies the documented bounded backpressure or returns a typed capacity error without spawning an unbounded worker

#### Scenario: Asynchronous retry resolves to a new path
- **WHEN** an async operation resumes or retries against a newly resolved effective path
- **THEN** it rechecks the applicable consent and scope before performing the next side effect

### Requirement: Asynchronous filesystem cancellation preserves ownership and side-effect truth

Cancellation before a queued native operation begins SHALL remove or mark the operation so it performs no filesystem effect. Once a non-interruptible host operation has begun, cancellation SHALL NOT claim that the operation was rolled back; the runtime SHALL keep its arguments and handles alive until the worker reaches a safe completion point, close any resource that cannot be delivered to the cancelled caller, and complete structured cleanup before reporting the branch terminal. Operations with atomic replacement semantics SHALL retain their atomic publication guarantee under cancellation. Watcher waits and async lock waits SHALL be cancellable without leaving registrations or acquired locks behind.

#### Scenario: Cancel before worker execution
- **WHEN** a task is cancelled while its filesystem operation remains queued and has not begun
- **THEN** the queued operation performs no host side effect and releases its admission and roots

#### Scenario: Cancel during a non-interruptible write
- **WHEN** cancellation arrives after the host has begun a write that cannot be interrupted
- **THEN** the runtime keeps all inputs alive until the write reaches a safe point and reports no false rollback guarantee

#### Scenario: Async open completes after caller cancellation
- **WHEN** an open succeeds after its waiting task has been cancelled
- **THEN** the runtime closes the undeliverable handle before releasing the task's retained state

#### Scenario: Cancelled lock waiter loses the acquisition race
- **WHEN** a lock waiter is cancelled while another owner releases the lock
- **THEN** it either acquires and immediately releases under cleanup ownership or is removed without retaining the lock

### Requirement: Advanced filesystem target support is explicit

Directory, link, temporary, lock, watcher, and asynchronous APIs SHALL declare their support on macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64. Platform-specific behavior MAY differ only where documented through typed capability or unsupported-operation outcomes. Browser execution SHALL reject native filesystem services during preflight and SHALL NOT substitute an ambient or fake filesystem.

#### Scenario: Run advanced filesystem conformance on each native target
- **WHEN** target conformance runs on a declared native target
- **THEN** it verifies scoped directory operations, symlink containment, temporary cleanup, lock release, watcher overflow/cleanup, async executor progress, and cancellation behavior

#### Scenario: Request an unsupported native feature
- **WHEN** the operating system or filesystem cannot provide a requested optional guarantee
- **THEN** the API returns a typed unsupported or degraded-guarantee outcome identified by the documentation

#### Scenario: Reach an advanced filesystem API in the browser
- **WHEN** browser-targeted code reaches a directory, link, lock, watcher, or filesystem operation
- **THEN** capability preflight reports the unsupported host effect before execution
