## Why

The current compiler accepts several safe programs whose managed references can be mis-traced, reclaimed, or interpreted through an invalid runtime representation; some boundary numeric operations also diverge between native code and the interpreter. These gaps undermine Zirk's memory-safety and deterministic-failure contracts, so correctness gates must precede deeper optimization of the existing type system and runtime.

## What Changes

- Make every GC root and every reference-bearing value use a representation and descriptor the collector can safely identify. Cover `Map`, `Set`, `Array`, `List`, tuples/records, closures, nullable values, and algebraic enums, including active-variant payloads.
- Keep references stored in maps, sets, arrays, lists, exception metadata, suspended tasks, channels, and clone state alive for exactly as long as their owners require them. Reclaim runtime-side collection handles and metadata rather than leaking them indefinitely.
- Align semantic admissibility, IR types, LLVM layouts, and runtime ABI. A source type that the backend cannot represent must be rejected with a diagnostic before code generation, not reach a panic or invalid cast.
- Make float-to-integer, decimal exponentiation, duration conversion, and temporal arithmetic/conversion range-safe, with consistent native/interpreter outcomes and controlled failures.
- Bound regex compilation retention and make grapheme and decimal operations avoid repeated work without changing their documented semantics.
- Make GC marking and deep cloning robust to deep object graphs; ensure cancellation follows the same `finally`, resource-close, and journal-cleanup paths as other controlled exits.
- Add type-combination, GC-pressure, graph-depth, cancellation, and numeric-boundary regression coverage; publish verified implementation status and measured optimization results.
- Enable or adjust LLVM optimization only behind semantic-equivalence evidence and benchmarks, after undefined or unchecked behavior in affected paths is removed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `zirk-memory-safety`: complete safe-root tracing, ownership reachability, deep graph traversal, and cancellation cleanup guarantees.
- `zirk-object-memory`: define safe managed handles and descriptor behavior for every identity-bearing object and partially constructed graph.
- `zirk-data-types`: specify correct representation and boundary behavior for compound, nullable, enum, and scalar values.
- `zirk-collections`, `map-collections`, and `set-collections`: require valid GC metadata, managed-reference retention, and supported generic representations.
- `zirk-type-system` and `zirk-ir-lowering`: reject unsupported generic/container combinations before backend emission and preserve checked failure semantics.
- `zirk-native-codegen` and `zirk-scalars`: require checked float conversions and semantic parity across native and interpreted execution.
- `exact-decimal-arithmetic` and `zirk-temporal-types`: define overflow-safe exponent, duration, date, and time operations.
- `zirk-regex`: bound compiled-pattern retention and preserve predictable resource use.
- `zirk-errors`, `zirk-resources`, `zirk-callables`, and `zirk-structured-concurrency`: keep cached exception references alive and run cleanup on cancellation and controlled exits.

## Impact

Affected code includes semantic analysis and lowering, IR verification, LLVM emission, the managed-memory collector, object and collection runtimes, exception/resource/concurrency runtimes, decimal/float/text/temporal helpers, the interpreter, compiler fixtures, and benchmarks. The change affects normative safety and failure behavior, implementation status, documented limitations, and optimization claims; update the handbook, feature-status/roadmap sources, and the companion `../zirk-lang-site` repository after reviewed implementation evidence exists. No dependency or public syntax change is intended. Any necessary restriction on currently accepted but unrepresentable generic collection element types must produce an explicit diagnostic and be called out as a compatibility change.
