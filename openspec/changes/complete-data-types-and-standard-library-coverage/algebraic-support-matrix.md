# Algebraic Lowering and Runtime Matrix

This is the current-stage matrix for task 1.3, not the final delivery matrix.
“Implemented” requires evidence for the named stage; source admission or
portable layout alone is not runtime evidence. Rows classified as pending
remain admitted by the current checker but must not be advertised as complete.

## Union alternatives and positions

| Composition or position | Semantic analysis | Portable IR/lowering | Interpreter/native evidence | Current disposition |
|---|---|---|---|---|
| Flat scalar alternatives such as `Int32 | String` | Admitted; nullable is normalized separately | Canonical tag and inline payload are represented by the tagged-enum bridge | On macOS arm64, IR lowering asserts tag dispatch and exact payload projection (`cargo test -p zirk-ir --test lowering union_match_tests_tags_and_projects_exact_alternative_payloads -- --test-threads=1`); interpreter injection/tag/projection/dispatch passes (`cargo test -p zirk-interpreter interpreter_executes_union_injection_tag_projection_and_dispatch -- --test-threads=1`); native construction, calls, narrowing, match, and optimization-level cases exist | Implemented for covered scalar rows; complete cross-target and GC matrix remains open |
| Scalar singleton alternatives | Admitted and canonicalized | Singleton identity maps to a scalar storage type while retaining its own canonical tag | Native tests cover scalar and enum singleton alternatives | Implemented for covered singleton rows |
| Structural shape alternatives | Admitted | Shape references have canonical alternative identity and tagged payloads | Native tests cover field narrowing, three alternatives, mixed scalar/shape payloads, and forced-GC capture; native/interpreter parity covers these representative shape rows | Implemented for covered shapes; container positions remain unverified |
| Payload enum alternatives | The prototype predicate admits enums | The union bridge preserves the enum payload under its canonical outer tag | On macOS arm64, `cargo test -p zirk-cli --test object_engine_parity payload_enum_union_narrowing_has_native_interpreter_parity -- --exact --test-threads=1` covers payload-bearing and payload-free enum variants mixed with an integer; larger nested compositions remain unverified | Implemented for the tested enum/scalar composition; recursive nested/container and GC rows remain incomplete |
| Tuple alternatives | Rejected by the prototype gate with `NOT_LOWERED` (`E0423`) | Tuple layout support exists independently; its union alternative identity and all payload cases are not proven together | Negative E2E coverage is in `union_unlowered_payloads.zrk`; no positive union-tuple corpus identified | Correctly rejected pending tuple tag identity, payload ABI, interpreter/native behavior, and GC evidence |
| Array/List/Deque/Queue/Stack/Map/Set alternatives | The prototype predicate admits these container families | Canonical identity plus active-tag projection lowers exact union-member type tests for collection values | On macOS arm64, `cargo test -p zirk-cli --test object_engine_parity collection_reference_union_has_native_interpreter_parity -- --exact --test-threads=1` covers member reads from `Array`, `List`, `Map`, `Set`, `Deque`, `Queue`, and `Stack`; `generic_list_union_payload_dispatch_has_native_interpreter_parity` covers enum and Boolean union payloads stored in `List<Value>` | Implemented for these exercised layouts; nested collection payloads, error/cleanup, and full GC boundaries remain open |
| PriorityQueue alternatives | Rejected by the prototype gate with `NOT_LOWERED` (`E0423`) | The union canonical identity function has no PriorityQueue case | Negative E2E coverage is in `union_unlowered_payloads.zrk` | Correctly rejected pending canonical identity, payload ABI, and runtime evidence |
| Union stored as a Map key or Set element | Admitted when each active alternative recursively supplies lawful Hash/Equal capabilities; the active alternative tag participates in equality and hashing | Map/Set receive tag-aware descriptors and compare/hash only the active payload under its canonical tag | On macOS arm64, `union_collection_keys_match_native_and_interpreter` verifies direct `String | Int32` keys/elements, lookup/contains, and tag-sensitive distinction in native and interpreter engines. `map_stored_union_payload_survives_forced_gc` additionally stores a managed text key and union element while forcing collections and confirms both remain usable. Broader nested aggregate/enum alternatives, all GC boundaries, and cross-host execution remain incomplete | Direct union key/element parity and representative forced-GC survival are implemented; recursive composition and full matrix evidence remain open |
| Union stored as a Map value | Map value layout admission accepts union layouts | Map constructors receive tag-aware descriptors for active union references, including structural nesting and enum guards | On macOS arm64, `map_union_values_round_trip_and_match_across_engines` verifies nullable lookup, active tag selection, and enum/integer payload retrieval in both engines; native forced-GC E2E covers direct union payloads, union fields inside structural values, and enum-wrapped structural values through growth and replacement; removal/iteration/clone/cleanup boundaries remain open | Map value parity is covered for these rows; full collection-boundary, target, and GC evidence remains incomplete |
| Union nested in another union or generic instantiation | Normalization flattens unions; generic substitution can materialize concrete alternatives | Canonical layouts and substituted forms exist, but nested combinations are not a complete evidence set | Generic alias and nested-container coverage is incomplete | Incomplete; each admitted composition needs stage evidence |
| Unsupported alternatives or payload larger than 4096 bytes | Unsupported tuple and PriorityQueue alternatives receive `E0423`; oversized payload is rejected during layout validation with `E0513` | `UnionLayoutDescriptor` enforces `MAX_INLINE_UNION_PAYLOAD_BYTES = 4096` before LLVM emission; the exact 4096-byte payload layout is accepted | `unions_with_unlowered_payload_identities_fail_before_backend` checks both source diagnostics and no panic; `oversized_union_payload_fails_during_layout_validation` checks the limit diagnostic and absence of a native executable; `inline_union_payload_at_the_limit_is_accepted` checks the exact IR layout boundary | These explicit rows fail closed before backend emission; remaining rejected type families need enumeration and tests |

