# Language Server

Zirk's editor support is built around two layers. `zirk-sema` publishes
compiler-resolved symbol identities and source ranges; `zirk-ide` provides
transport-neutral queries; and `zirk-lsp` adapts those results to the Language
Server Protocol over stdio. The language server does not ask the editor to
guess which declaration a name refers to.

When `zirk-lsp` is available, supported editors receive live diagnostics for
unsaved text, type-aware hover, document and workspace symbols, go-to-definition
and type definition, find references, document highlights, binding-safe rename,
contextual completion, completion detail resolution, signature help with
compiler-resolved generic substitutions, inferred-type and parameter-name
hints, canonical formatting, and semantic tokens negotiated with the client.
For clients that support versioned workspace edits, the server also provides
quick fixes for deprecated numeric type spellings and `Void?`.

Hovering an imported name reports the resolved declaration kind and signature or
type, together with its source module. For `exported_name -> LocalName`, the
hover explains the exported declaration and the local alias. Hovering a module
path distinguishes standard-library modules from local modules.

The negotiated semantic-token legend separates functions, methods, type kinds,
parameters, locals, module variables, fields, and imported names. Declaration,
reference, standard-library, and import roles are modifiers; a resolved alias
uses the target symbol's semantic kind. If semantic analysis cannot classify a
name, the TextMate grammar remains the lexical fallback. A Zirk file opened
inside a workspace also gets semantic coloring when it is not reachable from
the project's entry point: the server analyzes that open file as a standalone
root and caches the result against the current document and workspace snapshot.
The standalone pass supplies semantic tokens and diagnostics for that open
document; project navigation and workspace queries continue to use the
compiler-loaded project graph.

The server analyzes the current workspace through the compiler's project and
module-loading rules. Open files are supplied as in-memory overlays, including
unsaved imported modules. An edit schedules analysis outside the notification
handler; an older result is discarded if any open document changed or the set
of open files changed before it finished. Requests wait for a snapshot that
matches the latest document versions. The extension watches `.zrk` files,
`.zkinit`, and `zirk.lock` within open workspace folders so changed source and
project inputs advance the workspace generation and invalidate the loaded
workspace graph. Clients that negotiate work-done progress receive an
indeterminate `Analyzing Zirk workspace` notification while the compiler loads
and checks the project. The compiler pipeline remains separate from LSP and
editor dependencies. If an import is temporarily
unresolved while the user edits its path, the editor keeps a partial snapshot
of the current text so path and export completion can help repair it; the CLI
and build frontend retain their stricter loading behavior. Rename preparation
and rename edits stay disabled until all imports resolve, so a partial graph
cannot omit references from the edit.

The VS Code extension displays whether the language server is starting,
indexing an open document, ready, unavailable with CLI fallback, or stopped.
When the server is running, its diagnostics and semantic providers take
ownership of their paths so the CLI bridge does not publish duplicate results.

## Current limits

Analysis currently reruns the compiler frontend for the loaded workspace graph
and serializes those passes. A separate, bounded filesystem index is built on
demand for import-path completion, only from local and package roots already
authorized by the compiler loader; it is reused until a watched project input
changes. The index discovers module paths; when imported-name completion targets
an unopened module, the server parses only that selected file and offers its
top-level `share` declarations. It does not type-check that file or produce
semantic references until the compiler loads it into the graph. LSP request
cancellation returns the protocol cancellation response and abandons the
request's result. A superseded analysis cannot publish stale results after an
open-document, workspace-generation, or file-set change. Once the frontend is
running in a blocking worker, its CPU work cannot yet be interrupted
cooperatively; progress has no percentage, and the separate import-path index
does not report granular progress.

The interpreter Debug Adapter Protocol adapter launches source files or
projects on the supported interpreter subset, maps breakpoints from IR source
spans, supports pause, continue, and stepping, and exposes bounded paused
snapshots of frames and locals. Unavailable values are labeled, stale handles
are invalidated, and SecretString values are redacted. The interpreter rejects
modules that require native host capabilities such as process-environment
access and reports the missing capability.

For native debugging, `zirk build --debug-info <file>` emits LLVM DWARF line and
local-variable metadata. On macOS, the compiler runs `dsymutil` and writes the
standard `<executable>.dSYM` bundle because Apple's linker does not retain the
object's DWARF sections in the executable. The editor's native target launches
`lldb-dap`; the interpreter target continues to use `zirk-dap`. The current
DWARF language code is C because DWARF does not define a Zirk language code.
Native debugging depends on a compatible LLDB installation and remains target
toolchain-specific. Breakpoints, stepping, stack frames, and local values have
been accepted in Devin on macOS; this does not establish acceptance for other
native toolchains. Browser debugging is unsupported.

Range formatting, automatic import insertion, and broader diagnostic fixes
also remain planned work.

The language server does not execute build scripts, decorators with build
authority, or application code merely because an editor opened a project.
Operations requiring authority must remain explicit user actions through the
trusted CLI.

---

**Previous:** [← Linter](10-linter.md) · **Next:** [Debugger →](12-debugger.md)
