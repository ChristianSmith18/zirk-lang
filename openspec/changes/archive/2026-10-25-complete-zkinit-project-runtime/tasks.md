## 1. Contract and migration baseline

- [x] 1.1 Review the live manifest parser, approval flow, resolver, compiler entry checks, module loader, LSP, and VS Code package; record the current behavior and preserve all pre-existing worktree changes, verified by a focused inventory and `git diff` review.
- [x] 1.2 Freeze representative application and package `.zkinit` examples covering optional metadata, entry/root defaults, source roots, application-only `build_targets`, shorthand/expanded dependencies, dev dependencies, globals, and one `requires` list; verify each example against the accepted spec deltas.
- [x] 1.3 Freeze the finite capability identifiers and operation/resource normalization rules, verified by a table mapping every privileged standard API to one capability and one runtime enforcement point.
- [x] 1.4 Freeze migration behavior for legacy permissions, dependency declarations, package entry fields, and source-level `fn main`; verify old authority is never carried forward as consent.
- [x] 1.5 Freeze lock and registry source identity rules, including default/named registry, local path, Git, vendored source, and integrity data; verify each dependency example maps to one stable logical import name.
- [x] 1.6 Record the contract decision log and remaining implementation constraints in the change design before code work begins; verify `design.md` contains no unresolved question that changes scope or sequencing.

## 2. Typed `.zkinit` parser and project model

- [x] 2.1 Define one typed manifest AST for project metadata, source roots, application build targets, dependencies, dev dependencies, globals, and requires, including source spans; verify application and package manifests parse to the same project model with type-specific fields.
- [x] 2.2 Parse the canonical `project` block, optional author/description/license fields, application `entry`, package `root`, repeated source roots, and application-only `build_targets`; verify missing required fields, duplicate fields, wrong-type fields, unsupported targets, and out-of-root paths produce field-specific diagnostics.
- [x] 2.3 Parse dependency shorthand and expanded `from` locators, plus the separate `dev_dependencies` block; verify malformed locator/revision/version combinations identify the exact field.
- [x] 2.4 Parse global shorthand and expanded `type`, `default`, `from`, and `readonly` entries; verify values and type paths retain spans for semantic diagnostics.
- [x] 2.5 Parse the single finite `requires` block and reject `permissions`, `compile_permissions`, and `during`; verify diagnostics point to the removed form and suggest `requires`.
- [x] 2.6 Add canonical manifest formatting and parse-format-parse checks for project metadata, source roots, targets, dependencies, dev dependencies, globals, and requires; verify formatting is idempotent and preserves strings, paths, target order, and logical package names.
- [x] 2.7 Keep a migration-only reader for supported legacy manifests and implement `zirk migrate --check` preview plus explicit apply; verify preview writes nothing and apply is atomic with a recoverable original.
- [x] 2.8 Verify migration converts old permission entries to declarations only, does not create approval records, maps package `entry` to `root`, and reports source-level `main()` migration without rewriting source implicitly.

## 3. Dependency graph, lockfile, and package resolution

- [x] 3.1 Resolve concise version declarations through the configured default registry and expanded named-registry locators; verify both resolve the same logical import path.
- [x] 3.2 Resolve project-relative and vendored packages with canonical path and symlink containment checks; verify package name/version mismatches and root escapes fail before source loading.
- [x] 3.3 Resolve Git dependencies and pin immutable revisions; verify tag movement cannot change a locked build without an explicit update.
- [x] 3.4 Extend `zirk.lock` to record logical name, version, source, immutable revision or artifact integrity, and package root; verify tampering or mismatched metadata is rejected.
- [x] 3.5 Implement `zirk install` to resolve/cache and write or verify the lock without executing user/package code; verify a second install with unchanged inputs leaves the lock unchanged.
- [x] 3.6 Implement `zirk update` as the only command that deliberately refreshes eligible selections; verify build, check, and run never rewrite selected versions.
- [x] 3.7 Make normal build/run resolution exclude `dev_dependencies` and make the designated test/development graph include them; verify consumers do not inherit a package's dev-only dependencies.
- [x] 3.8 Preserve transport-independent source imports and package-root visibility; verify registry, path, Git, and vendored copies of a package use the same unquoted logical import syntax.
- [x] 3.9 Verify lock, source, path containment, and dev-graph behavior with focused `zirk-cli` package-resolution tests and the relevant package CLI checks.

