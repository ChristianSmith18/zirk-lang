## Context

See `proposal.md` for motivation and scope. The repository already checks normalized unions/intersections, contextual structural literals, common members, and control-flow narrowing. A basic tagged structural union also has end-to-end CLI execution coverage, so this work must extend the existing path rather than replace it. The current implementation and status notes identify remaining gaps in structural intersection representation, structural/mixed union combinations, native payload layout and ABI coverage, and platform parity. Finite template-literal types already expand under a deterministic budget; unbounded interpolation is rejected.

The principal constraint is that semantic acceptance, portable IR, interpreter behavior, native code generation, and the package boundary must describe the same type and representation. A type accepted by checking must never reach an LLVM panic or be interpreted using a guessed layout.

### Baseline support matrix

The initial repository inspection found the following concrete state. This is the implementation baseline for the tasks; it must be revised as support is added.

| Type family | Semantic analysis | Portable IR / interpreter / native evidence | Gap for this change |
| --- | --- | --- | --- |
| Two-way discriminated structural union | Accepted; literal properties narrow alternatives | Existing CLI execution and interpreter/native parity fixtures pass | Preserve as a regression baseline; extend to N-ary, mixed, nested, and package-boundary cases where needed |
| Structural union with scalar or nominal alternatives | The union layout prototype admits shapes with the existing scalar/class families | Shapes lower as `ObjectAny`; union layout metadata handles managed references, but this exact cross-family matrix lacks complete runtime evidence | Add evidence and close any unsupported lowering, ABI, tracing, or engine-parity case found by the matrix |
| Structural `Shape & Shape` intersection | Parsed and semantically interned, then rejected by the runtime-representation gate | `Base::Shape` already lowers as `ObjectAny`; intersection lowering currently selects only class/contract representations | Allow a structural intersection to remain a view of the existing object and verify construction, member access, mutation, and identity |
| Contract/class intersections | Contract views are accepted; unrelated multiple-class intersections remain rejected | IR has an existing intersection-view path and a lowering test for contract dispatch and single allocation | Preserve working contract/class behavior; do not broaden unrelated nominal class intersections |
| N-ary normalization and relation budgets | Canonical unions/intersections and bounded relation work already exist | Semantic tests cover order, flattening, and budget behavior | Add only missing structural cases and keep deterministic no-partial-result behavior |
| Finite template-literal types | Parsed as `template:` plus interpolation type references; finite singleton expansion and budgets are implemented | Semantic tests cover string/numeric/Boolean finite expansion and rejection of broad `String` | Preserve the finite path and its existing diagnostics/budgets |
| Open template inference | Type interpolation currently requires finite string-convertible values; unresolved generic parameters fail expansion | No symbolic pattern survives to generic call inference | Preserve fixed text and generic-hole identity, invert a call-site string singleton, check constraints, then contextually type `T[K]` |
| Union package metadata | Canonical union fingerprints and compatibility metadata already exist | Existing integration coverage checks metadata rejection for exported union aliases | Reuse and extend this contract for structural layouts instead of inventing a parallel package identity system |

The initial `cargo test -p zirk-cli --test end_to_end` run exposed a failure in `generic_free_function_specializes_nested_collection_types`: IR verification reported a missing `preserve` specialization and a `List` slot type mismatch. The specialization path was corrected and its focused regression now passes. The complete CLI suite was attempted afterward but did not terminate because the unrelated bundled HTTP/WebSocket server integration test waits indefinitely for another session; this is recorded as a validation limitation, not a feature pass. The focused feature suites listed in `tasks.md` provide the evidence for the implementation claims below.

### Operation-level characterization matrix

This matrix separates implemented paths from evidence gathered so far. “Supported” means there is a source path and an execution or verifier fixture; “partial” means some representative paths work but the full combination is not yet characterized; “rejected” means the frontend reports a source-located diagnostic before backend emission.

