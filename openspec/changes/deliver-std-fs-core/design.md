## Context

See `proposal.md` for motivation and the two-milestone scope. The working tree may contain partial filesystem code; source presence alone is not evidence that a milestone is complete. The implementation must compose with the existing `Result<T,E>`, `Resource<E>`, `match with`, package-identity, permission-consent, and browser-preflight contracts instead of introducing parallel mechanisms.

## Goals / Non-Goals

**Goals:**
- Deliver an inspectable `std.path` and `std.fs` contract with portable signatures and a narrow native host boundary.
- Complete the path from inferred standard-library effect through `.zkinit` requirement provenance, first-use consent, exact resource validation, typed denial, and no side effect when denied.
- Make bounded whole-file operations concise while retaining explicit `File` ownership for progressive reads.
- Bound allocation and progressive read sizes, and cap the process at 256 simultaneously open managed `File` resources.
- Deliver the same documented synchronous behavior on declared native targets and fail closed for browser execution.

**Non-Goals:**
- Directory enumeration/mutation, recursive traversal, symbolic-link APIs, temporary paths, locks, and watchers; these belong to the follow-on change.
- Async file methods or a general asynchronous I/O abstraction.
- A sandbox for all Zirk code or a replacement for operating-system permissions.
- Treating lexical path normalization as authorization or physical identity.

## Decisions

### 1. Keep filesystem behavior in the official source package and bind only narrow host operations

Publish ordinary Zirk declarations under the bundled `std.path` and `std.fs` identities. Implement API composition, validation, outcome mapping, and convenience helpers in Zirk where feasible; reserve native bindings for operations that require host access. Compiler/runtime behavior is associated only with the official bundled package identity. An external package with a matching short name remains ordinary Zirk code.

**Alternative considered:** Add filesystem intrinsics directly to the language grammar or bind by method name. Rejected because that leaks platform APIs into the language core and lets unrelated packages imitate privileged host operations.

### 2. Treat `Path` as a value and separate lexical manipulation from host resolution

`Path` is immutable. Construction, joining, basic decomposition, lexical normalization, and lexical equality do not access the host filesystem or require consent. `absolute`, `canonicalize`, and physical identity checks are fallible host operations. Preserve native path representation internally where possible; conversion to `String` is strict and lossy conversion, if ever exposed, is explicit. Lexical equality never substitutes for authorization.

**Alternative considered:** Keep paths as `String`. Rejected because concatenation and Unicode conversion obscure target separators, native path data, and the difference between lexical and filesystem-aware operations.

### 3. Make the first delivery a complete vertical slice

The first milestone includes minimum `Path`, exact path-scoped permission checks, bounded whole-file reads, direct writes, typed `OpenOptions`, `File.open`, and managed resource cleanup. Do not begin the next-stage API until the first milestone passes path-race, consent, denial, resource-lifecycle, and native-target checks. Convenience reads and writes own and close their internal handles; `File.open` returns a managed non-copyable `File` resource. The runtime permits at most 256 simultaneously open managed `File` resources per process and returns a typed limit error before opening another handle. There is no implicit open mode, and `create_new` uses an exclusive host primitive.

**Alternative considered:** Expose only open/read/close. Rejected because trivial reads would repeat resource boilerplate. **Alternative considered:** Return nullable data or project I/O errors away. Rejected because absence, denial, decoding, limits, and host errors must remain distinguishable and catchable.

### 4. Make progressive iterator errors explicit and tie iterator lifetime to `File`

`File.read_line()` returns `Result<FileLine, FileError>`, where `FileLine` distinguishes `Line(text)` from `End`. `File.lines()` lazily yields `FileLine.Line(text)` and yields one `FileLine.Error(error)` before stopping on failure; normal EOF ends iteration without an extra line. `File.chunks(size)` uses the corresponding `FileChunk.Data(bytes)` and `FileChunk.Error(error)` events. This avoids nesting `Result` inside the language's nullable iterator projection. Iterators borrow their parent `File` and cannot outlive it.

**Alternative considered:** Make iteration throw on I/O failure. Rejected because filesystem failures elsewhere are typed `Result` values. **Alternative considered:** Buffer all lines. Rejected as the default because it scales memory with file size; any future convenience method must remain explicitly bounded.

