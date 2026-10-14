## Context

The current compiler resolves quoted imports between files of one crate and
uses `share` for binary visibility. Standard modules are compiler-known and
the current `std.io` surface is an intrinsic-oriented compatibility slice.
`.zkinit` is already the project manifest and `zirk.lock` is the documented
lockfile; `.zpkg` and package-manager commands are roadmap work. See
`proposal.md` and the delta specs for the required public behavior.

## Goals / Non-Goals

**Goals:**

- Build one canonical, cycle-safe source graph spanning local modules and
  resolved packages.
- Make a package root an explicit, reviewable public API assembled with
  `share mod name from "./path"`.
- Preserve quoted relative imports and existing `share` visibility while
  adding unquoted logical package paths.
- Open published members of imported modules by default and provide stable
  `module.member` qualification for explicit calls and collision resolution.
- Let official standard-library declarations look like ordinary Zirk source;
  keep any Rust implementation association in compiler-owned metadata rather
  than source annotations.
- Keep package acquisition, lockfile integrity, permission declarations, and
  public API metadata deterministic and auditable.

**Non-Goals:**

- Implementing the complete standard library in this change.
- Exposing `native`, `intrinsic`, `extern`, or Rust ABI syntax to package authors.
- Allowing external packages to bind arbitrary declarations to Rust runtime
  symbols by spelling or name matching.
- Allowing a package to grant itself application runtime permissions.
- Making browser execution acquire filesystem, process, terminal, network, or
  other ambient package capabilities.
- Replacing `.zkinit` with a new manifest format or claiming `.zpkg` publication
  is complete before the packaging tasks are implemented and verified.

## Decisions

### D1 — `package` is the distributable identity; `mod` is the source namespace

`package acme.cli;` identifies the logical package, while `mod terminal {}`
defines a module within it. A package root (`src/package.zrk` by convention)
is an index, not a second implementation namespace. “Library” remains an
ecosystem description, not a source-language category.

**Alternative rejected:** treating every source directory as an implicit
package. Directory names are physical layout and cannot provide stable identity
or dependency/version boundaries.

### D2 — `share mod ... from` is the only package-root re-export form

The package root publishes modules and selected declarations explicitly:

```zirk
package acme.cli;
share mod terminal from "./terminal";
```

The relative path is resolved from the declaring file, omits `.zrk`, and is
never exposed as the consumer's import path. The loader assigns one canonical
identity to the target module even when multiple roots re-export it.

**Alternative rejected:** implicit directory indexes or exporting every file in
`src`; both make public API and compatibility commitments accidental.

### D3 — Quoted sources are local paths; unquoted sources are package paths

The import resolver classifies source syntax before filesystem lookup:

```zirk
import { User } from "./models/user"; // RelativeModule
import { Parser } from acme.compiler;  // ExternalPackage or project package
import { stdin } from std.io;          // StandardPackage
```

`std.io` and external packages share the logical package-path grammar. Their
ownership comes from the resolver registry and `.zkinit` dependency set, not
from punctuation. A quoted non-relative package-like string is a relative path
and must not silently access a dependency.

**Alternative rejected:** using quotes for all packages. It erases the useful
local/package distinction already documented by Zirk.

### D4 — Imported modules open published members by default

Importing a module creates its qualified binding and a set of unqualified
candidate names for its published members. Resolution accepts an unqualified
candidate only when it is unique after local declarations and other imports are
considered. Explicit qualification uses `module.member` and never depends on
opening state. `use` remains solely the existing `.zkinit` application-global
mechanism; it is not part of module opening.

**Alternative rejected:** opening only compiler-known stdlib objects. That would
make external packages semantically different and prevent ordinary packages
from behaving like reusable Zirk modules.

### D5 — Native standard-library behavior is provenance-based and invisible

The standard-library source contains ordinary Zirk declarations and bodies.
The compiler's trusted standard-library registry may associate a fully
qualified declaration (package, module, name, and signature) with a Rust
runtime implementation for a target. The association is allowed only for the
official standard-library source set and is not expressible through source
annotations. External declarations with the same short name remain Zirk code.

**Alternative rejected:** public `native`/`intrinsic` declarations. They leak
ABI details, create an unsupported extension mechanism, and undermine the rule
that standard-library internals are compiler-owned.

### D6 — Package dependencies are project metadata, not source directives

`.zkinit` declares package identity/project kind, dependency constraints,
targets, permissions, and other existing manifest concerns. `zirk.lock` fixes
resolved package versions, sources, and hashes. Source imports name logical
packages only. Acquisition backends (local path, registry, Git, or vendored
artifact) remain interchangeable behind the resolver.

### D7 — Keep native and browser graphs honest

Native builds may link the Rust runtime for compiler-owned standard-library
declarations. The browser interpreter receives only the explicitly bounded host
surface and must reject package/module effects that require unavailable ambient
authority. Package resolution itself may be shared, but browser execution must
not pull in `zirk-runtime` or native ABI machinery.

### D8 — Proposed ADR

Materialize `docs/decisions/ADR-021-package-and-module-boundaries.md` during
implementation. Its decision records D1–D4, the distinction between logical
package paths and relative source paths, default member opening, collision
diagnostics, and the compatibility migration from current single-crate
`share`/`import`. It must explicitly state that no Rust binding syntax is
public and that official stdlib provenance is compiler-owned.

## Risks / Trade-offs

- **[Implicit opening can create collisions]** → Require source-order-independent
  ambiguity diagnostics and always support `module.member`.
- **[Package identity can drift from physical layout]** → Store the logical
  identity in `.zkinit`, validate package-root declarations, and preserve both
  canonical and written paths in diagnostics.
- **[Re-exports can duplicate initialization or declarations]** → Intern every
  source module by canonical package/module identity and load it once.
- **[Hidden compiler-owned stdlib mappings can be difficult to audit]** → Keep a
  checked-in registry, require fully qualified signatures, and test that
  external same-name declarations never resolve to Rust.
- **[Package resolution expands the attack surface]** → Verify lockfile hashes,
  preserve application-owned permissions, reject undeclared dependencies, and
  keep runtime from prompting or editing manifests.
- **[Current single-file compiler cannot adopt this atomically]** → Land the
  loader/source-map layer behind the existing frontend, keep quoted imports
  valid, and migrate standard-library intrinsics only after package fixtures
  and native/browser parity tests pass.

## Migration Plan

1. Add package/module syntax and a graph loader while retaining current
   single-file entry behavior.
2. Represent the existing same-crate `share`/quoted-`import` graph through the
   new canonical identities and preserve diagnostics.
3. Add package roots, unquoted package resolution, lockfile checks, and package
   fixtures.
4. Move the documented `std.io` surface behind the standard package registry;
   keep the old intrinsic lowering as a temporary compatibility path until the
   new path is verified.
5. Update normative docs, handbook examples, feature status, and the companion
   website from a clean committed revision.

Rollback is source-compatible at each intermediate step: disable package-root
loading and retain the existing single-file resolver until the next migration
slice is verified. Do not publish a package artifact from a partially migrated
compiler.
