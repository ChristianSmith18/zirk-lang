# Generic Dispatch and Target Evidence Matrix

This is the frozen current-stage matrix for task 1.4. A source fixture or a
passing native host test does not imply evidence for another compiler stage,
another target, or another generic composition.

## Feature-stage matrix

| Generic feature | Semantic checking | Lowering / portable IR | Interpreter execution | Native object emission | Native linking / execution | Current disposition |
|---|---|---|---|---|---|---|
| Generic enum construction and direct `match` | Generic enum definitions, concrete instantiations, and branch payloads have source fixtures (`generic_enum_box`, `generic_enum_either`, `generic_enum_nested_payload`); sema now checks payload-free variants against the expected concrete instantiation and diagnoses an uninferable use | `rebox<T>` specializes an enum body with `Full(T)` and `Empty`, returning `Box<T>` while retaining its concrete payload layout | `generic_enum_match_dispatches_after_recursive_substitution` passes native/interpreter parity for `Int32`, `String`, nested `Box<String>`, and `Empty` on macOS arm64 | No advanced generic-enum corpus is yet attached to the nine-target matrix | The focused local native/interpreter test passes on macOS arm64; no five-host advanced generic-enum run is recorded here | Direct generic-body construction and dispatch are implemented for the verified cases; target-evidence task 8.1/8.2 remains open |
| User generic function bodies, type defaults, and value defaults | Declaration-site body/default diagnostics and substitutions are covered by sema tests | Nested specialization and growing-recursion rejection are covered by IR/lowering and CLI tests | No complete all-default interpreter parity matrix is recorded | No dedicated nine-target default/specialization object corpus is recorded | Focused local native E2E covers inferred/explicit arguments and defaults; no five-host matrix is recorded | Behavior task 4.2 is implemented; cross-target evidence remains open under 4.6/8.1/8.2 |
| Generic contract method substitution | Constrained methods and concrete adopters are checked; exported signatures are fingerprinted | Compile-time specialization and stale-metadata rejection are covered | `generic_contract_method_specialization_matches_native_and_interpreter` covers the `Watched<Person>` open-template callback case | No dedicated advanced generic-contract nine-target corpus is recorded | Feature-status CI links focused run 40497901428 for Linux x86_64, Linux aarch64, Windows x86_64, and macOS aarch64; this is not the full required host matrix | Existing focused behavior is implemented; target matrix remains incomplete |
| Generic trait default method substitution | `generic_trait_defaults_substitute_contract_types_for_each_adopter` checks `T: Clone` defaults, receiver properties, `T` arguments/results, and `List<T>` arguments/results for `Int32` and `String` adopters | Contract default bodies specialize per concrete adopter; recursive semantic substitution now descends through arrays, sequence collections, sets, maps, shapes, tuples, unions, and nested generic instances | The same fixture passes interpreter/native output parity for direct class calls and calls through `DefaultContainer<Int32>` / `DefaultContainer<String>` | No dedicated advanced generic-trait nine-target corpus is recorded | Focused local parity passes on macOS arm64; no five-host run is recorded | Behavior task 4.3 is implemented for the verified cases; target evidence remains open under 4.6/8.1/8.2 |
| Nested generic contract arguments such as `Container<Box<T>>` | `nested_generic_contract_mismatch_reports_substituted_types` verifies nested conformance and diagnostics for expected `Box<Int32>` versus actual `Box<String>` | Contract inheritance resolves after nominal registration, recursively substitutes nested enum/class arguments, and records inherited generic contract instances for assignability and dispatch | `nested_generic_contract_arguments_substitute_and_dispatch_recursively` passes through nested contract inheritance and a `Container<Box<Int32>>` view | No dedicated advanced nested-contract nine-target corpus is recorded | Focused local interpreter/native parity passes on macOS arm64; no five-host run is recorded | Behavior task 4.4 is implemented for the verified nested argument and dispatch cases; target evidence remains open under 4.6/8.1/8.2 |
| Enum `Clone` over tuples, nullable values, unions, and class graphs | Recursive component checks cover enum variants, generic substitutions through tuple fields, nullable members, union alternatives, and exact non-Clone paths | Tuple values rebuild field-by-field; enum clones dispatch only on the active variant; union clones project, recursively clone, and re-inject only the active canonical alternative | `clone_handles_tuple_nullable_enum_and_union_payloads` and the existing shared/cyclic class graph tests pass through native execution; focused semantic positive/negative tests pass | No dedicated nine-target Clone corpus is recorded | Focused native execution is verified on macOS arm64 only; other required hosts remain open under 4.6/8.1 | Behavior task 4.5 is implemented for the covered compositions; target evidence remains pending |

## Required target and stage evidence

