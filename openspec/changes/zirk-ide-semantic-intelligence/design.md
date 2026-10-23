## Context

See `proposal.md` for motivation and scope. Today, `zirk-lsp` runs the shared frontend on the current open document and its disk imports, then offers diagnostics, hover, document symbols, and formatting. It performs whole-crate analysis under a single analysis lock. The checker exports type entries with spans and kinds, while the scope resolver returns name bindings during checking; neither public analysis result is yet a complete declaration-to-use index. The extension has a lexical TextMate grammar and a CLI fallback. Existing compiler project and import resolution rules remain authoritative.

The work is deliberately layered. Static source understanding is supplied by compiler analysis; runtime object values are supplied only by a paused debug session. LSP transports static editor queries, while DAP transports debugger sessions. Neither protocol should own Zirk language semantics.

## Goals / Non-Goals

**Goals:**

- Make one compiler snapshot the source of truth for symbols, references, types, visibility, diagnostics, and source positions.
- Keep editor query behavior independent of VS Code and LSP protocol types.
- Provide project-wide features from a bounded, versioned workspace model with unsaved overlays.
- Deliver features progressively and advertise only providers that are implemented and valid for the current client/workspace.
- Preserve fast syntax grammar feedback while allowing semantic tokens to refine it.
- Make Devin a repeatable end-to-end acceptance environment for extension behavior.
- Isolate runtime debugging from static analysis, with explicit per-target capability reporting.

**Non-Goals:**

- Changing Zirk syntax, static type rules, import semantics, runtime behavior, or standard-library APIs.
- Implementing compiler semantics in JavaScript/TypeScript inside the VS Code extension.
- Claiming static analysis can determine a dynamic runtime value or object identity.
- Making DAP a prerequisite for shipping static LSP capabilities.
- Supporting a feature on every execution target before that target can provide the required source maps or runtime hooks.
- Replacing TextMate grammar with semantic tokens; the two layers complement each other.

## Decisions

### Layer 0: Define the editor contract and capability states

Treat editor support as explicit capability groups rather than a binary “LSP enabled” flag:

1. **Language basics:** file recognition, lexical grammar, bracket/comment behavior, commands.
2. **Compiler feedback:** diagnostics, hover, symbols, formatting.
3. **Semantic navigation:** definition, references, document highlights, safe rename, workspace symbols.
4. **Authoring assistance:** completion, signature help, semantic tokens, inlay hints, code actions, import insertion.
5. **Workspace experience:** project discovery, dependency/source indexing, multi-document overlays, progress and cancellation.
6. **Runtime debugging:** launch, breakpoints, stepping, stack, scopes and values over DAP.

The server SHALL advertise each LSP feature only when its provider is implemented and its prerequisites are satisfied. The extension SHALL communicate starting, ready, degraded, and failed states clearly. A CLI-only fallback must never imply that project-wide navigation or completion works.

### Layer 1: Add a compiler-owned semantic snapshot

Extend the checked frontend result with a snapshot-scoped semantic index. A symbol record contains an opaque identity, declaration kind, name, declaration and selection spans, owning module/scope, visibility, and resolved type/signature when known. A reference record contains its span, role (read, write, call, import, type use, member use, or pattern binding), and target symbol identity. The same snapshot carries the source map, diagnostics, module/export graph, and type information.

Symbol identity is guaranteed unique within one snapshot and distinguishes shadowing, imports, overload-like callable/member cases, and same-named declarations. It is not a persistent ID across edits; clients must use document versions and recompute after invalidation. Do not synthesize identity from a name alone. Unsupported or ambiguous syntax has no target identity and must not produce a confident navigation or rename result.

Name resolution should record bindings at the point the compiler already resolves them. Avoid a second parser or a later string-search pass. For incomplete source, preserve results only for regions the frontend can safely recover; discard stale data for changed versions.

### Layer 2: Introduce a transport-neutral IDE query boundary

Build query functions against immutable semantic snapshots, without `tower_lsp`, VS Code types, or process I/O. The boundary accepts a document identity, UTF-16 position/range, snapshot/version, and operation-specific parameters; it returns source-mapped results, edits, completion items, tokens, or no result. It exposes:

- Definition, type definition, references, document highlights, and prepare/apply rename.
- Contextual completion, completion resolution, signature help, and import suggestions.
- Document/workspace symbols, semantic classifications, inferred type and parameter hints.
- Compiler diagnostic code actions and safe source edits.

