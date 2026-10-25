## Why

Zirk needs a small, safe standard API for working with files before expanding into the full filesystem surface. Delivering bounded reads, writes, explicit file ownership, and path-scoped consent first gives ordinary programs useful file access while keeping memory use, resource cleanup, and user authority predictable.

## What Changes

- Deliver the synchronous filesystem API in two ordered milestones within this proposal. The second milestone starts only after the first milestone's path, permission, resource, and native-target checks pass.
- **First delivery — minimum Path and managed files:** add the bundled `std.path` and `std.fs` packages; an immutable `Path` with construction, string conversion, joining, basic components, parent/name, lexical normalization/equality, and fallible host resolution; typed filesystem errors and open options; bounded whole-file text/byte reads; direct text/byte writes; `File.open`; and a managed `File` resource usable with `match with`.
- Enforce `filesystem.read` and `filesystem.write` independently at the concrete path operation. The next stage adds `filesystem.metadata` for metadata-only inspection; an existing read grant also permits metadata at that exact path, while metadata consent never permits content reads. First-use consent identifies the exact operation, effective path, and requesting application or dependency. On approval, add the capability to the application's single `requires` declaration only for application code; preserve dependency provenance for library requests, ensure a minimal `.zkinit` exists, and save exact resource consent separately. A manifest requirement alone never grants consent.
- **Next stage — progressive and durable operations:** add reads from an opened file, exact reads, line-by-line reading, lazy lines and chunks, append, immutable metadata and existence checks, and atomic text/byte replacement. Preserve bounded memory use and the same path/permission rules.
- Keep text decoding strict UTF-8 by default; support only explicitly listed encodings. Set documented initial ceilings of 64 MiB for whole-file reads, 4 MiB per chunk, 1 MiB per line, and 256 simultaneously open managed `File` resources per process; reject caller limits outside the supported range before allocating or opening.
- Use typed `Result` outcomes for filesystem failures and make denied operations non-fatal to unrelated program execution. Browser execution rejects reachable filesystem effects during preflight.
- Exclude directory traversal or mutation, symbolic-link APIs, temporary paths, locks, watchers, and asynchronous filesystem calls. Those belong to the sequenced follow-on proposal `complete-std-fs-advanced`.
- Update the standard-library contract, handbook pages, examples, status evidence, and then synchronize the companion `../zirk-lang-site` from a committed source revision.

## Capabilities

### New Capabilities
- `zirk-filesystem`: Path values, managed files, bounded and progressive file operations, typed outcomes, and native/browser behavior.

### Modified Capabilities
- `zirk-standard-library`: Add the official `std.path` and `std.fs` packages and their public API contract.
- `zirk-permissions`: Define exact path and operation authorization for file effects, including first-use consent and denial behavior.

## Impact

- Compiler-owned package loading and bindings, semantic capability effects, IR/lowering, native code generation, Rust runtime, CLI consent, and resource cleanup.
- Native host filesystem effects and target conformance; browser capability preflight must fail before execution.
- Public documentation and implementation-status evidence in `zirk-lang`, plus the companion `../zirk-lang-site` repository after the source revision is committed.
- No filesystem permission is granted merely by adding it to `.zkinit`, and no broad directory-subtree grant is introduced by this change.
