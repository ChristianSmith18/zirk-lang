## REMOVED Requirements

### Requirement: Lowering of task creation and awaiting
**Reason**: `task` / `await` are removed.
**Migration**: `BranchStart` / `JobWait` / `ScopeEnter` / `ScopeExit` — `concurrent-blocks-and-timers`.

### Requirement: Lowering of structured scopes with cleanup edges
**Reason**: `task scope` is removed. The cleanup-edge mechanism itself (used by `finally`, resource close, unsafe-journal rollback) is unchanged and is reused for `concurrent { }` scope exit in `concurrent-blocks-and-timers`.
**Migration**: `ScopeEnter` / `ScopeExit` on cleanup edges — `concurrent-blocks-and-timers`.

### Requirement: Lowering of cancellation, shields, and timeouts
**Reason**: `await ... timeout` and `cancellation shield` are removed.
**Migration**: safe-point cancellation checks — `concurrent-blocks-and-timers`; `Concurrent.protect` shield-depth edges and `.within(d)` — `concurrency-completion`.

### Requirement: Lowering of select
**Reason**: `select` is removed.
**Migration**: `Concurrent.of(...).first()` lowering — `concurrency-completion`.