Each advanced-generic corpus revision must record separate results for the
stages below. “Not run” and “not supported” are different states; an unsupported
row needs a source diagnostic at the earliest stage where the limitation is
known.

| Target/stage | Required evidence |
|---|---|
| Semantic checking | The source corpus accepts supported generic declarations/instantiations and rejects unsupported substitutions with stable source diagnostics. |
| Lowering / portable IR | IR verification proves concrete type substitutions, payload layouts, and dispatch identities; malformed descriptors fail IR validation. |
| Interpreter execution | The same source cases produce the expected branch/payload results, distinct from native output evidence. |
| Object emission: `aarch64-macos`, `x86_64-macos`, `x86_64-linux`, `aarch64-linux`, `armv7-linux`, `x86-linux`, `x86_64-windows`, `x86-windows`, `aarch64-windows` | Each target reports object format/architecture and emission success or a target-specific pre-emission diagnostic. Object emission never counts as executable runtime evidence. |
| Native execution: Linux x86_64, Linux aarch64, macOS x86_64, macOS aarch64, Windows x86_64 | Each host runs the same advanced generic corpus and records revision, test command, and result. The current repository CI workflow lists Linux x86_64, Linux aarch64, macOS aarch64, and Windows x86_64; macOS x86_64 is absent from that workflow and therefore remains an explicit evidence gap. |

## Evidence boundary

The existing `generic_enum_*` fixtures establish selected native source
behavior, not interpreter parity or nine-target object support. The focused
generic-contract workflow cited in `docs/init/ZIRK_FEATURE_STATUS.md` covers
four CI hosts and a specific constrained method case, not all generic enum,
trait-default, or nested-contract cases. Task 8.1/8.2 must add the advanced
corpus to the target pipelines and report every stage independently. No
unverified generic composition is promoted to complete by this matrix.

## Focused verification recorded during implementation

The current macOS arm64 worktree passed:

- `cargo test -p zirk-sema generic_enum_match_reuses_the_expected_instantiation_for_empty_variants -- --test-threads=1`
- `cargo test -p zirk-sema generic_enum_empty_variant_without_expected_instantiation_is_rejected -- --test-threads=1`
- `cargo test -p zirk-cli --test object_engine_parity generic_enum_match_dispatches_after_recursive_substitution -- --test-threads=1 --nocapture`
- `cargo test -p zirk-cli --test object_engine_parity generic_contract_method_specialization_matches_native_and_interpreter -- --test-threads=1`
- `cargo test -p zirk-cli --test end_to_end the_valid_corpus_compiles_and_produces_the_expected_output -- --test-threads=1`

The named parity cases prove the `Watched<Person>` contract method and the
generic enum body on this host. The valid-corpus test also runs the three
`generic_enum_*` fixtures natively. These focused results do not prove the full
five-host matrix or nine-target advanced-generic object emission.

Generic trait-default focused verification on macOS arm64:

- `cargo build -p zirk-browser --example interp_repro`
- `cargo test -p zirk-cli --test object_engine_parity generic_trait_defaults_substitute_contract_types_for_each_adopter -- --test-threads=1` (1 passed)

The parity fixture covers generic receiver properties, constrained `clone`,
direct and contract-typed dispatch, and nested `List<T>` parameters/results for
both `Int32` and `String`. This is focused host evidence only; target coverage
remains open under tasks 4.6, 8.1, and 8.2.

Nested generic-contract focused verification on macOS arm64:

- `cargo test -p zirk-sema nested_generic_contract_mismatch_reports_substituted_types -- --test-threads=1` (1 passed)
- `cargo test -p zirk-cli --test object_engine_parity nested_generic_contract_arguments_substitute_and_dispatch_recursively -- --test-threads=1` (1 passed)

The source case inherits `Container<Box<T>>`, checks `Box<Int32>` conformance,
and dispatches through `Container<Box<Int32>>` in both engines. The negative
case confirms the nested expected and provided types appear in the diagnostic.
These results remain focused macOS arm64 evidence.

Enum `Clone` focused verification on macOS arm64:

- `cargo test -p zirk-sema clone_ -- --test-threads=1` (9 passed)
- `cargo test -p zirk-sema non_clone_enum_diagnostic_names_variant_and_nested_field_path -- --test-threads=1` (1 passed)
- `cargo test -p zirk-cli --test end_to_end clone_ -- --test-threads=1` (7 passed)

The semantic tests cover a generic enum containing tuple, nullable, union,
and cloneable class components plus a non-Clone pointer path naming the
variant, field, and tuple element. Native tests execute tuple, nullable, enum,
and active-union clone paths and assert tuple/enum class references are
independent copies. Existing end-to-end tests cover class graph sharing,
cycles, mutation independence, and collection during clone. These results do
not establish the five-host or nine-target evidence required by task 4.6 and
8.1/8.2.