The exact home should follow the dependency audit: first prefer a dedicated `zirk-ide` library that consumes compiler frontend output; if that would create a dependency cycle, place the pure query boundary in the downstream LSP crate until the frontend exposes a suitable analysis API. In either arrangement, compiler crates must not depend on the LSP or VS Code.

### Layer 3: Add a workspace model and bounded index

The server resolves the project root and `.zkinit` using existing CLI/project-loader rules. It tracks open document overlays and their versions, closed source files discovered through the module graph, declared dependencies, standard-library declarations, and a bounded workspace symbol index. It MUST use the compiler's existing root-containment and package-resolution policies; it must not broaden access by recursively indexing arbitrary filesystem locations.

Import paths use a bounded on-demand file index over only the local and package roots returned by the compiler loader. It caches paths until a watched project input changes, skips symlinks and hidden/build directories, and caps directory entries, source files, and traversal depth. When a valid import points to an unopened indexed file, the query parses that single file and offers top-level `share` declarations; it does not add the file to the checked semantic graph or treat those parsed names as resolved compiler symbols.

Index entries and query responses are tied to a project snapshot generation. Changes to open documents, manifests, source roots, lockfiles, or dependency resolution invalidate only affected inputs and dependent semantic results. New requests cancel or supersede work for older versions. Analysis may begin as whole-crate but must have a measured path to per-file/module invalidation; no hard performance promise is made before baseline measurement.

Use immutable snapshots or read-optimized shared state for queries. Keep workspace loading and analysis off the UI protocol loop. Avoid holding a global mutex while performing filesystem reads or compiler analysis. Publish diagnostics and edits only if their document version and project generation are still current.

### Layer 4: Implement LSP providers as thin adapters

Map LSP methods and client capabilities to the query boundary. Preserve correct UTF-16 ranges and document URIs for cross-file results. Implement, in dependency order, semantic navigation/editing; completions and signatures; semantic tokens and hints; workspace symbols/code actions; then client-specific refinements. Support cancellation, partial progress where appropriate, `workspace/applyEdit`, rename preparation, and token legends. Do not return a partial rename edit if validation fails.

Diagnostics, hover, symbols, and canonical formatting remain supported throughout the migration. Existing CLI fallback behavior remains limited to the operations it can answer from compiler output. The protocol handshake is the authoritative capability declaration.

### Layer 5: Integrate the editor and semantic color system

Keep TextMate grammar scopes as immediate lexical fallback. Add semantic token mappings for symbol kinds and modifiers such as declaration, readonly, mutable, library, deprecated, and unresolved only where the compiler has evidence. The server assigns token categories; VS Code themes assign colors. Keep Zirk's current palette by defining explicit semantic-token theme mappings and test common themes, including distinctions for functions, methods, types, interfaces/contracts, modules, parameters, locals, fields, imported symbols, and declarations versus references.

For an explicitly open `.zrk` file that is not reachable from the project entry, the LSP runs a standalone frontend analysis for semantic tokens and document diagnostics, then caches it for the current document/workspace snapshot. This avoids making colors depend on whether a file happens to be imported and ensures the editor receives a completion notification for its analysis. Navigation, workspace symbols, and cross-file queries remain bounded to the compiler-loaded project graph. Invalid or unclassified text continues to receive the immediate TextMate fallback.

Expose navigation and authoring actions through VS Code's standard commands, completion UI, outline, Problems panel, Code Actions, and status item. Provide a concise language status view for server readiness, project loading, and fallback mode. Do not add custom parsing in the extension to imitate missing semantic features.

### Layer 6: Keep DAP and runtime inspection separate

Implement the interpreter as the first DAP target. Its IR instructions already carry source spans, and its execution state already contains explicit call frames and value slots, so the first milestone can establish the DAP lifecycle, breakpoints, stepping, stack, and value inspection without first building native DWARF support. Add native LLVM output later, after source-level debug metadata and runtime variable-location mapping have been designed and tested. The adapter maps source breakpoints to executable locations and reports verified status. It exposes stack and variable data from a paused process and labels unavailable values accurately. The static LSP may show a variable's compile-time type; only DAP at a pause point can show the current runtime value or object identity.

Native and interpreter targets require different launchers and introspection mechanisms. The interpreter is first because its state is explicit and its instructions retain source spans. The native launch path compiles with source-level DWARF and delegates execution to LLDB DAP; native breakpoints, stepping, stack, and local-value inspection have now been accepted in Devin on macOS. Other host toolchains remain target-specific, and web/browser execution remains unsupported until its host can pause and inspect execution reliably.

