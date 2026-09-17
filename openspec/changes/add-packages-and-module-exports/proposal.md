## Why

Zirk currently has a single-crate `share`/`import` mechanism and compiler-known
standard-library names, but it has no coherent package boundary, package root,
multi-file module namespace, or distribution-facing public API. This prevents
the standard library from becoming an ordinary consumer-facing Zirk package and
prevents users from publishing reusable Zirk code without coupling imports to
implementation details.

## What Changes

- Add package identity and package-root semantics for projects described by
  `.zkinit`, while preserving quoted relative imports for files inside the
  current project.
- Add `mod` declarations and `share mod ... from "./path"` re-exports so a
  package root can act as an index without duplicating module implementations.
- Define package-qualified import resolution for standard and external packages
  using unquoted logical paths (`std.io`, `acme.web`) and reserve quoted paths
  for relative source files (`"./models/user"`).
- Extend `share` to publish modules and their selected declarations as the
  package's public API; retain private declarations as non-importable.
- Make published members of an imported module available unqualified by
  default when unambiguous, while supporting explicit `module.member` access.
- Define deterministic collision diagnostics, import aliases, package-root
  re-exports, cycle handling, and source/package identity rules.
- Define the standard library as compiler-provided packages resolved through the
  same public package/module model; Rust-backed behavior remains an internal
  compiler/runtime decision for official standard-library sources only, with no
  `native`, `intrinsic`, or ABI annotations exposed in Zirk source.
- Define package-project metadata, dependency resolution, lockfile integrity,
  package artifacts, and the initial local/registry transport boundaries while
  keeping the existing `.zkinit` and `zirk.lock` model.
- Update the normative language, module, project, standard-library, grammar,
  handbook, roadmap, feature-status, and toolchain documentation; add a
  decision record for package/module terminology and import resolution.
- **BREAKING** Replace the current standard-library-only implicit convenience
  rule and the `use`-for-convenience interpretation with default opening of
  published members of imported modules; retain `use` only where it remains an
  application-global manifest construct, or remove its module-opening wording
  from the language contract.

## Capabilities

### New Capabilities

- `zirk-packages`: Package identity, package roots, package-project metadata,
  dependency resolution, package artifacts, lockfile integrity, and package
  transport boundaries.

### Modified Capabilities

- `zirk-modules`: Add `package`, `mod`, module re-exports, package-qualified
  imports, default member opening, explicit `module.member` access, and
  package-level public API rules.
- `zirk-grammar`: Recognize the package/module declarations, package paths, and
  `module.member` access while preserving quoted relative paths.
- `zirk-standard-library`: Make `std` packages follow the public package/module
  model and define the compiler-owned implementation boundary for standard
  library functions.

## Impact

- Frontend AST, parser, semantic resolver, source-map identity, and cycle-safe
  loader changes across `zirk-ast`, `zirk-parser`, and `zirk-sema`.
- IR/module construction and both native and interpreter entry pipelines must
  consume the resolved multi-file package graph without duplicating module
  contents.
- CLI project discovery and `.zkinit`/`zirk.lock` handling must support package
  projects and deterministic dependency resolution; `.zpkg` publication remains
  gated by the packaging tasks in the roadmap.
- Standard-library source layout and compiler-owned standard-library registry
  must be introduced without exposing Rust implementation annotations to users.
- Existing single-file and same-crate `share`/quoted-`import` programs must
  remain valid during migration; current `std.io` intrinsic behavior remains a
  compatibility path until the package-backed implementation is verified.
- Public documentation in `zirk-lang` and the synchronized companion
  repository `../zirk-lang-site` will change. Website synchronization and
  separate local commits are required after implementation and documentation
  verification.
