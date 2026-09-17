# ADR-021: Package and module boundaries

## Status

Accepted

## Context

Zirk needs a source model that keeps physical files private while allowing a
standard library and user code to publish stable APIs. The former terminology
blurred a distributable library, a file and a namespace, and made standard
library convenience behavior special rather than a general language rule.

## Decision

- `package` names the logical, importable and distributable unit. “Library” is
  documentation terminology for a package intended for reuse.
- `mod` names a namespace published by a package. A root publishes a file with
  `share mod name from "./relative-path"`; the source path is never part of a
  consumer import.
- Quoted imports are relative source paths. Unquoted imports are logical
  package paths. Acquisition may be local, registry, Git or vendored, but it
  is declared in `.zkinit` and pinned by `zirk.lock`, not exposed in source.
- `share` is the sole publication marker. Private declarations remain private
  inside a published module.
- Importing a module opens its shared members only while a bare name is unique.
  There is no source-order tiebreak: collisions require `module.member`.
  `use` remains limited to application globals.
- The standard library is authored as ordinary package/module surface. The
  compiler owns any host binding behind a fully qualified package, module,
  name and signature lookup. No `native`, `intrinsic` or ABI marker exists in
  Zirk source, and external packages cannot acquire such a binding by name.

## Consequences

Consumers can distinguish `import { X } from "./path"` from package imports
without a separate library-import syntax. Package roots act like deliberate
index files, APIs have stable logical identity across checkout locations, and
tools can colour imported modules distinctly. Qualification remains available
for clarity and for collisions without sacrificing concise calls in the common
case.