### 5. Use a conservative, explicit filesystem scope model

Check `filesystem.read` and `filesystem.write` independently at every host effect. The next stage also has `filesystem.metadata`, a narrower read-only capability for type, size, and timestamp inspection; a matching full read grant covers metadata, but metadata consent never covers file contents. Authorize the effective canonical path and bind consent to the requesting application/package provenance. A direct file grant covers only that file and operation. A directory subtree is not introduced here and must never be inferred from a string prefix or package declaration. `.zkinit requires` declares a possible need; it does not grant user consent. First-use consent remains yes/no, records only the chosen concrete scope, and on denial returns `PermissionDenied` before the effect while unrelated program execution can continue.

**Alternative considered:** Treat `.zkinit requires` as a grant or default to project-root access. Rejected because a manifest declaration and a project root do not express informed consent for a concrete resource.

### 6. Make path checks race-resistant at the operation, not just before it

Do not canonicalize a string and then issue an unrelated path-based open/write. After deriving the effective canonical path, open its parent from the filesystem root or volume and traverse one canonical component at a time with no-follow directory opens. Keep each resulting directory handle pinned, validate the final parent handle against the resolved path, and perform the effect relative to that handle; never reopen the complete parent path after authorization. Open existing final targets without following a final symlink and verify the opened target against the resolved path. New targets validate the nearest existing parent and use an exclusive final-name create without a check/use gap. Atomic replacement uses an exclusive unique sibling temporary, syncs its contents, and publishes by same-filesystem rename. This promises old-or-new visibility, not power-loss durability unless a target-specific parent-directory guarantee is separately documented.

**Alternative considered:** Resolve a path, compare strings, then operate on the original path. Rejected because an attacker can replace a component between validation and use.

### 7. Keep the next-stage API behind a verified first-delivery gate

The second milestone reuses the first milestone's resolver, typed errors, resource ownership, and permission checks. It adds bounded partial/exact reads, line/chunk iteration, append, metadata/exists, and atomic replacement. Metadata APIs use `filesystem.metadata`; full `filesystem.read` scope also authorizes metadata at the same exact path. Enforce limits during the read, not only from preflight metadata. Initial documented ceilings are 64 MiB for whole-file reads, 4 MiB per chunk, and 1 MiB per line. Directory, links, watchers, locks, temporary resources, and async behavior remain in the companion change.

**Alternative considered:** Implement every filesystem feature at once. Rejected because traversal, links, watchers, locks, and cancellation introduce separate security and lifecycle risks.

### 8. Keep browser behavior capability-based and synchronous behavior honest

Native filesystem calls in this change are synchronous and may block their calling execution context; effect metadata and documentation must say so. Browser builds reject reachable filesystem operations before any program side effect and never substitute a fake or ambient browser filesystem. Async methods and bounded worker admission are reserved for the companion change.

**Alternative considered:** Add async variants to the first delivery. Rejected to let the follow-on define executor admission and cancellation as one complete contract.

## Risks / Trade-offs

- [Path validation may contain platform-specific race bugs] → Use handle-relative operations, adversarial symlink/replacement tests, and per-target native conformance before reporting delivery.
- [Direct writes can leave partial output after interruption] → Document replacement semantics and keep atomic replacement as a separate next-stage method.
- [A file may grow after metadata is checked] → Enforce byte ceilings while reading and never trust metadata alone.
- [Synchronous operations can stall a task] → State the behavior explicitly and add async methods only in the companion change.
- [Native filesystem behavior differs across targets] → Keep public outcomes stable, document typed unsupported cases, and run target conformance.
- [Bundled packages can make status documentation overstate delivery] → Update implementation status and the website only after implementation evidence is committed and reviewed.

## Migration Plan

This is additive. Existing programs remain valid. After delivery, `std.fs` and `std.path` ship with the toolchain. A first protected operation may add the direct capability to `.zkinit` only through the existing consent workflow; approval remains separate from the manifest and is tied to requester and concrete resource. Browser code that reaches these APIs receives an unsupported-capability diagnostic. Rollback requires reverting the compiler/runtime/package change; user approval records remain outside versioned toolchain directories.

## Open Questions

None. Event types and initial size ceilings are part of this proposal. Any change based on target testing must update the specs, design, and examples together before release.