### Layer 7: Validate in Devin and CI

Use a committed fixture workspace with multiple modules, imports, shadowed names, generic calls, classes/contracts, standard-library imports, malformed intermediate edits, and a manifest/dependency boundary. Add protocol-level tests for each provider and end-to-end editor tests for visible user behavior.

The requested Devin smoke test uses the actual VS Code-compatible desktop app and this checkout: build the `zirk-lsp` executable, ensure the extension starts it, open the fixture, verify diagnostics and type hover, and invoke definition/references/rename/semantic highlighting as providers become available. Capture whether each interaction succeeds and the active server state. The initial baseline recorded the providers missing at proposal time; current delivery still requires visual acceptance rather than treating protocol tests as a substitute.

**Observed baseline (2026-10-22):** `cargo build -p zirk-lsp` succeeded. After restarting Devin's extension host, the active `.zrk` editor displayed `Zirk: LSP`, confirming that Devin discovered and launched the server. Pressing F12 on the open example did not navigate to a definition, consistent with the current server capability list and extension documentation. The UI hover result was not independently confirmed during this smoke test; the historical hover baseline therefore remains incomplete.

**Local implementation evidence (2026-10-23; not Devin UI acceptance):** `cargo test -p zirk-sema -p zirk-ide -p zirk-lsp -- --test-threads=1` passed 719 tests (41 semantic unit tests, 647 typing tests, 4 IDE query tests, and 27 LSP tests); `cargo test -p zirk-cli --lib -- --test-threads=1` passed 37 tests. Coverage includes UTF-16 quick fixes, completion resolve, generic and incomplete signature help, multi-document unsaved imports, stale document versions, compiler-authorized local/package paths and unopened-module `share` exports, private-member and undeclared-root filtering, recovery during unresolved-import editing, refusal of rename on partial graphs, cached-index refresh after source changes, and file-watcher invalidation after `.zkinit` and `zirk.lock` changes. The built `zirk-lsp` process passed a stdio smoke test that initialized the real binary, opened an unresolved import, suggested an unopened local module, filtered a private declaration, and shut down cleanly. The Devin extension copy passes JavaScript and JSON validation.

**Release-binary LSP protocol smoke (2026-10-23; not Devin UI acceptance):** A stdio client initialized the optimized `target/release/zirk-lsp` against the committed multi-module fixture, opened `src/main.zrk`, and received diagnostics. The server advertised completion, definition, formatting, document highlights and symbols, hover, references, rename, semantic tokens, signature help, type definition, and workspace symbols. Queries returned an inferred `Int32` hover, the `area` definition in `domain/geometry.zrk`, three references, 165 semantic-token integers, the `area` workspace symbol, `fn area(radius: Int32): Int32` signature help, and `display_name`/`name` member completions. This verifies the actual server binary and protocol responses; it does not verify Devin rendering or editor latency.

**Devin LSP acceptance (2026-10-23):** The installed 0.6.0 extension and bundled `zirk-lsp`/`zirk-dap` binaries were synchronized from this checkout. After restarting the extension host with the multi-module fixture open, Devin displayed `Zirk: Starting` and then `Zirk: Ready`; readiness was reached in 2.5 seconds. The fixture had no Zirk diagnostics. Hover on `area` showed `fn area(radius: Int32): Int32`; F12 from its call opened `geometry.zrk` at the declaration; Find References listed the declaration, import, and call across both files. This is a single small-workspace startup observation, not a median or an isolated request measurement. Medium and large workspace timings, edit and hover latency, and regression thresholds remain unmeasured. Devin's screenshot API returned black frames after reload, so semantic-token colors and visual palette consistency could not be reviewed from pixels.

**Devin diagnostics, code-action, and edit acceptance (2026-10-23):** A temporary project containing the deprecated type `BinaryFloat32` produced compiler diagnostic `E0402` in Devin's Problems panel, including its cause and help text; the status bar reported `Zirk: 1 error`. The Quick Fix menu offered `Replace BinaryFloat32 with Float32`. The action was inspected but not applied. Replacing the type in the unsaved editor buffer with `Float32` cleared the diagnostic and returned the status bar to `Zirk: Ready`; undo restored `E0402`. This exposed an extension integration bug: registering `textDocument/publishDiagnostics` as a custom notification replaced the language client's internal diagnostic handler, leaving VS Code without its diagnostic collection. The extension now observes diagnostics through `middleware.handleDiagnostics` and calls `next(uri, diagnostics)`, so the editor receives them while Zirk updates its status. Along with the earlier Devin checks for navigation, references, rename preview, completion, signature help, and workspace symbols, this verifies each interaction in task 6.4. The fixture also exposed the inline hints `Int32` and `radius` beside `area(3)` in the accessibility tree. Rename confirmation, project refresh, missing-server fallback, and clean-window discovery were exercised in the Devin follow-up below.

