## Why

Zirk's accepted type surface and its executable runtime are not yet aligned: Windows cannot reliably link and run `Float128`, formatting narrows it to `Float64`, composite unions and intersections have incomplete lowering/runtime coverage, and `Map`/`Set` still depend on a one-word representation despite their generic contracts. The collection contract also names `Deque`, `PriorityQueue`, `Queue`, and `Stack`, but these are not available as Zirk APIs. This change closes those gaps under one verifiable cross-engine and cross-target contract.

## What Changes

- Complete IEEE binary128 execution on all supported native targets, including Windows x86_64, and provide a precision-preserving `Float128.to_string()`/`format()` path. Arithmetic or conversion that cannot be implemented on a supported target must not silently produce a different result.
- Make every type admitted by `Map<K, V>` and `Set<T>` usable through verified generic storage. Map keys and set elements must satisfy the language's `Hash` and `Equal` capabilities; map values may use any storable first-class Zirk value. Add layout-aware storage and GC tracing for aggregate, nullable, enum, union, and managed-reference payloads instead of limiting entries to one machine word.
- Complete the documented `Map`/`Set` operations, including deterministic insertion-order traversal, set algebra, and key/value/entry iteration. Resolve API discrepancies consistently: collection `length` is `UInt64`, `Map.set` returns `Void`, and `Set.add` returns whether insertion occurred.
- Add Zirk-visible `Deque<T>`, `PriorityQueue<T>`, FIFO-restricted `Queue<T>`, and LIFO-restricted `Stack<T>`, with explicit contracts for ordering, empty outcomes, mutation, iteration, allocation failures, generic storage, and complexity.
- Close the accepted union/intersection execution matrix from semantic checking through verified IR, interpreter, native code generation, GC, and compiled-package boundaries. Every representation admitted to an executable program must work through construction, storage, calls, returns, captures, narrowing, matching, and collection; unsupported or over-limit forms must receive stable source diagnostics before backend emission. The initial inline union payload ceiling remains 4096 bytes unless separately changed with safety and performance evidence.
- Update the feature status, limitations, roadmap, handbook, examples, and the companion `../zirk-lang-site` so published implementation claims match the delivered support matrix.

## Capabilities

### New Capabilities

None. The change completes existing language and standard-library capabilities.

### Modified Capabilities

- `map-collections`: generic key/value eligibility, `Hash`/`Equal`, layout-aware storage, iteration, deterministic insertion order, and managed reachability.
- `set-collections`: generic hashable elements, Boolean insertion result, algebraic operations, iteration, insertion order, and managed reachability.
- `zirk-errors`: catchable typed empty-collection and allocation failures with no partial collection mutation.
- `zirk-collections`: consistent `UInt64` lengths, collection order/copy rules, and requirements for `Deque`, `PriorityQueue`, `Queue`, and `Stack`.
- `zirk-scalars`: cross-target `Float128` semantics and precision-preserving formatting.
- `zirk-data-types`: the accepted runtime representation boundary for composite unions and intersections.
- `zirk-ir-lowering`: complete checked representation paths and early rejection for unsupported composites and generic collection layouts.
- `zirk-ir-interpreter`: interpreter execution parity for every accepted composite family and generic collection value layout.
- `zirk-native-codegen`: target-correct `Float128`, collection payload layouts, and union/intersection ABI behavior.
- `zirk-memory-safety`: GC tracing and copy/move guarantees for generic collection entries and composite payloads.
- `target-matrix`: runtime verification of `Float128` on every native build platform, including Windows x86_64.

## Impact

This affects `zirk-sema`, portable IR and its verifier, the interpreter, LLVM code generation, `zirk-runtime`/`zirk-runtime-core`, standard-library declarations, native target CI, compiler and runtime conformance tests, and public documentation. It introduces no required external dependency and no new source syntax. Public status and documentation changes also affect the sibling `../zirk-lang-site`; synchronize it from the committed source revision and review its generated status catalog before declaring the work complete.