| Operation | Structural union with scalar/shape alternatives | Shape intersection | Evidence / remaining work |
| --- | --- | --- | --- |
| Contextual construction | Supported for a discriminated shape alternative and a scalar alternative | Supported for a literal satisfying both shapes | `end_to_end::structural_union_discriminants_narrow_before_field_reads`, `end_to_end::structural_union_mixed_with_scalar_preserves_tag_and_managed_payload`, `end_to_end::structural_intersection_object_literal_keeps_one_object_identity` |
| Assignment and local storage | Supported for three structural alternatives and for a shape/scalar union; all nested and nullable combinations remain partial | Supported for aliasing the same object through the intersection view | `structural_union_three_alternatives_narrow_across_else_if`, `structural_union_mixed_with_scalar_preserves_tag_and_managed_payload`, `structural_intersection_object_literal_keeps_one_object_identity` |
| Property reads | Supported after discriminant narrowing for shape alternatives | Supported for properties contributed by either shape | `end_to_end::structural_union_discriminants_narrow_before_field_reads`, `end_to_end::structural_intersection_object_literal_keeps_one_object_identity` |
| Property writes | Partial; shape mutation exists but the union matrix needs readonly/optional and narrowing cases | Supported for mutable properties through the view; overlapping modifier conflicts still need semantic tests | Intersection execution fixture mutates `same.id` and observes the change through `person` |
| Narrowing and match | Supported for two- and three-way discriminated shapes; shape/scalar match has native and interpreter parity. Broader nested and exhaustiveness combinations remain partial | Not applicable to selecting one intersection constituent; contract-view dispatch remains supported separately | `end_to_end::structural_union_discriminants_narrow_before_field_reads`, `end_to_end::structural_union_three_alternatives_narrow_across_else_if`, `object_engine_parity::structural_union_with_scalar_alternative_matches_across_engines` |
| Function arguments, returns, and calls | Supported for direct shape/scalar call-and-return, and a structural/scalar union crosses an external package call; arbitrary nested and captured generic combinations remain partial | Partial; shape view is passed to `describe`; callable common members are not characterized | `object_engine_parity::structural_union_with_scalar_alternative_matches_across_engines`, `end_to_end::compiled_package_union_metadata_is_checked_before_link`; nested generic-list specialization regression now passes independently |
| Captures and closures | Supported for a captured shape/scalar union in interpreter/native parity and for a managed structural payload under forced native GC; broader capture graphs remain partial | No dedicated intersection-capture fixture yet | `object_engine_parity::captured_structural_union_payload_matches_across_engines`, `end_to_end::captured_structural_union_payload_survives_forced_gc` |
| IR verification | Tagged union operations and layouts are verified; shape/scalar call-and-match and three-way subset narrowing have native fixtures | Shape intersection lowers to `ObjectAny` and verifies as a view | `cargo test -p zirk-ir`; focused structural CLI fixtures in `tasks.md` |
| Managed-reference tracing | Tag-aware union descriptors are tested, and structural payload survival is verified under forced collection; other nested/aggregate combinations remain partial | A view keeps the original managed object reference; no second allocation | `end_to_end::captured_structural_union_payload_survives_forced_gc`, `zirk-runtime::collector` active-reference tests, and intersection identity fixture |
| Target ABI | Host-native execution passes for tested cases. A Windows cross-check was attempted but the macOS environment lacks Windows SDK headers; no target-matrix evidence exists for this change yet | Host-native object-reference ABI passes; Windows evidence remains outstanding | Run focused target CI under task 5.6; do not interpret the local cross-check failure as a source-level Windows result |
| Package boundary | A structural/scalar union alias crosses a compiled package call, and stale union fingerprint metadata is rejected before object emission; broader generic and intersection package signatures remain partial | Not yet characterized across a package boundary | `end_to_end::compiled_package_union_metadata_is_checked_before_link` |
| Unsupported representation | `Shape | List<Int32>` is rejected with E0423 at the type alias before lowering | Intersections outside the existing nominal/contract rules remain rejected | `check::unsupported_structural_mixed_union_is_rejected_at_its_type_declaration` |