## 4. Entry selection and once-only module execution

- [x] 4.1 Add `zirk run <path>` project-context discovery and explicit entry override; verify the nearest containing manifest, lock, dependencies, globals, and requirements remain active.
- [x] 4.2 Add pathless `zirk run` manifest discovery and application-type validation; verify an application runs its configured/default entry and a package or missing manifest receives a targeted diagnostic.
- [x] 4.3 Preserve standalone explicit-file execution and constrain standalone imports to the documented local/standard roots; verify project lookup does not widen a standalone path outside its canonical root.
- [x] 4.4 Parse file-scope executable statements, retain item order, and require static imports before executable statements; verify malformed order is diagnosed at the import.
- [x] 4.5 Remove the checker requirement for a source `fn main` and treat any same-named function ordinarily; verify a top-level entry without `main` checks and a non-`Void` function named `main` is not an entry error.
- [x] 4.6 Check the complete static graph before runtime effects and compute deterministic depth-first initialization order from import source order; verify compile/type errors prevent every module and entry body from running.
- [x] 4.7 Evaluate each canonical imported module once before its importer and the application entry; verify diamond imports and multiple re-exports do not repeat initialization.
- [x] 4.8 Detect executable initialization cycles while retaining supported declaration-only cycles; verify diagnostics include the full cycle and no partial module body executes.
- [x] 4.9 Replace source-main dispatch in native codegen with an internal ABI entry wrapper that initializes runtime/globals, dispatches the selected file body, waits for structured work, and returns status; verify a native program runs without a user `fn main`.
- [x] 4.10 Verify build target precedence (CLI override, manifest list, then host detection) and host-only `run`; verify module startup error handling, source-order effects, process status, and CLI entry selection with focused parser, sema, IR, codegen, and CLI tests.

## 5. Typed application globals

- [x] 5.1 Resolve `globals` only for applications and build the project global symbol table before source type checking; verify packages cannot declare globals and every source file sees the same application table.
- [x] 5.2 Implement built-in defaults and custom constructor parameter default derivation, including ambiguity and unsupported-type diagnostics; verify a `Person(name: String)` default receives the default empty string and its selected constructor runs once.
- [x] 5.3 Resolve optional `from` paths relative to the project, load type-only source closure without startup effects, and type-check explicit defaults against the declared type; verify missing/invalid files, unresolved types, and mismatched defaults identify the declaration.
- [x] 5.4 Infer `Global.get`, `Global.set`, and `Global.watch` types from literal manifest keys; verify explicit generic arguments cannot override a key's declared type and dynamic/unknown keys are diagnosed.
- [x] 5.5 Enforce `readonly: false` by default and reject only `Global.set` for a read-only global; verify read access remains valid and compatible mutable globals accept only the declared type.
- [x] 5.6 Implement snapshot storage and atomic replacement without mutable aliases; verify mutating a retrieved copy cannot change stored state.
- [x] 5.7a Implement the native bounded subscription hub with per-global revisions, ordered event delivery, explicit close/reset cleanup, and terminal lag reporting; verify no initial event, unchanged publications do not advance revisions, accepted events retain order, one lag error is reported, reset closes active handles, and tokens never wrap into reused handles.
- [x] 5.7b Connect `GlobalSubscription<T>` snapshot/next/close to compiler lowering and native publication; construct previous/current snapshots atomically, suppress structurally equal replacements, close handles at explicit close or scope exit, and verify the typed API end to end.
- [x] 5.8 Initialize globals after project/dependency validation and before imported module bodies; verify constructor failure prevents module and entry execution while ordinary global reads in imported code observe initialized values.
- [x] 5.9 Verify global type inference, defaults, readonly behavior, equality, revision ordering, subscription cleanup, and lag recovery with focused sema/runtime/native tests.

## 6. Unified capability tree and runtime consent