**Local interpreter DAP evidence (2026-10-23):** The interpreter exposes an owned-module debug session, pre-entry pause, source spans, and bounded immutable stack/local snapshots. Repeated inspection at a pause returns the same snapshot; unavailable locals remain explicit; SecretString values are redacted even when held in derived layouts; the adapter invalidates frame and variable handles on resume and termination. `cargo test -p zirk-dap -- --test-threads=1` passed the adapter unit and process-protocol tests for UTF-8 framing, launch, executable and rejected breakpoints, stack/scopes/variables, stop-on-entry, step-in, pause during an infinite loop, continue, terminate, stale handles, unsupported-target reporting, and normal session completion. This adapter currently runs only the interpreter's browser-safe capability subset: programs that require native environment or other unsupported host operations fail closed with a capability diagnostic.

**Native debug metadata evidence (2026-10-23; Devin acceptance pending):** `cargo test -p zirk-codegen-llvm --test emission -- --test-threads=1` passes 66 tests, including source-line, parameter, and local metadata emission. A debug build of the fixture produces a macOS `.dSYM`; `llvm-dwarfdump` confirms the Zirk functions, source file, lines, local names, and primitive types. The DAP extension target selects LLDB DAP after compiling with `--debug-info`. This establishes symbol generation and launch wiring, not successful native breakpoints or value inspection in the editor.

**Direct native debugger attempt (2026-10-23; inconclusive):** Xcode's `lldb-dap` (LLDB `lldb-2103.0.34.103`) returned its `initialize` response, but a launch request with the documented launch fields, a source breakpoint, and `configurationDone` did not return within 15 seconds. The LLDB CLI created the Zirk target and resolved the breakpoint at `debug_main.zrk:7`, but `run` did not reach a stop within 20 seconds. At the same time, desktop automation reported that macOS was locked, so this first attempt could not distinguish a desktop-state issue from a native-debug integration defect. A later Devin attempt with the desktop available is recorded in the native DAP follow-up below. The request sequence and launch fields were checked against the [official LLDB-DAP configuration reference](https://lldb.llvm.org/use/lldbdap.html).

**Devin interpreter DAP acceptance (2026-10-23):** A temporary launch configuration started the bundled adapter on `debug_main.zrk`, paused at entry in `main`, and displayed the source stack frame. Step-over reached the local assignment and showed `base = 10`; step-in entered `add` with `left = 10` and `right = 5`; step-out returned to `main`; continuing ended the session and disabled Stop/Continue controls. The general semantic fixture's `std.io` import failed closed with `ZB002` (`StdinReadLineResult` unavailable in the browser-safe interpreter), so the dedicated debug fixture intentionally uses no standard-library host operation. Native editor acceptance was completed later in the Devin native DAP follow-up below. Browser DAP remains unimplemented.

**Devin acceptance follow-up (2026-10-23):** After the desktop was unlocked, screenshots rendered normally. The semantic palette was visually checked in `main.zrk`, `domain/geometry.zrk`, and `domain/model.zrk`, covering imported modules and aliases, functions, generic functions, methods, parameters, local variables, fields, types, interfaces/contracts, declarations, and references. The categories were visibly distinct and TextMate remained the lexical fallback. An unsaved edit that introduced a return-type error appeared immediately in Problems and changed the status to `Zirk: 1 error`; undo cleared it and returned to `Zirk: Ready`. Renaming the imported alias `keep` to `retain` produced Devin's confirmation (“2 text edits in one file”); undo restored the original. Editing the imported geometry module on disk from `area` to `area_renamed` refreshed the importing file and produced `E0401` for unresolved `area`; restoring the module cleared the diagnostic. With `zirk.lspPath` temporarily pointed at `/tmp`, Devin reported that it could not create the server connection and displayed `Zirk: CLI fallback`; removing the override and reloading restored `Zirk: Ready`. A clean window discovered the bundled `zirk-lsp` without a `zirk.lspPath` override. The installed extension metadata, JavaScript, `zirk-lsp`, and `zirk-dap` each compare byte-for-byte with this checkout. This completes tasks 6.3, 6.5, and 6.6. It does not measure Devin UI latency, and the `/tmp` override tests an unlaunchable configured server path rather than removing the packaged binary.

