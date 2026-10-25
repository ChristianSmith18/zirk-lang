## 1. Define the filesystem host boundary

- [x] 1.1 Trace standard-package loading, compiler-owned host bindings, capability inference, runtime permission requests, and native target support; record the filesystem boundary and verify external same-name declarations cannot bind to it.
- [x] 1.2 Define public `FileError`, `FileLine`, encoding, option, and metadata-independent value types; verify success, EOF, permission, limit, decoding, and host-error cases have unambiguous typed representations.
- [x] 1.3 Add official `std.path` and `std.fs` package roots and source declarations; verify native standard-package resolution and that host bindings resolve only from the bundled package identity.

## 2. First delivery: minimum Path and race-resistant path authorization

- [x] 2.1 Implement immutable `Path` construction, string conversion, joining, basic components, parent/name, lexical normalization/equality; verify these operations do not access the filesystem or claim authorization/physical identity.
- [x] 2.2 Implement fallible absolute/canonical path resolution and native path conversion; verify invalid, missing, denied, and non-Unicode conversion outcomes on supported targets.
- [ ] 2.3 Implement handle-relative effective-path validation for existing and new files on macOS, Linux, and Windows; verify a component or symlink swapped between resolution and use cannot escape the approved scope.
- [x] 2.4 Propagate `filesystem.read`, `filesystem.write`, and next-stage `filesystem.metadata` effects through direct, indirect, and higher-order calls; verify requester provenance identifies the application or dependency that requested the operation.
- [x] 2.5 Wire first-use consent and non-interactive denial to concrete canonical file paths; verify approval stores only the selected scope, rejection has no filesystem side effect, warnings identify requester/resource, and unrelated code continues.

## 3. First delivery: bounded reads, direct writes, and managed File

- [x] 3.1 Implement typed read-only, write-only, read/write, create, create-new, and truncate options; verify empty modes fail before opening and `create_new` is atomic under competing creation.
- [x] 3.2 Implement `File.open` and the managed non-copyable `File` resource with a 256-resource process ceiling; verify the ceiling rejects before opening, and `match with` closes on normal exit, early return, error, and cancellation while preserving close failures.
- [x] 3.3 Implement bounded whole-file `File.read_bytes` and `File.read_text` with a 64 MiB hard ceiling; verify the caller limit and read-time ceiling reject growth without allocating the full oversized file.
- [x] 3.4 Implement strict UTF-8 defaults and explicitly supported encodings; verify malformed text returns a decoding error and byte reads preserve arbitrary bytes.
- [x] 3.5 Implement direct `File.write_bytes` and `File.write_text` with documented create/replace semantics; verify new-file and replacement writes, and verify denied writes preserve prior contents.
- [ ] 3.6 Gate the next stage on first-delivery conformance; verify permission, path-race, resource-lifecycle, typed-error, browser-preflight, and supported-native-target checks all pass before progressive APIs begin.

## 4. Next stage: progressive reads, append, metadata, and atomic replacement

- [x] 4.1 Implement bounded partial/exact reads, `read_line`, lazy line events, and fixed-size chunk events; verify LF/CRLF, empty lines, EOF, premature exact-read failure, one-error-then-stop, and 1 MiB line/4 MiB chunk ceilings.
- [x] 4.2 Tie line/chunk iterators to their parent `File`; verify an iterator cannot outlive a closed file and managed cleanup still runs on early loop exit.
- [x] 4.3 Implement append operations with native append semantics; verify they preserve existing contents and require write consent independently of read consent.
- [x] 4.4 Implement immutable metadata and `File.exists`; verify metadata-only scope does not grant content access, full read scope permits metadata, and absence, denial, wrong type, and host failure remain distinct.
- [x] 4.5 Implement atomic text/byte replacement with an exclusive same-directory temporary, file sync, rename, and cleanup; verify readers observe old-or-new complete contents, pre-publication failure preserves old contents, and documentation does not promise power-loss durability without target evidence.

## 5. Integrate targets, diagnostics, and compiler behavior

- [x] 5.1 Integrate official filesystem bindings with semantic effects, IR lowering, LLVM code generation, and the native runtime; verify signatures and capability effects remain consistent through direct and indirect calls.
- [x] 5.2 Add browser capability preflight for every reachable filesystem operation; verify unsupported effects fail before execution and never use ambient or in-memory filesystem behavior.
- [ ] 5.3 Add target-specific native conformance for macOS ARM64, Linux x86_64, Linux ARM64, and Windows x86_64; verify equivalent public outcomes or explicit typed unsupported behavior.
- [x] 5.4 Verify typed failures compose with contextual `Result` projection; verify explicit matching receives permission/decoding/limit errors and ordinary denial does not terminate unrelated program flow.

## 6. Publish and synchronize the delivered contract

- [x] 6.1 Update `docs/ZIRK_STDLIB_SPEC.md`, `std.fs`/`std.path` handbook pages, API indexes, examples, and error/permission/resource links; verify documented signatures and examples match the sources and specs.
- [x] 6.2 Update `docs/init/ZIRK_FEATURE_STATUS.md`, limitations, and roadmap only after target and security conformance; verify delivered, next-stage, and deferred APIs are clearly separated and target gaps remain explicit.
- [ ] 6.3 Review and commit the complete `zirk-lang` change before website synchronization; verify the commit contains implementation, tests, specs, and documentation evidence.
- [ ] 6.4 Run `./scripts/sync-website-content.sh` from the clean committed source state, review the generated `../zirk-lang-site` diff and status catalog with an explicit audit date, then verify both repositories and report their separate revisions.