- [x] 6.1 Define compiler metadata for every protected standard API and propagate capability effects through functions, closures, callbacks, and package boundaries; verify indirect calls retain the originating API and source span.
- [x] 6.2 Build a locked dependency capability tree that preserves requester identity, version/integrity, and every dependency path; verify equal effective scopes can share consent without hiding individual requesters.
- [x] 6.3 Implement canonical concrete-resource normalization for the currently exposed network and environment/secret APIs; keep filesystem and process identifiers reserved until those APIs exist. Verify read/write and connect/listen scopes remain independent, network resources normalize canonically, and unsupported filesystem/process resource shapes grant no authority.
- [x] 6.4 Enforce consent immediately before each currently exposed protected side effect, including DNS answers, redirects, reconnects, proxy hops, and dynamic listener binds; verify denied operations perform no host side effect. Filesystem/process effects remain outside the delivered source surface.
- [x] 6.5 Return typed permission errors through preserved `Result` values and use existing contextual projection to produce `null` elsewhere; verify `match`, standalone Result statements, and ordinary value contexts retain their established semantics.
- [x] 6.6 Emit a deduplicated warning for an attempted-but-denied operation with exact capability, resource, and requester path while redacting secret values; verify repeated identical denials do not flood the console.
- [x] 6.7 Add the first-use yes/no prompt at the operation boundary, including application identity, exact resource, requesting package, and dependency path; verify yes permits the current operation and no denies only that operation with a manual PTY acceptance/rejection run and dependency-provenance E2E coverage.
- [x] 6.8 Persist accepted exact scopes in the user-owned approval store bound to project/user/device/lock/requester identity; verify changed project location, scope, version, integrity, or path requires fresh consent through signed-record and dependency invalidation tests.
- [x] 6.9 On accepted consent, ensure the application `.zkinit` exists and add only a missing direct capability declaration when appropriate; verify rejection writes neither manifest nor approval state and transitive needs are not manually copied.
- [x] 6.10 Make no-TTY execution fail closed at the protected operation without blocking or stopping unrelated code; verify a handled denial allows the rest of the application to continue.
- [x] 6.11 Adapt inspect/diff/approve/revoke/history CLI flows to the single `requires` model and trusted consent record; verify revocation takes effect before the next operation.
- [x] 6.12 Verify capability inference, provenance, prompt persistence, stale approval invalidation, exact dynamic scopes including OS-assigned port `0`, no-side-effect denial, and non-interactive behavior with focused permission/runtime tests; document that filesystem/process APIs are not implemented.

## 7. Language server and VS Code integration

- [x] 7.1 Add configuration-specific TextMate scopes for `.zkinit` sections including `source` and `build_targets`, keys, built-in/custom types, option values, strings/versions/target identifiers, numbers, booleans, punctuation, and comments; verify `.zrk` source scopes remain unchanged by grammar/package review and JSON validation.
- [x] 7.2 Apply the agreed role palette while preserving active theme behavior and readable light/dark contrast; verify disabling brand colors does not affect unrelated languages by extension review and JavaScript syntax validation.
- [x] 7.3 Preserve the `zirk-init` language ID, icon, comment/bracket behavior, and exclusion from source check/build/run commands; verify editor command registration and activation contributions in the packaged manifest.
- [x] 7.4 Load unsaved `.zkinit` overlays in the LSP and publish compiler-backed field, dependency, global, entry/root, and capability diagnostics; verify unsaved manifest edits update source type inference immediately with the focused LSP suite.
- [x] 7.5 Add version-aware code actions for direct missing application requirements and fields; verify actions use the canonical formatter, identify external dependency provenance, and never record consent or edit an external dependency silently.
- [x] 7.6 Verify completion/action results are invalidated on newer manifest versions and no stale diagnostics or edits are published with the focused LSP suite.
- [x] 7.7 Copy the finished extension into `~/.devin/extensions/christiansmith.zirk-lang-0.4.2/` and validate extension JSON/JavaScript as required by the repository instructions.

## 8. Project initializer — final product feature before synchronization

This is the final product-code task in the sequence. Finish and verify all items in this section before starting documentation/status synchronization or updating `../zirk-lang-site`; no product implementation task follows it.

- [x] 8.1 Expose `zirk init` as a top-level CLI command and implement its npm-init-style interactive flow, prompting one field at a time for project type, identity, type-specific `entry`/`root`, optional metadata, source roots, and application targets; verify each prompt displays its contract default, Enter accepts it, explicit input replaces it, and blank optional answers are omitted.
- [x] 8.2 Implement `zirk init -y` by accepting the canonical default for every field; verify it runs without a TTY, skips prompts, derives or falls back for the directory name, and omits unset optional metadata/targets while dependencies, dev dependencies, globals, and requirements are empty.
- [x] 8.3 Parse, validate, canonically format, and atomically create only `.zkinit`; verify existing manifests are never overwritten and cancellation, invalid input, or non-interactive execution without `-y` leaves no partial file or consent record.
- [x] 8.4 Verify interactive application/package initialization, package root defaults, `-y` defaults, name fallback, no-overwrite behavior, and failure paths with focused CLI tests.

