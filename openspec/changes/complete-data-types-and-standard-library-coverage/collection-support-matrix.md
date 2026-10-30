# Collection Capability Matrix

This is the source-audited collection contract for task 1.1. It describes the
current checker/runtime behavior after recursive Map/Set union-root tracing. It is
not a claim that every admitted row is already lawful; rows marked incomplete
are gaps to close before they can be part of the final supported matrix.

## Current checker admission

| Family | `Map` key / `Set` element | `Map` value | Current representation and behavior | Current gap |
|---|---|---|---|---|
| Integer and binary float scalars, including `Float128` | Admitted | Admitted | Typed byte slots; no managed references. Before native insertion and lookup, codegen canonicalizes signed zero and all NaN bit patterns to one quiet NaN per width; infinities retain their sign. The interpreter applies the same key equivalence. | Parity covers signed zero, infinities, Float128, and native key normalization. Source arithmetic raises `FloatNanError` before producing NaN, so NaN-key behavior is currently reachable only through an injected/runtime value; direct native/interpreter NaN-key parity and generated-bit-pattern properties remain unverified. |
| `Decimal` | Admitted | Admitted | Key/value bytes use the canonical decimal value layout; no managed references. Runtime constructors and arithmetic normalize trailing decimal zeroes. Native/interpreter parity covers `1.0`/`1.00`, the maximum 38-digit coefficient, and the minimum supported fractional scale; Map lookup and Set deduplication agree for equivalent values. | Property coverage generated from arithmetic results remains open under 1.2 and 2.9. |
| `Boolean`, `Duration`, `Date`, `Time`, `DateTime` | Admitted | Admitted | Typed scalar bytes; no managed references. `DateTime` uses canonical epoch nanoseconds. | Confirm byte equality is equivalent to each public equality rule on boundary values. |
| Direct `String` and `Char` | Admitted | Admitted | Key/element uses an owned normalized text snapshot; value uses its managed handle and GC offset. | None identified for the direct case; preserve Unicode normalization and snapshot behavior. |
| Nullable hashable types, including `Decimal?` | Admitted recursively | Admitted when the base type is in the storage-value allowlist | Presence flag plus payload; direct nullable text uses a special text-key path. Nullable Decimal now has an explicit IR representation and canonical Decimal payload; parity tests cover None/Some distinction, equivalent Decimal scales, Set deduplication, and Map lookup in both engines. Ordinary values carry discovered reference offsets. | Recursive nullable text is admitted inside tuples and enum payloads and has focused native/interpreter parity. Other nested presence/layout combinations still need generated coverage. |
| Tuples | Admitted when every field has a recursive capability, including `String`, `Char`, and nullable text fields | Admitted when a tuple layout exists | Inline aggregate bytes; codegen zeroes padding, canonicalizes nested floating values, and emits recursive text snapshot metadata. GC reference offsets and nested union roots are derived from IR layout. Interpreter equality recursively compares fields and snapshots text on collection boundaries. | Focused parity covers nested String/Char and nullable String tuples, mutation snapshots, Decimal?, and nested enum/union combinations. Exhaustive generated nested-layout and target coverage remain open. |
| Payload enums | Admitted when all associated fields have a recursive capability after substituting generic arguments | Admitted when an enum layout exists | Inline discriminant plus flattened payload; codegen canonicalizes the active payload and emits enum-guarded union-root and text-snapshot metadata. Runtime snapshots only the active variant's text fields; the interpreter compares and snapshots active fields recursively. | Direct String/Char enum payload keys/elements now have native/interpreter parity for active tags, NFC-equivalent values, mutation snapshots, deduplication, and Map lookup. Generated deeper composition and target coverage remain open. |
| Union and intersection | Union keys/elements are admitted when every alternative has a recursive capability; intersections remain rejected as keys/elements | Admitted when the semantic type exists | Union keys/elements use an inline active tag and recursively canonicalized payload. Map and Set constructors receive tag-aware union-root records, reference offsets, and text snapshot metadata. Map values support direct, nested, nullable, and enum-guarded union roots; Set elements trace active managed union payloads too. | Focused parity and forced-GC tests cover scalar/text/class alternatives, tuple and enum nesting, nullable unions, Map/Set placements, and ordered sequences. Intersections are Map-storable but not hashable. Exhaustive placements, GC boundaries, and cross-target evidence remain open. |
| Class references | Admitted for ordinary mutable classes, including generic class instances | Admitted for ordinary mutable classes and generic instances | Reference-sized handle; key semantics are stable object identity; generic type arguments affect static layout but do not participate in equality/hash; GC reference is traced. | Must keep identity semantics explicit and test lifetime under collection. |
| Contract references | Admitted | Admitted | Reference-sized handle; intended identity follows the underlying object. | Verify all contract instance forms have identical identity and tracing rules. |
| `Shape`, `Object`, `Dynamic`, arrays, lists, maps, sets | Rejected as keys/elements | Admitted where the checker allowlist permits | Runtime handle or layout-specific value; managed references are found by IR layout. | Mutable/dynamic shapes do not have derived stable structural Hash/Equal; aggregate value layout support needs full matrix tests. |
| `Atomic` | Rejected | Admitted | Managed handle with scalar payload; handle is traced. | Explicitly not hashable as a key/element under the current contract. |
| `Instance`, `ContractInstance`, `EnumInstance` | Generic mutable class instances use class identity; generic enum instances are admitted only when all substituted active payload field types are recursively hashable. Generic contract instances remain rejected unless covered by the contract-reference identity rule. | Admitted by the storage allowlist | Lowered according to resolved instance type; layout/tracing depends on the materialized type. | Verify every substituted layout and ABI composition; nested text snapshots and generic union payloads remain outside current Hash/Equal coverage. |
| `Regex`, `Range`, closures, pointers/slices, mutexes, channels, tasks, resources, unsupported type forms | Rejected | Rejected unless an explicit storage rule is added | No general collection key/value contract. | Keep rejected with a source diagnostic until storage, equality, ABI, ownership, and tracing are defined. |

