# Debugger

> **Implementation status:** interpreter debugging is implemented for the
> supported interpreter subset. Native source debug information and the LLDB
> DAP launch path are implemented experimentally; end-to-end acceptance in
> Devin is still pending. See the [feature status](../11-reference/12-feature-status.md)
> and [compiler status](../../init/ZIRK_FEATURE_STATUS.md) for current limits.

Zirk exposes debugging through the Debug Adapter Protocol. The editor selects a
target in `launch.json`; the interpreter uses the bundled `zirk-dap` adapter,
while native debugging builds the selected source with `zirk build
--debug-info` and starts `lldb-dap` against the resulting executable.

The interpreter adapter maps breakpoints to IR source spans and provides
continue, pause, stepping, bounded stack frames, and local-variable snapshots.
Values the interpreter cannot inspect are reported as unavailable. Handles are
invalidated when execution resumes or terminates, and `SecretString` values
remain redacted. The interpreter accepts only its supported capability subset;
programs that need unavailable native host operations fail with an explicit
capability error.

Native debug builds emit LLVM DWARF line and local-variable metadata. On macOS,
the compiler runs `dsymutil` and creates the standard `<executable>.dSYM`
companion bundle because Apple's linker does not retain the object's DWARF
sections in the executable. The current DWARF language code is C because DWARF
does not define a Zirk language code. Native debugging requires a compatible
LLDB installation and remains target-toolchain-specific. The current acceptance
work still needs to verify native breakpoints, stepping, stack frames, and local
values in Devin; the existence of DWARF records alone does not establish that
editor behavior.

The current debugger does not yet expose structured tasks, async logical
contexts, channels, or decorator-generated frames. Future source maps should
connect generated wrappers and decorator applications to their original source
locations. A suspended task should be shown by logical async context rather
than as an invented source thread.

Variable rendering follows language safety: secrets stay redacted,
`inmut::strict` values cannot be edited, optimized-out values are reported as
unavailable, and inspecting a value does not invoke arbitrary user
`to_string()` code or mutate the process. Raw pointer or memory views require an
explicit unsafe debug operation and never weaken program permissions. Release
builds may inline, eliminate, or merge values; the debugger reports those
limitations instead of displaying fabricated state.

---

**Previous:** [← Language Server](11-lsp.md) · **Next:** [Debug and Release →](13-debug-and-release.md)
