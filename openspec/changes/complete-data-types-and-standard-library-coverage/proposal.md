## Why

Zirk's normative contracts describe broader type and standard-library behavior than the current delivered subset, leaving users without a precise boundary for generic `Map`/`Set` keys, algebraic values, advanced generic dispatch, zone-aware time, and extended JSON. This change closes the explicitly identified gaps with one coherent capability matrix, compiler/runtime behavior, and executable evidence rather than treating source presence or a focused host test as complete coverage.

## What Changes

- Complete lawful `Hash`/`Equal` support for eligible `Map<K, V>` keys and `Set<T>` elements, including nested tuples with content-equal `String`/`Char`, stable structural value shapes, active-variant-aware unions, and nullable `Decimal`; reject unsupported or unstable forms during semantic analysis.
- Extend union/intersection lowering, runtime representation, narrowing, `match`, error propagation, cleanup, and managed-container support across an explicit alternative-family/container matrix. Keep the 4096-byte inline payload ceiling as a contract limit and diagnose unsupported layouts before backend emission.
- Complete advanced generic cases: generic enum dispatch across the target matrix, checked generic function bodies and defaults, trait default methods under substitution, and nested contract instantiations such as `Container<Box<T>>`.
- Reconcile enum `Clone` derivation with the documented component-wise rule by identifying and implementing every unsupported payload composition, with tests for nested payloads and non-`Clone` members.
- Publish a stable `std.collections` import surface backed by the existing compiler/runtime collection implementations, preserving existing built-in spellings and type identity; document the boundary between the public package and compiler-owned operations.
- Deliver zone-aware temporal values (`Instant`, `ZonedDateTime`, `TimeZone`, `Period`, `TimeShift`) and the defined locale-aware temporal formatting/parsing surface, while preserving the already delivered civil `Date`, `Time`, `DateTime`, and `Duration` behavior.
- Expand `std.json` with generated `JSONCodec<T>` implementations, bounded configurable limits, JSON Pointer, RFC 9535 JSONPath, bounded streaming reader/writer APIs, pretty and canonical output, schema validation, and a separate JSON5 parser that emits ordinary Zirk values.
- Define and verify complete evidence for native execution on supported CI hosts, object emission on all nine compiler targets, interpreter/native parity, and GC stress at managed-container boundaries. Resolve the existing `Float128` Windows coverage gap as implemented support or an explicit, tested target exclusion.
- Update normative specifications, implementation-status records, limitations, runnable examples, and the companion website to match the resulting evidence.

## Capabilities

### New Capabilities

None. The change extends existing owners for data types, collections, generics, temporal behavior, memory safety, standard-library packaging, and portability.

### Modified Capabilities

- `map-collections`: require coherent recursive hash/equality for all declared supported key/value combinations and early diagnostics for unsupported layouts.
- `set-collections`: require coherent recursive hash/equality for all declared supported element combinations and early diagnostics for unsupported layouts.
- `zirk-data-types`: specify compositional `Hash`/`Equal`, tagged union value semantics, advanced union/intersection runtime behavior, enum `Clone` derivation, and expanded JSON behavior.
- `zirk-generics`: specify generic enum dispatch, user-defined generic function bodies/defaults, substituted trait defaults, and nested generic contract instantiations.
- `zirk-memory-safety`: specify GC and clone-graph guarantees for the expanded algebraic and managed-container combinations.
- `target-matrix`: specify stage-by-stage evidence for advanced type/generic behavior and Float128 availability across the target matrix.
- `zirk-standard-library`: specify the stable importable collection package boundary and the expanded `std.json` public surface.
- `zirk-temporal-types`: replace the obsolete out-of-scope rule for zone-aware values and specify `Instant`, `ZonedDateTime`, `TimeZone`, `Period`, and `TimeShift` semantics.
- `zirk-temporal-members`: specify the locale-aware temporal operations and members introduced by this change.

## Impact

Compiler type checking and trait derivation, generic substitution and dispatch, type lowering and runtime descriptors, interpreter and LLVM execution, collection storage/hash tables, GC tracing and clone paths, standard-library package resolution, temporal runtime and bundled time/locale data, JSON parsing/serialization/query/streaming, target CI, feature-status documentation, handbook examples, and `../zirk-lang-site`. The public collection import path is additive; current built-in spellings and the identity of existing collection types remain compatible.
