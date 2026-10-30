# Target and Evidence Matrix

## Revision boundary

The evidence in this matrix describes the current working tree based on commit
`bed848d376960501244b9839e1eca3677a0866f9`. The feature changes are still
uncommitted, so this is not yet a reproducible committed revision. Replace this
revision boundary with the final source commit before closing task 8.5.

Evidence stages are intentionally distinct:

- **Check/lower**: semantic acceptance and IR lowering.
- **Interpreter/native parity**: the same fixture executes in both engines on
  the named host.
- **Object emission**: LLVM emits an object for the target triple; this does
  not prove linking or execution there.
- **Native execution**: generated Zirk executables run on the named host.
- **WASM execution**: the browser package executes under Node's WebAssembly
  runtime; this does not prove graphical-browser behavior.

## Latest local rerun (2026-10-29)

On macOS arm64, both
`cargo test -p zirk-codegen-llvm --test emission advanced_type_corpus_emits_for_all_nine_targets -- --exact --test-threads=1`
and
`cargo test -p zirk-codegen-llvm --test emission float128_runtime_helper_calls_emit_for_all_nine_targets -- --exact --test-threads=1`
passed in the current working tree. This revalidates the nine object-emission
rows above, not native execution on those target systems.

Also on macOS arm64:

- `cargo test -p zirk-cli --test object_engine_parity generic -- --test-threads=1`:
  5 passed.
- `cargo test -p zirk-cli --test end_to_end generic_function -- --test-threads=1`:
  3 passed.
- `cargo test -p zirk-cli --test end_to_end clone_handles_tuple_nullable_enum_and_union_payloads -- --exact --test-threads=1`:
  1 passed.

These focused executions establish local interpreter/native behavior only.
They do not close the Linux, macOS x86_64, or Windows native host rows.

In an arm64 Linux container (`zirk-linux-arm64-toolchain:latest`, LLVM 20),
`cargo test -p zirk-cli --test object_engine_parity -- --test-threads=1`
passed all 69 parity cases, including localized temporal parsing and the
previously added union, intersection, collection, generic, Clone, Float128,
JSON, Period, TimeShift, and zone cases. The run required building the CLI,
runtime static library, and browser interpreter harness in that container; the
exact oversized-union diagnostic test also passed. Forced-GC end-to-end tests
`map_stored_union_payload_survives_forced_gc`,
`nested_union_enum_tuple_collection_keys_survive_forced_gc`, and
`managed_union_elements_survive_forced_gc_in_ordered_sequences` each passed.
A follow-up `cargo test -p zirk-cli --test end_to_end
generic_function -- --test-threads=1` passed 3 tests, and
`cargo test -p zirk-cli --test end_to_end
clone_handles_tuple_nullable_enum_and_union_payloads -- --exact
--test-threads=1` passed 1. This is local Linux aarch64 container execution,
not a hosted CI result; Linux x86_64, macOS x86_64, Windows, and the hosted
Linux aarch64 run remain unverified for this working-tree revision.

## Required native hosts

| Host | Current evidence | Status for this feature |
| --- | --- | --- |
| Linux x86_64 | Advanced union/generic/collection/intersection/Float128/Clone filters are configured in `.github/workflows/ci.yml`; no result for this working-tree revision is available. | Hosted execution pending |
| Linux aarch64 | Local Linux arm64 container: 69/69 `object_engine_parity` tests on the current working tree; 3 generic function E2E tests; 1 aggregate Clone E2E test; Map/Set, nested aggregate, and ordered-sequence forced-GC tests; locale parsing/formatting, zone, tagged TimeShift, and Period parity tests; and the oversized-union diagnostic passed. Container execution is distinct from the hosted `ubuntu-24.04-arm` runner. | Local container execution evidenced; hosted execution pending |
| macOS x86_64 | Same named CI groups are configured for `macos-15-intel`; local execution evidence is from macOS arm64, not this host. | Hosted execution pending |
| macOS aarch64 | Focused runtime, CLI end-to-end, interpreter/native parity, forced-GC, and temporal/JSON tests listed in `tasks.md` pass locally on macOS arm64. | Local execution evidenced; hosted current-revision result pending |
| Windows x86_64 | Same named CI groups are configured for `windows-latest`; no result for this working-tree revision is available. | Hosted execution pending |

CI configuration is coverage intent only until a run against the final commit
records a result for each host. A green build on one host does not close the
other rows.

## Object-emission targets

`advanced_type_corpus_emits_for_all_nine_targets` and the corresponding
Float128 emission test iterate `zirk_codegen_llvm::TARGETS`. The advanced
corpus includes a generic enum and Clone, a nominal/contract intersection,
scalar and nested aggregate unions, Decimal and union collection keys,
tuple Set elements, and Float128. Its stage is object emission only.

| Target triple | Format | Evidence stage | Current status |
| --- | --- | --- | --- |
| `i686-pc-windows-msvc` | COFF | Advanced type and Float128 object emission | Passed in the recorded local macOS arm64 runs |
| `x86_64-pc-windows-msvc` | COFF | Advanced type and Float128 object emission | Passed in the recorded local macOS arm64 runs |
| `aarch64-pc-windows-msvc` | COFF | Advanced type and Float128 object emission | Passed in the recorded local macOS arm64 runs |
| `i386-unknown-linux-gnu` | ELF | Advanced type and Float128 object emission | Passed in the recorded local macOS arm64 runs |
| `x86_64-unknown-linux-gnu` | ELF | Advanced type and Float128 object emission | Passed in the recorded local macOS arm64 runs |
| `armv7-unknown-linux-gnueabihf` | ELF | Advanced type and Float128 object emission | Passed in the recorded local macOS arm64 runs |
| `aarch64-unknown-linux-gnu` | ELF | Advanced type and Float128 object emission | Passed in the recorded local macOS arm64 runs |
| `x86_64-apple-darwin` | Mach-O | Advanced type and Float128 object emission | Passed in the recorded local macOS arm64 runs |
| `aarch64-apple-darwin` | Mach-O | Advanced type and Float128 object emission | Passed in the recorded local macOS arm64 runs |

## Separate web-target evidence

The release `wasm32-unknown-unknown` browser package ran under Node's WebAssembly
runtime and passed 378 behavior/corpus cases, including locale and zoned
formatting. This is separate from the nine LLVM object-emission rows and from
native host execution. Public JSON streaming is also implemented and exposed
through the Zirk JSON module: the reader accepts text or byte chunks (including
UTF-8 split across byte chunks), and the writer serializes incrementally into
bounded pull chunks with cancellation and output limits. Focused native and
interpreter tests validate the public API; full native host-matrix validation
remains pending.

## Remaining gates

- Run the configured native matrix against the final committed feature revision
  and record the exact run URL, revision, and per-host result.
- Preserve per-stage outcomes for union/container GC combinations and generic
  enum/function/trait/contract/Clone cases; do not infer execution from object
  emission.
- Update this matrix after hosted results and again after source commit so its
  revision boundary names the exact tested commit.
