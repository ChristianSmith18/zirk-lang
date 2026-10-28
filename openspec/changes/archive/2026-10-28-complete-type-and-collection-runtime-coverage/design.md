## Context

See `proposal.md` for the delivery gaps and `specs/` for the observable contracts. The current compiler admits collection and composite types more broadly than the runtime can represent. `Map`/`Set` storage is effectively one word per entry, native union coverage varies by alternative family, intersection views are incomplete, and Windows `Float128` helper/link and formatting paths are not reliable. The public collection contract also names sequence families that have no Zirk-facing implementation.

The implementation spans semantic analysis, portable IR and verification, the interpreter, native code generation, managed-reference tracing, runtime APIs, package ABI metadata, CI, and public documentation. Preserve target evidence distinctions: emitting a cross-target object does not prove that its runtime can execute.

## Goals / Non-Goals

**Goals:**

- Use one canonical, recursively composable value-layout description for generic collection entries and composite union payloads.
- Define `Hash`/`Equal` eligibility and stable ordering independently of hash-table bucket order.
- Add the four collection APIs with explicit ordering, empty behavior, allocation behavior, and generic storage rules.
- Keep `Float128` binary128 from lowering through formatting and linking on every supported native build platform.
- Make source acceptance, verified IR, interpreter execution, native code generation, GC tracing, and package metadata agree on the supported composite matrix.
- Make test evidence and public feature status traceable to this matrix.

**Non-Goals:**

- Add source syntax for generics, unions, intersections, collection literals, or operator overloading.
- Change `Float128` to a different precision or add an implicit conversion to `Decimal`.
- Replace the 4096-byte inline union payload ceiling without measured safety and performance evidence; oversized values may receive a diagnostic or use a verified managed fallback.
- Add unordered public Map/Set variants, a configurable priority comparator API, or a second intersection object representation.
- Implement unrelated standard-library gaps or claim cross-target runtime execution where only object emission is available.

## Decisions

### 1. Use a recursive runtime layout descriptor as the shared contract

Each accepted runtime type will have a canonical layout description that records its storage size/alignment, scalar encoding, nested field offsets, managed-reference edges, and tagged payload alternatives. Semantic analysis uses the same classification to decide whether an operation is admissible; portable IR carries the canonical type/layout identity; the verifier checks typed operations against it; interpreter and native codegen consume it for storage and tracing. The descriptor must not turn into a dynamic type descriptor for ordinary generic dispatch: concrete generic arguments remain compile-time substitutions, while runtime metadata is emitted only where an aggregate or active union tag requires it.

Alternatives considered: keep one-word entries and grow the semantic whitelist incrementally; this preserves the current mismatch and repeats ad-hoc checks. Boxing every entry would simplify storage but add allocation and GC overhead to scalar-only collections. A recursive descriptor plus inline storage where safe and managed fallback for large composites gives one correctness model without forcing boxing for every value.

### 2. Separate hash indexing from observable insertion order

Map and Set will keep a hash index for expected constant-time lookup and an insertion-ordered node arena with stable node identifiers and previous/next links. This supports expected constant-time deletion without making iteration scan an unbounded tombstone tail. Overwriting an existing map key retains its position; deletion followed by reinsertion appends it. Set algebra creates a new result and follows the left/right order rules in the spec. Rendering remains deterministic under the existing renderer contract; iteration order is insertion order and does not depend on the per-process hash seed.

`Hash` and `Equal` remain the eligibility contracts. Built-in immutable values, nullable values with a defined runtime representation, tuples, and closed enums can derive coherent hashing/equality recursively when every participating field has a canonical stored representation for both operations. Direct text keys/elements keep the existing normalized snapshot behavior; nested text fields remain ineligible until aggregate comparison can use that content-based contract rather than pointer bytes. Reference objects use identity unless they explicitly implement stable `Hash` and `Equal`; such an implementation must preserve equal-values-have-equal-hashes and stable lookup behavior while stored. A list or mutable aggregate does not become hashable merely because it can be stored as a map value.

Alternatives considered: expose hash iteration order, which would make output vary with seed and runtime layout; sort every traversal, which requires an ordering for arbitrary keys and loses insertion semantics; keep both orders public, which duplicates the collection API. The ordered entry sequence meets the existing standard-library contract without requiring keys to implement ordering.

### 3. Implement missing collection families with restricted wrappers

Deque uses a growable ring buffer with typed end operations. Queue and Stack expose only FIFO and LIFO methods over private sequence storage, so their source APIs cannot bypass those constraints. PriorityQueue uses stable minimum-first ordering over a total natural order; equal-priority items use a monotonically increasing insertion sequence as the tie-breaker. It does not accept an arbitrary runtime comparator in this change, avoiding a new closure/ordering ABI surface.

Empty removals and inspections return `Result<T, EmptyCollectionError>` so `T?` remains an ordinary element value. Capacity growth uses fallible allocation and raises catchable `AllocationError` before mutation; it never drops an item or leaves a partially updated structure. Collection constructors infer generic arguments from context as `Map()` and `Set()` do. Sequence iteration follows the order stated in each spec; structural mutation invalidates existing iterators.

Alternatives considered: return `null` for empty pop/peek, which is ambiguous when `T` is nullable; let Queue/Stack alias a public Deque, which permits operations outside their contracts; or use unstable equal-priority ordering, which makes identical programs observably different.

### 4. Keep binary128 values intact and provide target-compatible runtime support