The mixed-union match fixture exposed and fixed a backend control-flow defect: after the final literal comparison, the false branch already targeted the fallback block, but lowering emitted an additional jump from that block to itself. The no-match route now jumps directly to fallback only when that alternative has no literal patterns. Native execution and the rebuilt interpreter harness both produce `object` for the structural alternative and `number` for `42`.

A three-alternative discriminated union then exposed a separate lowering mismatch: flow analysis narrowed the union to a subset, while the loaded value still used the original tag layout. Lowering now projects each retained source alternative into the canonical narrowed-union representation and marks excluded paths unreachable. Native and interpreter parity, including a chain of `else if` branches, passes for three distinct structural alternatives.

## Goals / Non-Goals

**Goals:**

- Close the implementation matrix for structural unions/intersections from checking through IR verification, interpreter, and native code generation.
- Keep structural intersections as static views of the same underlying value.
- Add open template-pattern inference for the event-name-to-property-type use case while preserving finite template expansion.
- Keep inference deterministic, bounded, erased at runtime, and consistent between CLI and LSP.
- Make target and package representation compatibility explicit and verifiable.

**Non-Goals:**

- Implement function overloads, user-declared predicates, or a broad TypeScript compatibility mode.
- Add dynamic runtime pattern matching for strings or runtime reflection of erased generic types.
- Change the language's object identity, string equality, literal syntax, permission model, or general package format beyond what is required to validate composite layout compatibility.

## Decisions

### 1. Establish an explicit structural-composite support matrix before extending code generation

Inventory combinations already accepted by semantic analysis and classify them by operation and representation: scalar, enum/value, class reference, structural shape/reference, nested union, nullable, generic instantiation, and mixed alternatives. Preserve existing executable cases as baseline fixtures. For each combination, either supply a verified execution path on a supported target or make checking/lowering reject it with a source-located diagnostic. This avoids treating a test for a basic tagged shape union as evidence that every composite ABI combination is complete.

### 2. Keep canonical type semantics separate from runtime representation

Normalize unions/intersections and decide assignability, contextual construction, property capabilities, and narrowing in semantic analysis. Carry the canonical alternative identity into IR wherever execution needs tag dispatch. Do not infer semantic relationships from LLVM layout or treat a shared physical representation as proof that two source alternatives are interchangeable. Use the existing deterministic relation and type-computation budgets; return no partial result when a limit is exceeded.

For structural intersections, retain one payload/reference and combine only the checked static member views. When intersected properties conflict, report the precise structural mismatch; do not silently select one constituent or create an implicit overloaded callable.

### 3. Give portable IR explicit checked composite operations

Use typed injection, tag inspection, projection, and coercion operations that the verifier can validate against canonical alternatives. Ensure these operations cover calls, returns, slots, captures, pattern branches, and managed-reference metadata. The interpreter and LLVM backend consume this same verified contract. A malformed tag/payload relation fails verification before either engine runs.

### 4. Choose native layout from representation and target ABI, not source syntax

Use inline payloads only when size, alignment, tracing, and the target calling convention are safe. Use a managed/boxed representation for large or irregular structural payloads where needed. Root metadata must follow the active alternative so inactive union bytes are not traced as references. Package-facing calls must validate the canonical union and representation metadata; incompatible package artifacts fail during load/link rather than corrupting a call. Keep the physical layout private to generated code and the package contract.

Alternatives considered: force every structural union into a large inline buffer, which wastes space and risks ABI limits; reject all mixed cases, which leaves accepted semantic combinations incomplete; or use a safe fallback only where inline layout is unsuitable. The last option preserves performance for simple cases and semantics for irregular ones.