IR verifier rejection coverage for malformed union descriptors is also explicit:
on macOS arm64, `cargo test -p zirk-ir --test verification
verifier_rejects_union -- --test-threads=1` passes six negative cases covering
invalid fingerprints, injection payload types, bridge payload widths,
cross-union projections, and dispatch metadata. This validates malformed
portable IR independently of source-level diagnostics.

## Container placement matrix

This table records the current boundary by position. “Admitted/pending” is a
known gap to close in tasks 3.2–3.6; it is not a claim of working parity.

| Union position | Current semantic/lowering status | Current runtime evidence | Required disposition |
|---|---|---|---|
| `Map` key | Admitted when each union alternative has recursive Hash/Equal capability; active tag is part of key identity | Native and interpreter use the tagged union capability for hashing/equality | `union_collection_keys_match_native_and_interpreter` passes on macOS arm64; `map_stored_union_payload_survives_forced_gc` covers managed text-key snapshot survival during forced collection. Nested aggregate/enum alternatives and cross-host evidence remain incomplete |
| `Map` value | Admitted for layouts the value checker accepts | Native forced-GC Map test covers direct union payloads, union inside a structural value, and enum-guarded union; growth and replacement are included | Preserve tag and payload; complete removal, iteration, clone, error cleanup, destruction, and target coverage |
| `Set` element | Admitted when each union alternative has recursive Hash/Equal capability; active tag is part of element identity | Native and interpreter use the tagged union capability for hashing/equality | `union_collection_keys_match_native_and_interpreter` passes on macOS arm64; `map_stored_union_payload_survives_forced_gc` verifies direct managed union elements survive repeated collections. Nested aggregate/enum alternatives and cross-host evidence remain incomplete |
| `Array`/`List` element | Admitted for normalized unions | `List<Value>` preserves and dispatches enum payloads and Boolean values; a union alternative may itself be read through narrowed `Array`/`List` members | Native/interpreter cases cover the tested shapes; nested aggregate/GC and control-flow rows remain open |
| `Deque`/`Queue`/`Stack` element | Lowering maps these sequence families to shared sequence storage and preserves canonical union tags | `union_elements_round_trip_through_ordered_sequences_in_both_engines` checks `Int32` and `String` alternatives through native/interpreter iteration and removal; `managed_union_elements_survive_forced_gc_in_ordered_sequences` forces GC after storing class payloads and verifies each payload remains readable; `removing_union_payloads_from_ordered_sequences_releases_gc_edges` verifies Weak references expire after removal and forced GC | Construction, storage, retrieval, match, insertion-to-GC survival, and removal-to-collection are covered; growth under GC, replacement, iteration under GC, clone, error cleanup, destruction, and cross-host evidence remain pending |
| `PriorityQueue` element | Collection operation exists, but union element composition is not established as supported | No complete union/PriorityQueue runtime corpus identified | Pending; do not infer support from PriorityQueue's scalar delivery |
| Union alternative whose payload is a managed collection | Several collection families pass the current semantic prototype; alternative identity/layout support is family-specific | No complete ownership/GC matrix for each collection family | Pending for each admitted family; unsupported alternatives must diagnose before backend |

## Error, cleanup, and boundary matrix

| Boundary | Frozen current behavior | Evidence / remaining work |
|---|---|---|
| Unsupported tuple or PriorityQueue alternative | `E0423` during semantic checking, before IR/backend; no panic | `unions_with_unlowered_payload_identities_fail_before_backend` checks two diagnostics and no panic |
| Union payload larger than 4096 bytes | IR layout validation reports `E0513` after lowering and before backend emission; no implicit boxed fallback | On macOS arm64, `cargo test -p zirk-cli --test end_to_end union -- --test-threads=1` (14 passed) covers the invalid fixture and verifies no executable; `cargo test -p zirk-cli --test check oversized_union_payload_has_source_level_size_diagnostic -- --exact --test-threads=1` (1 passed) verifies source-located 4100-byte failure |
| Payload exactly 4096 bytes | Accepted as the defined inline ceiling | On macOS arm64, `cargo test -p zirk-ir inline_union_payload_at_the_limit_is_accepted -- --test-threads=1` (1 passed) and `oversized_union_payload_has_source_level_size_diagnostic` verify the layout/build boundary |
| Conflicting structural intersection property | Rejected during semantic analysis with the conflicting property named | `structural_shape_intersections_compose_property_capabilities_and_reject_conflicts` |
| Intersection of unrelated nominal classes | `NOT_LOWERED` during semantic analysis; no runtime representation is promised | `invalid_intersection_type_is_not_lowered_yet` |
| Early return, catchable error, nested match, and cleanup with managed union payloads | Supported for the exercised structural payload path | On macOS arm64, `union_narrowing_survives_return_error_nested_match_and_finally` passes native/interpreter parity and asserts one `finally` cleanup marker; `union_narrowing_payload_survives_error_cleanup_and_forced_gc` forces collections inside `finally` and verifies the narrowed payload remains readable afterward | This control-flow row is covered; broader alternative/container combinations and the full GC boundary matrix remain open under tasks 3.2, 3.6, and 8.3 |