## Runtime and ABI evidence checked

- `crates/zirk-sema/src/checker.rs`: `collection_hash_eq` recursively builds
  scalar, nullable, tuple, enum, union, text, and identity capabilities;
  `is_map_storage_value` independently gates Map value representations.
  Nullable Decimal, direct union keys/elements, tuples with nested text fields,
  and generic mutable class instances are admitted. Enum payload text,
  intersections, mutable Object, and other capability-less forms remain
  rejected with source diagnostics.
- `crates/zirk-runtime/src/map.rs` and `set.rs`: ordinary keys are stored in
  `OrderedMap<Vec<u8>, _>` and therefore compared/hashed as bytes; direct text
  keys/elements use a separate normalized, owned `Box<str>` snapshot.
- `crates/zirk-codegen-llvm/src/emit.rs`: Map/Set constructors receive
  type-specific sizes, direct-text flags, GC reference offsets, text snapshot
  descriptors, and nested union-root metadata derived from IR layouts.
  `store_canonical_collection_value` zeroes aggregate padding and recursively
  canonicalizes signed zero and NaN before native collection operations. Both
  Map and Set copy active-tag descriptors, nullable presence offsets, and
  enum-variant guards into their runtime bodies.
- `crates/zirk-ir/src/layout.rs` and `crates/zirk-ir/src/unions.rs`: portable
  inline layouts and reference paths exist, but do not by themselves prove
  collection equality, payload canonicalization, or native/interpreter parity.
- `crates/zirk-interpreter/src/lib.rs`: interpreter collection operations use
  the recursive collection capability and `runtime_value_equals`; native keys
  use canonical bytes plus text snapshots. Focused parity exists, but generated
  tests must continue checking both representations for every admitted
  recursive composition.

The table includes every family currently returned as hashable by the recursive
`collection_hash_eq` capability builder and every family admitted as a Map
storage value, plus the relevant `IrType` layout and GC-path behavior. It separates checker admission from verified capability so
an admitted-but-untraced union value is visible as a defect rather than
mistaken for supported behavior.

## Final acceptance rule

The final matrix must be generated from, or checked against, one recursive
capability description shared by semantic eligibility, interpreter equality,
native key hashing/equality, typed storage, and GC tracing. Every row must
record independent key/element and value eligibility. A rejected row must
produce a semantic diagnostic before IR or backend emission, including the
nested type path. This baseline is not the final acceptance matrix until those
properties and the positive/negative corpus pass.

## Proposed lawful key and element semantics

This section freezes the semantic contract for tasks 1.2 and 2.1–2.5. It does
not claim that the currently admitted native byte representation already
implements every rule below.