### 5. Represent open templates symbolically and infer from string singletons

Retain template structure as fixed text segments plus typed interpolation references instead of expanding an open interpolation into all possible strings. Version one supports exactly one inference-bearing hole with a fixed prefix and/or suffix; finite interpolations continue using the current expansion path. At a generic function or method call, gather receiver and ordinary argument evidence first. For a method on a parameterized contract, substitute the receiver's contract arguments before inferring method-local parameters. Match the string singleton against fixed text, bind the unique substring as the hole's singleton, validate its constraint, resolve indexed access such as `T[K]`, then use that result for contextual callback typing.

Do not infer an arbitrary key from a broad `String` value. Multiple open holes, ambiguous decompositions, or mismatched text produce diagnostics. This bounded v1 directly supports `"{K}Changed"` without requiring regex-like matching, search over the key universe, or overload resolution.

### 7. Keep checker and editor inference on one semantic path

Expose resolved generic substitutions and structural alternatives through the existing semantic analysis result consumed by LSP features. LSP hover, completion, and diagnostics must not reimplement template matching or structural assignability. Open-template types are erased and add no runtime representation.

### 6. Preserve method-local generics on parameterized contract members

The public event API is a generic contract method (`Watched<T>.on<K ...>`), not only a free generic function. Retain method type parameters and constraints in `ContractMethod`, substitute the receiver's contract arguments before inferring method arguments, and run indexed access/callback contextual typing on the composed substitution. Validate that implementing classes provide a compatible generic method. At a dynamic call site, monomorphize the method implementation for the inferred compile-time arguments and dispatch through the existing method slot; do not reify `K` at runtime. Include package metadata/fingerprinting for exported generic contract signatures and reject incompatible callers before native emission. Keep first delivery limited to ordinary method calls with one uniquely inferred open-template hole; generic method values, overloads, and higher-rank polymorphism remain out of scope.

### 8. Ship in dependency order and synchronize public documentation last

First fix behavior/specifications and the accepted-representation matrix; then complete type relations and IR; then interpreter/native parity and package ABI; then open-template parsing/inference and LSP; finally update handbook, feature status, limitations, roadmap, demos, and website content. The website import must run only after the relevant Zirk documents are committed and the site diff is reviewed, per repository instructions.

## Risks / Trade-offs

- **[Structural type acceptance can outrun backend support]** → Gate each source combination on an explicit matrix and require lowering diagnostics before code generation until that matrix entry is supported.
- **[Managed payload tracing can misread inactive union storage]** → Make root enumeration tag-aware and test mixed scalar/reference payloads through allocation and collection.
- **[Package ABI layout changes can break compiled artifacts]** → Version or fingerprint the relevant representation metadata and reject incompatible package artifacts before execution.
- **[Boxed fallback can affect allocation and performance]** → Limit fallback to layouts that cannot safely use inline storage, document its conditions, and include performance evidence for representative shapes.
- **[Generic inference can become expensive or ambiguous]** → Limit open patterns to one inference hole in v1, reuse canonical relation memoization, impose deterministic budgets, and never publish a partial substitution.
- **[LSP behavior may diverge from CLI]** → Consume shared checker results and keep editor fixtures for both successful and failing inference.

## Migration Plan

Existing finite template types and already working structural-union programs keep their source syntax and observable behavior. Newly supported structural combinations become executable after their target representation is verified. Cases that are semantically accepted but cannot yet be represented must instead fail before backend emission with a stable source diagnostic. If package layout metadata changes, the package format/version check must reject stale incompatible artifacts and provide a rebuild diagnostic. Since open templates erase at compile time, they require no runtime migration.

Public status must distinguish semantic support from execution support until the full matrix has passed the required target and interpreter/native validations. Update the Zirk handbook/status documents first, then commit them, run `./scripts/sync-website-content.sh`, review the website diff and status catalog, and validate both repositories before marking the work complete.