`Float128` remains an LLVM `fp128` value through typed IR and native arithmetic wherever the target supports it. The target support layer must resolve every helper emitted by LLVM to an ABI-compatible implementation. Where the configured toolchain has no suitable binary128 builtin, the Zirk runtime supplies the required software helper implementation and includes it in native linking. Formatting uses a binary128-aware decimal conversion path and never calls a binary64 formatter as an intermediate. The default text form must round-trip; explicit format precision is applied to the binary128 value.

Alternatives considered: depend on a platform-specific quadmath library, which is not a portable Windows/MSVC contract; silently convert to binary64, which loses values; or reject `Float128` on Windows, which leaves the declared type unusable on a supported platform. Runtime helper resolution and symbol-level link checks make the dependency explicit while preserving source semantics.

### 5. Make composite admission and runtime support one matrix

The initial executable union matrix covers the runtime families in `zirk-data-types`: numeric scalars (including `Float128` and `Decimal`), text and temporal values, dataless and payload enums, tuples/records, structural values, nominal references, collection references, nullable alternatives, and nested normalized unions. Each supported family must exercise construction, assignment, parameters, returns, local and aggregate storage, capture, narrowing, matching, copying, managed tracing, and package calls where applicable. Any family or operation not represented by a valid path must be rejected with a source-located diagnostic before native emission. The 4096-byte inline payload ceiling stays in force; over-limit layouts require a safe boxed path or a deterministic diagnostic.

Intersections stay static views. Compatible structural shapes and contracts combine their checked member sets over the original object; a compatible nominal class may participate when one concrete class representation suffices. Contradictory member constraints or unsatisfiable nominal combinations fail during semantic analysis. No runtime object is allocated to represent the intersection itself.

Alternatives considered: accept all type algebra and defer failures to a backend, which produces stage-dependent errors; or restrict support to the current prototype family without making that subset explicit, which leaves the documented contract ambiguous. A canonical matrix makes both accepted behavior and intentional diagnostics reviewable.

### 6. Fingerprint representation-affecting package metadata

Exported signatures containing generic collection or composite types must identify canonical type arguments, layout version, union alternative ordering/tag mapping, and relevant calling-convention metadata. A caller/callee mismatch is rejected before a native call is emitted. Runtime behavior changes that alter those fingerprints require a package ABI metadata version bump; source-level collection method additions that do not alter representation do not independently force a package format change.

Alternatives considered: trust source recompilation to keep both package sides aligned, which fails for cached compiled packages; or fingerprint target-specific layout only, which risks cross-target confusion. Canonical source-level identity plus target-specific ABI metadata is checked at the boundary.

### 7. Verify the cross product by layout class, operation, engine, and target

Use a data-driven conformance matrix with layout classes (single-word scalar, multiword scalar, nullable, tuple/record, payload enum, union, managed reference, and nested reference-bearing aggregate), valid key/element capability combinations, and value combinations for Map. Exercise each public operation and lifecycle boundary in both interpreter and native execution. For unions/intersections, cover every admitted alternative family and the operations listed above. Run native execution on supported CI hosts; run object-emission and ABI checks for cross-target triples, labeling those results separately. Windows x86_64 must link and run `Float128`, not just compile its source.

The matrix avoids an unbounded source-type Cartesian product while checking every distinct representation and dispatch path. Any uncovered representation class must be added before the feature is marked complete.

## Risks / Trade-offs

- [Risk] Layout descriptors and ordered hash collections increase runtime metadata and memory use → Keep primitive-only entries inline, benchmark realistic scalar and aggregate cases, and use fallible allocation.
- [Risk] User-defined `Hash`/`Equal` can violate coherence or stability → Require explicit capability contracts, derive only from eligible fields, define reference identity as the default, and test mutation-sensitive key cases.
- [Risk] Windows binary128 ABI details differ from other targets → Validate helper symbols and calling conventions in CI and never use a narrowing fallback.
- [Risk] New collection families and errors expand the public API → Publish exact signatures, ordering, typed failures, and complexity before marking delivery status complete.
- [Risk] Existing compiled packages may carry stale layout assumptions → Version representation metadata and reject stale artifacts with an actionable package diagnostic.
- [Risk] Public documentation may claim more than CI proves → Track native execution separately from cross-object emission and synchronize/review the companion website status catalog.

## Migration Plan

1. Land runtime/layout and type-checking changes behind the existing supported-feature reporting until each representation matrix row passes.
2. Add `AllocationError` and `EmptyCollectionError`, then implement collection storage and APIs against those error contracts.
3. Enable the full composite acceptance matrix only after IR, interpreter, native, GC, and package-boundary paths are available; unsupported rows remain source diagnostics.
4. Update handbook signatures and examples, including `UInt64` lengths, `Map.set: Void`, `Set.add: Boolean`, ordered traversal, and the new family APIs.
5. Update feature status, limitations, pipeline, and roadmap only from verified evidence. After the zirk-lang source changes are committed, run `./scripts/sync-website-content.sh`, review the site-owned status catalog with an explicit audit date, and validate the site diff.
6. Rollback may disable a not-yet-reported semantic acceptance row or revert an implementation commit. Package layout metadata changes require a version bump; old incompatible package metadata is rejected rather than loaded optimistically.

## Open Questions

None. The ordering, empty-result, allocation-failure, supported intersection, and over-limit union policies are fixed by this design and the delta specs.