**Devin native DAP acceptance (2026-10-23):** The earlier `ENOENT` launch failures were resolved by deferring native compilation until VS Code had substituted `${workspaceFolder}` and by resolving the emitted executable against the configured working directory. In Devin, a temporary LLDB `initCommands` source breakpoint at `debug_main.zrk:7` stopped on the Zirk assignment `inmut result = add(base, 5);`. The selected stack frame was `zk_main` at `debug_main.zrk:7`; the Locals scope displayed `base = 10`, `result = 1`, and `final_value = 1`. This verifies the native DAP launch, source mapping, source breakpoint, stack frame, and local values in Devin. The temporary launch command and test breakpoints were removed after acceptance. Native stepping was not part of this acceptance.

**Devin native DAP stepping follow-up (2026-10-23):** The native fixture was relaunched with a temporary LLDB source breakpoint at `debug_main.zrk:7`. Step-in entered `zk_add` and Devin showed `left = 10` and `right = 5` at the function entry and body; step-over advanced to `return left + right;`; step-out returned to `zk_main` with `(Return Value) = 15`; a further step-over set `result = 15`. Continuing ended the native process with exit status 0. This exposed and fixed a DWARF location issue: parameter slots are now described as local variables after the compiler's parameter-copy prologue, avoiding uninitialized stack values while retaining the source names and types. The focused LLVM emission test and `cargo build -p zirk-cli` passed. The temporary launch command was restored byte-for-byte, generated fixture output was removed, and no LLDB process remains. This completes the native portion of task 8.4 on macOS; it does not claim debugger acceptance on other host toolchains.

**Imported-symbol hover follow-up (2026-10-23):** `textDocument/hover` now resolves imported source names, aliases, and module paths through the compiler semantic index. Imported declarations show their actual kind and available signature/type; aliases identify both the exported declaration and the local name. Module-path hovers distinguish standard-library and local modules. The focused unit test covers a generic function alias, interface alias, record, type alias, class, standard module, and local module. The cross-module LSP protocol test covers an imported standard module binding, function/alias, interface and its `NamedValue` alias, record, and both module-path kinds. Both focused tests passed. In Devin, after reloading with the repository's current `target/debug/zirk-lsp`, hover displayed `fn identity(value: T): T` and `Imported as keep from ./domain/geometry`; other hovers displayed `interface Named`, `record User`, `module std.io`, and `module "./domain/geometry"`. The screenshot confirmed the alias color convention for both naming styles: `identity` and `Named` stay muted gray, while `keep` and `NamedValue` retain their semantic colors. The capitalized-source case required broadening the TextMate alias rule; after that change Devin showed `interface Named` and its alias provenance in hover while the language status was `Zirk: Ready`. The earlier `/tmp` override was unstable after a reload; the final Devin check used the repository binary path explicitly.

### Alternatives considered

- **Keep expanding `TypeMapEntry`:** useful for hover and hints, but it lacks target identity and cannot safely support cross-file references or rename. Keep it as type information inside the richer snapshot.
- **Implement providers in the VS Code extension:** fast for a narrow feature, but duplicates parser/resolution logic, ties support to one editor, and makes semantics drift. Keep the extension as client and UI.
- **Run a full build for every edit:** expensive and unnecessary for source queries. Use the frontend semantic pipeline for LSP and compiler/build diagnostics as applicable; reserve the backend for runtime/debug work.
- **Adopt incremental infrastructure before measuring:** risks a large design investment before identifying the bottleneck. Add snapshot and invalidation boundaries now, measure whole-crate latency, then choose incremental granularity based on evidence.
- **Ship runtime debugging as an LSP feature:** LSP is request/response source intelligence; process control and paused runtime state belong to DAP. Keep protocols separate.

## Risks / Trade-offs

- **Resolver currently exposes transient bindings rather than a complete cross-file identity graph** → record identity and reference facts at semantic resolution points and test shadowing/import behavior before adding editor providers.
- **Rename can silently change binding resolution** → validate candidate names against scopes, visibility, imports, and collision rules; return no edit on uncertainty.
- **Project-wide indexing can expose files outside the project** → reuse the compiler's existing package roots and containment checks, with explicit tests for traversal and symlink boundaries.
- **Whole-crate analysis can cause typing latency** → collect latency and cancellation baselines in Devin, keep requests versioned, and introduce dependency-aware invalidation where measurements justify it.
- **Themes map semantic token names inconsistently** → provide documented token mappings, theme defaults, and visual checks in Devin for the supplied Zirk palette and common themes.
- **Debug metadata may not preserve all source variables after optimization** → report target capability and unavailable values; validate in a debug build and do not promise values for optimized-out locals.
- **The public roadmap may get ahead of shipped support** → update status only after verified implementation; sync the website from a clean committed source tree and review status changes before publication.

