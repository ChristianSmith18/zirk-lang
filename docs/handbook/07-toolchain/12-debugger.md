# Debugger

Debug metadata maps native execution to `.zrk`, including tasks, awaits, threads, channels, generated wrappers, decorator applications and expansion paths. Decorator objects are not present at runtime; source maps connect generated behavior to its original application. Release optimization may limit visibility explicitly.

The debugger supports source breakpoints, stepping, call stacks, variables,
threads, structured tasks, channels, and exception stops. A suspended task is
shown by logical async context rather than as an invented source thread.
Generated wrapper frames link both generated location and decorator application
so users can move between framework behavior and their code.

Variable rendering follows language safety: secrets remain redacted,
`inmut::strict` cannot be edited, unavailable optimized values are reported as
such, and inspecting a value must not invoke arbitrary user `to_string()` code
or mutate the process. Raw pointer/memory views require an explicit unsafe debug
operation and never weaken program permissions.

Debug metadata is complete in debug profiles. Release builds may inline,
eliminate, or merge values; the debugger reports those limitations instead of
displaying fabricated state.

---

**Previous:** [← Language Server](11-lsp.md) · **Next:** [ Debug and Release](13-debug-and-release.md)
