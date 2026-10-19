# Debugger

> **Implementation status:** the debugger is not implemented yet. This chapter records
> its target behavior; it is not a list of currently available commands or runtime
> capabilities. See the [feature status](../11-reference/12-feature-status.md) and
> [compiler status](../../../init/ZIRK_FEATURE_STATUS.md) for delivered tooling.

When implemented, debug metadata should map native execution to `.zrk`, including
tasks, awaits, threads, channels, generated wrappers, decorator applications and
expansion paths. Decorator objects are not present at runtime; source maps should
connect generated behavior to its original application. Release optimization may
limit visibility explicitly.

The target debugger should support source breakpoints, stepping, call stacks,
variables, threads, structured tasks, channels, and exception stops. A suspended
task should appear by logical async context rather than as an invented source
thread. Generated wrapper frames should link both generated location and
decorator application so users can move between framework behavior and their
code.

Variable rendering must follow language safety: secrets stay redacted,
`inmut::strict` cannot be edited, unavailable optimized values are reported as
such, and inspecting a value does not invoke arbitrary user `to_string()` code
or mutate the process. Raw pointer/memory views require an explicit unsafe debug
operation and never weaken program permissions. Release builds may inline,
eliminate, or merge values; the debugger must report those limitations instead of
displaying fabricated state.

---

**Previous:** [← Language Server](11-lsp.md) · **Next:** [ Debug and Release](13-debug-and-release.md)