| Type family | Collection equality | Hash input | Mutation and eligibility |
|---|---|---|---|
| Integer, Boolean, temporal scalars | Same concrete type and same language value | Canonical typed scalar encoding | Immutable value snapshot. `DateTime` uses its canonical epoch-nanosecond value. |
| Binary floats | Same width and numerically equal value; `+0` equals `-0`; all NaN payloads of one width form one collection-equivalence class; infinities compare by sign | Width-tagged canonical bits, with both zeros mapped to zero and each width's NaNs mapped to one canonical NaN | Immutable value snapshot. This is a total collection equivalence and intentionally differs from ordinary IEEE `==` for NaN. |
| Decimal | Same mathematical decimal value regardless of representational scale; signed zeros compare equal | Canonical sign/coefficient/scale after removing trailing coefficient zeroes; zero has one encoding | Immutable value snapshot. `Decimal?` adds the standard presence flag and uses the Decimal rule only for `Some`. |
| String and Char | String uses Zirk text equality (canonical NFC equivalence); Char uses its scalar value | NFC UTF-8 bytes for String; scalar value for Char, within the statically typed collection | Capture an owned key/element snapshot on insertion. Subsequent mutation or rebinding of the source cannot change lookup/removal. Nested tuple fields use the same rule, not pointer identity. |
| Nullable values | `None == None`; `None != Some(_)`; two `Some` values compare recursively | Presence tag followed by the recursive payload hash for `Some` | Snapshot the full recursive key. `Decimal?` is now represented and admitted. |
| Tuples and immutable structural values | The collection’s statically fixed tuple layout and recursively equal corresponding fields | Ordered canonical field values under that fixed layout | Value snapshot. Mutable `Object`/shape and mutable class structure do not derive structural Hash/Equal. |
| Payload enums and unions | The collection’s statically fixed enum/union layout, active variant/tag, and recursively equal active payload | Active tag and only the active payload under that fixed layout | Snapshot the active value. Inactive storage and padding never affect equality or hashing. Unsupported payloads remain rejected before lowering. |
| Mutable class and contract references | Underlying object identity, independent of mutable fields and contract view | Stable object identity token | The reference itself is retained and traced; mutation of the object cannot change its key. |

For every admitted type, equality must be an equivalence relation and
`Equal(a, b)` must imply `Hash(a) == Hash(b)`. The following current tests are
positive evidence only for the named rows: `map::tests::canonically_equivalent_text_keys_share_one_hash_entry`,
`set::tests::canonically_equivalent_text_elements_share_one_hash_entry`, and
`object_engine_parity::canonically_equivalent_text_collection_keys_match_native_and_interpreter`
prove direct NFC equality/hash behavior at runtime and through both engines;
the existing text snapshot runtime cases in `map.rs`/`set.rs` cover mutation.
Decimal scale/boundary equivalence and nullable Decimal cases in native and
interpreter collection tests; float signed-zero normalization in native
collection tests and NaN equivalence in interpreter tests; and class identity
and managed-lifetime cases in collection tests provide evidence for their rows.
Early diagnostics cover mutable structural values, enum payload fields without
a recursive capability, unsupported Map value layouts, and unsupported
key/element families. Focused positive cases cover nested tuple text keys,
active-variant enum text snapshots, and union key/element tags. The
remaining exhaustive recursive compositions are tracked in tasks 2.8 and 2.9,
and target-specific proofs remain in section 8. Task 1.2 closes the documented
semantics and focused positive/negative/coherence corpus; those broader matrix
proofs are separate acceptance work.

## Focused verification recorded during implementation

The following focused commands passed on the current macOS arm64 worktree:

- `cargo test -p zirk-cli --test object_engine_parity decimal_collection_keys_match_native_and_interpreter -- --test-threads=1`
- `cargo test -p zirk-sema invalid_collection_keys_reject_unsupported_aggregate_types -- --test-threads=1`
- `cargo test -p zirk-runtime map_ -- --test-threads=1` (12 passed)

These checks establish Decimal scale and boundary equivalence, nullable Decimal
Map/Set parity, and semantic rejection of unsupported collection families.
Task 1.2 also records the generated Decimal arithmetic parity and 512-case
equality-implies-equal-hash commands. Cross-target evidence remains in section 8.
