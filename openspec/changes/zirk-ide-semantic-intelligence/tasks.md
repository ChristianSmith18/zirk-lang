## 1. Baseline and contract

- [x] 1.1 Preserve the historical Devin baseline from the 2026-10-22 smoke test; record verified server discovery and F12 behavior, and mark hover as unconfirmed rather than inferring unsupported providers from incomplete evidence.
- [x] 1.2 Add a reusable IDE fixture workspace with local imports, aliases, shadowing, classes/contracts, generic calls, standard-library imports, malformed intermediate edits, and manifest boundaries; verify `zirk check` accepts valid cases and diagnoses invalid cases.
- [x] 1.3 Capture small, medium, and large workspace latency for open/change analysis, hover, and project load in Devin; save measured observations and their single-sample limitations as the baseline for regression thresholds.
- [x] 1.4 Review the proposed capability groups against the current extension manifest, `zirk-lsp` initialize response, and OpenSpec requirements; verify no provider is claimed before it exists.

## 2. Compiler semantic snapshot

- [x] 2.1 Define snapshot-scoped symbol identity, declaration/reference roles, visibility, source ranges, and type/signature data; verify same-name declarations in separate scopes receive distinct identities.
- [x] 2.2 Record declaration identities at declaration registration and record resolved uses at the compiler's actual resolution points; verify locals, parameters, fields, methods, types, imports, and pattern bindings are represented.
- [x] 2.3 Preserve identity edges for cross-module imports and exported declarations; verify a use in an importing file points to the defining file's symbol.
- [x] 2.4 Publish a versioned semantic snapshot with source-map and UTF-16 conversion access; verify spans map correctly for Unicode before a symbol and across multiple source files.
- [x] 2.5 Define recovery behavior for incomplete source and ambiguous resolution; verify a newer failed analysis cannot reuse stale symbol targets from an older document version.
- [x] 2.6 Add compiler semantic snapshot tests for shadowing, closures, imports/aliases, members, generics, and malformed edits; verify every reference target agrees with compiler diagnostics and type resolution.

## 3. Transport-neutral IDE query layer

- [x] 3.1 Audit crate boundaries and implement the pure IDE query API downstream of compiler analysis; verify the compiler pipeline has no dependency on LSP or VS Code crates.
- [x] 3.2 Implement definition and type-definition queries from symbol identities; verify local, member, imported, and standard-library declarations return correct source locations.
- [x] 3.3 Implement references and document highlights; verify results include only the selected identity across modules and exclude a same-spelling local from another function scope.
- [x] 3.4 Implement prepare-rename and workspace rename edits; verify aliases are preserved and collisions, invalid names, reserved words, and capture-changing edits produce no partial workspace edit.
- [x] 3.5 Implement contextual completion and import suggestions using scope, member visibility, project exports, standard library, and declared dependencies; verify inaccessible members and undeclared packages are omitted.
- [x] 3.6 Implement signature help with active parameter and available generic substitutions; verify nested calls and incomplete argument lists select the expected signature.
- [x] 3.7 Implement document/workspace symbol search, semantic classifications, and reliable inferred-type/parameter hint queries; verify results are source-mapped and omit uncertain hints.
- [x] 3.8 Implement diagnostic-backed code actions with versioned edits; verify each action fixes its target diagnostic without changing unrelated ranges.

## 4. Workspace model and index

- [x] 4.1 Resolve workspace root, `.zkinit`, source roots, modules, and dependencies through existing compiler project-loading rules; verify unopened imported files participate in project queries.
- [x] 4.2 Track open document overlays and document versions across multiple files; verify importing-file analysis observes unsaved edits from an open dependency.
- [x] 4.3 Build a bounded index for project exports, standard-library exports, and explicitly resolved dependencies; verify files outside authorized roots are neither loaded nor indexed.
- [x] 4.4 Invalidate dependent snapshots when a source file, manifest, lockfile, root, or resolved dependency changes; verify stale workspace-symbol and navigation results are replaced.
- [x] 4.5 Add cancellation, request supersession, and progress reporting for project loading and analysis; verify an older analysis cannot publish over a newer document/project generation.
- [x] 4.6 Measure query and analysis latency against task 1.3 and set conservative local regression thresholds; verify repeated queries reuse valid snapshots and edits invalidate affected results.
- [x] 4.7 Add an on-demand, bounded import-path index over compiler-authorized local and declared-package roots; verify unopened modules are suggested, undeclared package roots are excluded, and watched source changes refresh the cache.

