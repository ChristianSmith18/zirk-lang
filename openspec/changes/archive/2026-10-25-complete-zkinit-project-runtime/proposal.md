## Why

Zirk has several partially implemented and currently conflicting descriptions of `.zkinit`, project entry, dependencies, globals, and permissions. This change establishes one coherent project contract before those pieces are implemented, so compiler, runtime, package resolution, and VS Code all interpret the same manifest and startup behavior.

## What Changes

- **BREAKING:** Define `.zkinit` as the authoritative project configuration for applications and packages, including project identity, optional author/description/license metadata, application `entry`, package `root`, source roots, and the existing `build_targets` selection.
- Define shorthand and expanded normal/development dependency declarations using logical package names, `from` source locators, and a deterministic `zirk.lock`; make lock refresh and legacy-manifest migration explicit CLI actions.
- Preserve source-root declarations and application `build_targets`: an explicit `zirk build --target` overrides the manifest list, otherwise builds use the list or host detection; `zirk run` targets the host.
- **BREAKING:** Remove the required source-level `fn main`. `zirk run <path>` executes that file in its nearest project context; `zirk run` discovers an application manifest and executes its configured entry. Top-level statements execute in order, with imported modules initialized once before their importers.
- Add application-only typed globals in `.zkinit`, with manifest-driven inference for `Global.get`, `Global.set`, and `Global.watch`, deterministic defaults, read-only enforcement, immutable replacement, and change events.
- **BREAKING:** Replace separate permission/grant/phase manifest forms with one finite `requires` declaration model for applications and packages. Build a provenance-preserving project requirement tree; check concrete resources at the protected operation, prompt interactively on first use, persist explicit consent, and deny only that operation when consent is absent or rejected.
- Give `.zkinit` a config-aware VS Code palette, compiler-backed validation, and safe editor actions after its grammar and project model are stable.
- Add `zirk init` as a top-level CLI command and the final product feature before documentation and website synchronization: interactively ask for project identity and type-appropriate fields, or create a canonical application manifest from documented defaults with `zirk init -y`.
- Reconcile existing grammar, package, permission, module, CLI, type-checker, native-entry, and editor requirements with this contract. Update public documentation, implementation status, limitations, roadmap, and the companion website when implementation begins.

## Capabilities

### New Capabilities

- `zirk-project-model`: Application/package manifest schema, metadata, entry/root selection, and project-aware startup.
- `zirk-globals`: Typed application globals, default construction, access, immutable updates, equality, and subscriptions.

### Modified Capabilities

- `zirk-grammar`: Parse the final `.zkinit` forms and executable top-level source statements; remove obsolete permission syntax.
- `zirk-packages`: Resolve logical-name dependencies, alternate sources, lockfile identity, package roots, and development-only dependencies.
- `zirk-permissions`: Use one `requires` model, aggregate transitive provenance, and enforce concrete capabilities at runtime with operation-local failure.
- `zirk-modules`: Define deterministic, once-only execution of imported modules before their importers, including initialization-cycle diagnostics.
- `zirk-cli-commands`: Add manifest discovery and explicit/default entry selection to `zirk run`, preserve standalone-file behavior, and provide interactive/default project initialization with `zirk init`.
- `zirk-type-system`: Remove the mandatory `fn main` check and type-check entry execution and manifest-declared globals.
- `zirk-native-codegen`: Keep the platform entrypoint internal while dispatching to the selected Zirk file body and lifecycle.
- `async-runtime-core` and `concurrent-scopes`: Make the selected file body, rather than a user `main` function, own the runtime's structured root scope.
- `zirk-decorators`: Remove build/runtime phase grants and align any future protected compile-time effect with the single capability vocabulary.
- `language-documentation-information-architecture`: Make the contributor source map point to the single `requires` model.
- `zirk-lsp`: Load and validate `.zkinit`, resolve global types, and provide version-aware diagnostics and supported code actions.
- `zkinit-editor-recognition`: Add configuration-role highlighting while retaining the dedicated `zirk-init` language and icon.

## Impact

The change affects syntax examples and normative project, module, permission, startup, and CLI initialization semantics. Expected implementation areas include `crates/zirk-cli`, `zirk-parser`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, runtime permission APIs, `zirk-lsp`, and `editors/vscode`. It also changes public documentation and status evidence; implementation work must update the authoritative repository documents first and then synchronize and review `../zirk-lang-site` as required by repository policy. This proposal itself changes no public status or website content.