## Migration Plan

1. Add semantic snapshot records without changing current LSP behavior; retain current diagnostics, hover, symbols, formatting, and CLI fallback.
2. Add and test the transport-neutral query layer; enable definition/references/highlights/rename only after identity tests pass.
3. Add workspace overlays and indexing, then completion, signature help, semantic tokens, hints, workspace symbols, and code actions in capability-sized increments.
4. Update VS Code/Devin activation, readiness, and visual mappings while preserving lexical highlighting and CLI fallback.
5. Add DAP behind target capability discovery, initially for the validated native path; add other targets only with evidence.
6. Update docs, limitations, roadmap, and project status based on actual merged features; update and review the companion site from a committed source revision.

Rollback consists of disabling the affected LSP/DAP provider in capability negotiation while retaining syntax grammar, compiler diagnostics, hover, formatting, and the CLI bridge. Do not remove compiler semantic data until all consumers and tests are reverted.

## Performance Baseline

Record analysis, hover, completion, and workspace-symbol latency in Devin against a representative small, medium, and large fixture before setting regression thresholds. The task breakdown includes the measurements and a follow-up threshold decision; this does not change the feature scope or query contract.

**Direct LSP measurements (2026-10-23; five cold-process samples per size, macOS ARM64, optimized local `zirk-lsp`):** Generated workspaces used 1/10, 10/20, and 50/50 imported modules/functions, for approximately 11, 201, and 2,501 functions across 2, 11, and 51 source files. Measurements start after sending the named LSP operation and stop at its matching response or diagnostics publication. Values below are the maximum of five samples in milliseconds.

| Workspace | Initialize | Open to diagnostics | Hover | Full-text change to diagnostics |
|---|---:|---:|---:|---:|
| Small | 1.72 | 0.47 | 0.08 | 0.33 |
| Medium | 1.78 | 1.16 | 0.08 | 0.90 |
| Large | 1.87 | 6.73 | 0.08 | 7.10 |

These are server-process measurements only. They exclude Devin startup, extension activation, editor scheduling, and rendering. The separate Devin observations and provisional local regression thresholds are recorded with tasks 1.3 and 4.6; these server numbers must not be reported as editor latency.

**Standalone-file semantic-color check (2026-10-23):** In a clean Devin window, the open `src/debug_main.zrk` fixture was not reachable from `.zkinit`'s `src/main.zrk` entry. Before the fix, it therefore received only TextMate lexical colors, while the entry file received compiler semantic tokens. The LSP now analyzes an explicitly open out-of-graph file as a standalone root for semantic tokens and document diagnostics. After rebuilding and reloading the bundled 0.6.0 extension, Devin displayed compiler-classified function names, parameters, types, and locals in `debug_main.zrk`, and the status changed to `Zirk: Ready`. The focused LSP test and full `zirk-lsp` crate test suite passed. This was one part of task 8.4; the later Devin native DAP stepping follow-up completes its debugger and end-to-end acceptance.

## Companion Site Audit

**Website sync and review (2026-10-23):** The companion site was regenerated from source revision `5d8903b862e1a7f02b18e19398671564a680c149`. All 372 handbook documents and routes validated. The site-owned project-status catalog reflects compiler-backed LSP providers and interpreter DAP support, and describes `std.net`, optional HTTP/3, browser Fetch limits, and the outstanding Windows HTTP/3 WebSocket cleanup failure. The site syntax theme maps standard modules to orange, module-path separators to white, external modules to neutral text, imported values and aliases to blue, imported types to green, and aliased source names to muted gray. That committed site revision predates the native stepping acceptance recorded above; the site status catalog therefore needs the next source-to-site sync after these source docs are committed.

Site verification passed `yarn content:verify`, `yarn content:validate`, `yarn content:test` (82 tests), and `yarn build` (378 prerendered routes; existing bundle and SCSS budget warnings remain). The site changes are committed separately as `f1da43cb9e8e6d06a4c2ad506b4b5f2bf13e30a4`. Devin has now verified standalone-file semantic colors and the full documented editor/native-debugger acceptance suite; task 8.4 is complete. The next website sync must carry this later native debugger status after the source docs are committed.
