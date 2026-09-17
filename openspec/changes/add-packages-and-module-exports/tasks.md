## 1. Language model and manifest foundations

- [x] 1.1 Define the package/module identity model in the AST and source-map types (`package`, canonical module path, written path, package root) and verify identity round-trips in unit tests
- [x] 1.2 Extend the `.zkinit` model/parser for application versus package projects, package identity, dependency constraints, and configured source roots; verify valid and invalid manifests with `zirk check` diagnostics
- [x] 1.3 Preserve `zirk.lock` package version/source/hash records in the project model and verify malformed or missing integrity fields produce deterministic diagnostics
- [x] 1.4 Add the compiler-owned standard-library package registry and verify that registry entries are keyed by fully qualified package/module/name/signature rather than short names

## 2. Syntax and AST

- [x] 2.1 Add AST nodes for package declarations and inline `mod` declarations, retaining spans and source ownership; verify parser snapshots for nested module declarations
- [x] 2.2 Add AST support for `share mod name from "./relative-path"` and package-root re-exports; verify the target path omits `.zrk` and retains its written span
- [x] 2.3 Extend import-source parsing to distinguish quoted relative paths from unquoted logical package/module paths; verify `"./models/user"`, `acme.web`, and `std.io` parse into distinct source kinds
- [x] 2.4 Parse `module.member` qualification in expressions, calls, and type positions; retain `::` for direct member imports
- [x] 2.5 Update lexer/parser diagnostics and syntax-highlighting fixtures for `package`, `mod`, `share mod`, unquoted package paths, and `::`; verify existing quoted imports remain unchanged

## 3. Source loading and visibility

- [x] 3.1 Implement canonical package/module resolution for local relative modules, project packages, external dependencies, and compiler-provided standard packages; verify each source kind resolves to the expected canonical identity
- [x] 3.2 Implement package-root traversal through `share mod` re-exports without implicitly compiling unreachable files; verify only reachable modules enter the source map
- [x] 3.3 Intern canonical modules so diamond re-exports and repeated imports read each source exactly once; verify declarations and initialization are not duplicated
- [x] 3.4 Preserve binary `share` visibility for declarations and enforce private-helper rejection across package boundaries; verify diagnostics name the importing and defining modules
- [x] 3.5 Add deterministic package and module cycle detection with complete dependency paths; verify legal same-crate declaration cycles remain handled according to the finalized module rules
- [x] 3.6 Implement import aliases and collision diagnostics across opened module members without using file or dependency order as a tiebreak; verify `name -> alias` and ambiguous names

## 4. Default member opening and qualification

- [x] 4.1 Replace standard-library-only convenience resolution with default opening of published members from every imported module; verify an unqualified call resolves when unique
- [x] 4.2 Implement `module.member` lookup for values, calls, and types; verify qualification succeeds even when an unqualified name is ambiguous
- [x] 4.3 Remove module-opening semantics from `use` while preserving `.zkinit` application-global behavior; verify `use` no longer imports or opens module members
- [x] 4.4 Add resolver tests for local declarations, explicit imports, opened members, aliases, and collisions; verify resolution is source-order independent

## 5. Native and interpreter execution boundaries

- [x] 5.1 Route resolved package/module graphs through IR construction without duplicating function, type, or initialization declarations; verify native and interpreter module counts agree
- [x] 5.2 Add compiler-owned standard-library implementation mapping with no public `native`, `intrinsic`, or ABI syntax; verify ordinary stdlib source declarations compile to their registered target implementation
- [x] 5.3 Verify an external package declaration with the same short name/signature is not mapped to a Rust runtime symbol; cover native linking and resolver tests
- [x] 5.4 Preserve the interpreter/browser boundary for package execution and reject unavailable ambient effects before execution; verify no browser dependency acquires `zirk-runtime`
- [x] 5.5 Migrate the current `std.io` compatibility surface to package-backed `stdin`, `stdout`, and `stderr` fixtures while retaining a tested fallback during migration

## 6. Package acquisition and project tooling

- [x] 6.1 Implement deterministic dependency acquisition interfaces for local path, registry, Git, and vendored sources without changing source import syntax; verify each backend yields the same logical package identity
- [x] 6.2 Validate lockfile versions, sources, hashes, and package-root metadata before frontend checking; verify tampered and undeclared dependencies fail before body type checking
- [x] 6.3 Add package-project CLI discovery and diagnostics while preserving single-file `zirk check`, `zirk build`, and `zirk run`; verify both application and package entry modes
- [x] 6.4 Add package API metadata generation for published declarations, canonical identities, signatures, permissions, targets, and documentation references; verify private declarations are omitted
- [x] 6.5 Add package artifact scaffolding compatible with the roadmap `.zpkg` format, but gate publication until reproducible build and integrity checks pass; verify an artifact can be inspected without executing package code

## 7. Tests, examples, and compatibility

- [x] 7.1 Add parser and semantic fixtures for package roots, module re-exports, package paths, relative paths, direct submodule imports, aliases, and `module.member`
- [x] 7.2 Add end-to-end multi-file application fixtures covering local modules, external packages, standard packages, private helpers, re-export diamonds, and collision diagnostics; verify expected output and errors
- [x] 7.3 Add native codegen/link tests and interpreter/browser conformance fixtures for an ordinary Zirk stdlib declaration backed by the compiler-owned standard registry
- [x] 7.4 Add negative fixtures proving external packages cannot invoke hidden Rust bindings, acquire undeclared permissions, or escape package-root visibility
- [x] 7.5 Run `cargo test -p zirk-lexer -p zirk-parser -p zirk-sema -p zirk-ir -p zirk-interpreter -p zirk-browser -p zirk-cli` and record any toolchain limitations
- [x] 7.6 Run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and the relevant project/package CLI checks

## 8. Documentation and decision records

- [x] 8.1 Update `docs/ZIRK_LANGUAGE_SPEC.md` section 10 with package/module identity, `share mod` re-exports, quoted-relative versus unquoted-package imports, default member opening, and `module.member` qualification
- [x] 8.2 Update `docs/handbook/02-handbook/19-modules/` pages to remove module-opening `use` wording, explain package roots and re-exports, document collision rules, and add complete application/package examples
- [x] 8.3 Update `.zkinit`, project layout, dependencies, lockfile, and package handbook pages to describe package projects without introducing `zirk.toml`
- [x] 8.4 Update `docs/ZIRK_STDLIB_SPEC.md`, `docs/handbook/04-standard-library/`, and standard-library indexes so `std` packages use the same public module model and hide Rust implementation details
- [x] 8.5 Add `docs/decisions/ADR-021-package-and-module-boundaries.md` documenting package versus module terminology, import-source resolution, default member opening, collision handling, and compiler-owned stdlib mappings
- [x] 8.6 Update `docs/init/ZIRK_ROADMAP.md`, `docs/init/ZIRK_FEATURE_STATUS.md`, `README.md`, and current limitations to distinguish delivered multi-file imports from pending package publication/registry work
- [x] 8.7 Validate all changed examples with `zirk check`/`zirk run` and validate the OpenSpec change with `openspec validate add-packages-and-module-exports --strict`
- [x] 8.8 After the zirk-lang commit is clean, run `./scripts/sync-website-content.sh` (with a human-reviewed `--audit-date YYYY-MM-DD` if status evidence changed), review the companion site diff, and commit website synchronization separately