## Intersection views

| Composition | Semantic analysis | Representation/lowering | Evidence and disposition |
|---|---|---|---|
| Structural shapes with compatible properties, optionally viewed as `Object` | Admitted; incompatible property types diagnose | Erased to the same managed object reference | Native object-literal identity test exists; additional mutation, narrowing, and GC cases remain open |
| Class/instance with compatible superclass and contract requirements | Admitted when the intersection has one compatible object representation | Remains a view over one reference | On macOS arm64, `class_contract_intersection_view_keeps_identity_across_engines` verifies a subclass/base-class view plus two implemented contracts and alias mutation in both engines; deeper inheritance and broader contract graphs remain pending |
| Multiple unrelated classes, nullable members, or value/scalar members | Rejected with `NOT_LOWERED` or a conflict diagnostic | No runtime intersection value layout | Must remain a source-level rejection and gain explicit negative coverage |

## Control flow, errors, and cleanup

| Boundary | Current evidence | Remaining requirement |
|---|---|---|
| Tag dispatch and projection | Interpreter and native dispatch instructions exist; invalid projection reports a source span | Compare every newly admitted row and malformed-tag failure across both engines |
| Narrowing and `match` | Scalar/shape narrowing and else-if cases have native tests | Add nested match, early return, catchable errors, and cleanup for payloads containing managed references |
| Managed references | Forced-GC E2E covers captured structural payloads and Map values containing direct, structurally nested, and enum-guarded union payloads through insertion, growth, and replacement. `managed_union_elements_survive_forced_gc_in_ordered_sequences` verifies retained class payloads in Deque/Queue/Stack; `removing_union_payloads_from_ordered_sequences_releases_gc_edges` verifies each family releases its final strong edge and Weak references become dead after forced collections. | Exercise growth/replacement/removal during collection, iteration, clone, error cleanup, and destruction under GC for all admitted algebraic container rows |
| Inline limit | `crates/zirk-ir/src/unions.rs` defines and validates the 4096-byte ceiling; oversized source composition produces `E0513` | Verify exact-limit accepted layouts execute, then retain the oversized pre-backend diagnostic test |

## Audit sources

- Semantic gates and intersection representation rules: `crates/zirk-sema/src/checker.rs` (`resolve_union`, `union_layout_prototype_supported`, `resolve_intersection_expression`, `intersection_has_object_representation`).
- Tagged bridge and narrowing: `crates/zirk-ir/src/lower.rs` (`UnionTypeLayout` construction, `lower_union_subset_narrowing`).
- Portable layout and payload ceiling: `crates/zirk-ir/src/unions.rs` (`UnionLayoutDescriptor::derive` and `validate`).
- Interpreter instructions: `crates/zirk-interpreter/src/lib.rs` (`UnionInject`, `UnionTag`, `UnionProject`, `UnionDispatch`).
- Native ABI and branch dispatch: `crates/zirk-codegen-llvm/src/emit.rs` (`UnionInject`, `UnionProject`, `UnionDispatch`).
- Existing native cases: `crates/zirk-cli/tests/end_to_end.rs` (`scalar_singleton_unions_match_and_execute_on_native_backend`, `structural_union_discriminants_narrow_before_field_reads`, `structural_union_three_alternatives_narrow_across_else_if`, `structural_union_mixed_with_scalar_preserves_tag_and_managed_payload`, `structural_intersection_object_literal_keeps_one_object_identity`, `captured_structural_union_payload_survives_forced_gc`).

The final delivery matrix must replace every “pending” row with executable
interpreter/native/target/GC evidence or change it to a verified semantic
diagnostic. An admitted row with missing active-reference tracing is a defect,
not support.

### Additional local Linux aarch64 execution evidence

In the current working tree, the arm64 Linux container ran the complete
`cargo test -p zirk-cli --test object_engine_parity -- --test-threads=1`
suite (68 passed), plus the focused
`map_stored_union_payload_survives_forced_gc`,
`nested_union_enum_tuple_collection_keys_survive_forced_gc`, and
`managed_union_elements_survive_forced_gc_in_ordered_sequences` end-to-end
tests (one passed each). This confirms the named Map/Set and ordered-sequence
GC regressions on a second operating system. It is local container evidence,
not a hosted runner result; the five-host CI requirement remains in task 8.1.