## 5. LSP protocol providers

- [x] 5.1 Advertise capabilities from implemented providers and client support; verify initialize never advertises an absent or incompatible feature.
- [x] 5.2 Adapt definition, type definition, references, document highlights, prepare-rename, and rename to LSP locations and workspace edits; verify protocol tests cover multi-file versions and cancellations.
- [x] 5.3 Adapt completion, completion resolve, signature help, inlay hints, and workspace symbols; verify client capability negotiation and correct UTF-16 ranges.
- [x] 5.4 Add semantic token full/delta/range support as negotiated with a stable legend; verify declaration/reference modifiers and token invalidation after edits.
- [x] 5.5 Add code actions and safe workspace edits for compiler diagnostics; verify unsupported actions return no edit and do not modify source.
- [x] 5.6 Preserve and regression-test diagnostics, hover, document symbols, formatting, in-memory overlays, and clean shutdown; verify the existing MVP behavior remains intact.
- [x] 5.7 Add protocol tests for initialize, didOpen/didChange/didClose, request cancellation, stale versions, multi-document imports, shutdown, and client feature negotiation; verify no stale response is observable.
- [x] 5.8 Provide semantic hover details on imported declarations, aliases, and module paths; verify imported functions, type aliases/classes, interfaces, records, standard modules, and local modules in unit/protocol tests and Devin.

## 6. VS Code and Devin experience

- [x] 6.1 Update the extension client to expose implemented LSP providers and avoid overlapping CLI providers; verify a running LSP produces one diagnostic/hover/format result per request.
- [x] 6.2 Add server/project readiness states for starting, indexing, ready, fallback, and failed modes; verify Devin clearly identifies which capabilities are currently active.
- [x] 6.3 Map semantic token types/modifiers to the Zirk palette and retain the TextMate grammar as immediate fallback; verify symbol distinctions in Devin and compiler-backed colors for open files outside the entry-point import graph.
- [x] 6.4 Expose navigation, rename, completion, signature, hints, workspace symbols, and code actions through standard editor interactions; verify each interaction against the fixture in Devin.
- [x] 6.5 Exercise unsaved edits, rename confirmation, project refresh, missing-server fallback, and server restart inside Devin; verify stale results are cleared and fallbacks are labeled accurately.
- [x] 6.6 Package/install the extension and `zirk-lsp` through the documented local workflow; verify a clean Devin window can discover the server without relying on a repository-specific manual process.

## 7. Debug Adapter Protocol

- [x] 7.1 Define interpreter debug controls and a source-to-IR mapping using instruction spans; verify a test program can pause at a requested executable source line.
- [x] 7.2 Add interpreter pause/resume/step controls and expose call frames, live values, and heap objects through a bounded debug snapshot; verify the paused state is stable while inspected.
- [x] 7.3 Implement DAP launch, initialize, breakpoints, stackTrace, scopes, variables, continue, pause, step, and terminate for the interpreter; verify a complete Devin debug session.
- [x] 7.4 Report optimized-out, unsupported, or unavailable values and target capabilities accurately; verify no stale runtime values survive resume or termination.
- [x] 7.5 Design native LLVM source-level debug metadata and variable locations, then implement the DAP native target only after source mapping tests pass; verify native breakpoints and locals in Devin.
- [x] 7.6 Add other execution targets only with their own launch, source mapping, pause, and value-inspection evidence; verify unsupported targets fail with explicit capability messages.

## 8. Documentation, status, and release validation

- [x] 8.1 Update authoritative LSP/editor/compiler documentation, feature status, limitations, and roadmap only for capabilities verified as implemented; verify each public claim maps to a test or Devin evidence.
- [ ] 8.2 Update the companion `../zirk-lang-site` from the committed source revision after source docs are committed; review generated changes and the site-owned status catalog, using a reviewed `--audit-date YYYY-MM-DD` when project status changes.
- [x] 8.3 Run focused Rust tests for changed crates, LSP protocol tests, extension validation, and a release-binary stdio smoke test; verify the server returns semantic results and documented target limitations are accurate.
- [x] 8.4 Run the documented Devin end-to-end suite; verify editor rendering, editing workflows, clean-window discovery, and native debugger behavior.
- [ ] 8.5 Validate the OpenSpec change and both repository diffs; verify source and website changes are committed separately and record both revisions for auditability.