## 9. Documentation and project-status synchronization

Start this section only after 8.1–8.4 pass. Update documentation from the verified command behavior, then synchronize and review the website in 9.6–9.7.

- [x] 9.1 Update authoritative language/runtime/package/permission/CLI specs and handbook examples with the final `.zkinit` syntax, source roots, build target precedence, `zirk init` prompts/defaults, `run` selection, no-main model, module initialization order, global APIs, and runtime consent behavior; verify examples match parser fixtures.
- [x] 9.2 Add migration guidance for old `permissions`/phase syntax, package `entry`, dependency declarations, and `fn main`; verify migration guidance makes clear that converted capabilities are not consent.
- [x] 9.3 Update Feature Status, Current Limitations, compiler pipeline, handbook roadmap, and `docs/init/ZIRK_ROADMAP.md` only from verified implementation evidence; verify each status claim links to its authoritative source.
- [x] 9.4 Validate the Zirk documentation, specs, and affected Rust crates; record exact commands and results without claiming untested targets or runtime behavior. See 10.1 for the package-level matrix and its combined-run caveat.
- [x] 9.5 Commit the authoritative Zirk implementation/documentation/status snapshot before website synchronization; verify no tracked Zirk content remains uncommitted and preserve unrelated local data. The implementation snapshot is `bd5a4905`, and the final source-content revision before sync is `99b6ee3e`.
- [x] 9.6 Confirm `../zirk-lang-site` is clean, review its status catalog, then run `./scripts/sync-website-content.sh --audit-date 2026-10-25`; the sync generated and validated 372 documents and routes from Zirk revision `99b6ee3e`.
- [x] 9.7 Review the site diff and validate both repositories; commit the website work separately and record revisions. Site content/tests/build passed, and the synchronization commit is `da93c74` (preceded by status and playground commits `83191a3` and `c94cbc6`).

## 10. Final integration gate

- [x] 10.1 Run the focused compiler/CLI/runtime/editor validation matrix for the agreed implementation phase; verify every phase gate and every spec scenario has passing evidence or an explicitly documented target limitation. Results: `cargo test -p zirk-lexer` (107 passed), `cargo test -p zirk-ast` (5), `cargo test -p zirk-parser` (230), `cargo test -p zirk-sema` (703), `cargo test -p zirk-ir` (260), `cargo test -p zirk-runtime-core` (173 passed, 2 ignored), `cargo test -p zirk-runtime --lib -- --test-threads=1` (545 passed, 3 ignored), `cargo test -p zirk-codegen-llvm` (86), `cargo test -p zirk-lsp` (34), `cargo test -p zirk-cli --test check -- --test-threads=1` (65), and `cargo test -p zirk-cli --test end_to_end -- --test-threads=1` (175). `cargo fmt --all --check`, `git diff --check`, extension package/grammar JSON validation, and `node --check editors/vscode/extension.js` passed. The installed extension matches the editor source files; its existing `bin/` directory is an additional packaged artifact. A combined multi-package Cargo invocation hit SIGBUS in the runtime test binary; the runtime suite passes when run independently as listed above.
- [x] 10.2 Run `openspec validate complete-zkinit-project-runtime --strict` and confirm no contradictory old requirement remains in the active specs; validation passes. Two informational archive checks note that one grammar requirement is already synced and one concurrent-scope rename source is absent; the change will be archived with `--skip-specs` because the main specs are already manually synced.
- [x] 10.3 Review generated artifacts and final diffs; both repositories have no remaining tracked changes. Preserve the unrelated local files `demo/34-express-like/build/main` and `docs/security/SECURITY_CATASTRO_2026-10-24.md` without staging or modifying them.
- [x] 10.4 Report delivered phases, exact validation evidence, unresolved target limitations, documentation/site revisions, and remaining tasks without presenting planned behavior as implemented.
