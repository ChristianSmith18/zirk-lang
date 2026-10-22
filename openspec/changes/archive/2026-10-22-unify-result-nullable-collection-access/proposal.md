## Why

Contextual `Result<T, E>` projection now gives callers a uniform way to consume expected failures as `T?`, but collection contracts still publish parallel `get` and `get_or_null` APIs and several public transport methods encode a second successful absence as `T?` inside `Result`. This change aligns those APIs with the language-wide rule and keeps meaningful success states distinct from failures.

## What Changes

- **BREAKING**: Make `get` the sole safe lookup API for `Array`, `List`, and `Map`; remove `get_or_null` from their public contracts and migrate examples to contextual projection, `??`, or `match`.
- Implement the specified collection `get` methods as typed `Result` lookups, with negative-index behavior and error types consistent with existing collection access.
- Replace public `Result<T?, E>` outcomes in HTTP and TLS with non-null domain outcomes inside `Result`, preserving distinctions such as end-of-stream versus transport failure and metadata absence versus TLS failure.
- Establish a standard-library rule: an operation MUST NOT publish a nullable/default helper that duplicates its `Result` outcome; nullable values remain appropriate when absence is the only outcome and no parallel `Result` operation exists.
- Update normative semantics, API specifications, handbook examples, feature status, migration notes, and the companion website source as required.

## Capabilities

### New Capabilities

### Modified Capabilities

- `zirk-collections`: Define one safe lookup API for arrays, lists, and maps and preserve strict indexing behavior.
- `map-collections`: Define the typed Result contract for Map lookup and retire the nullable alias.
- `zirk-http`: Use non-null domain outcomes for successful absence in HTTP response and SSE APIs.
- `zirk-networking`: Use non-null domain outcomes for optional TLS metadata while preserving typed TLS errors.
- `zirk-standard-library`: Prohibit redundant nullable/default alternatives to `Result` APIs and define when nullable-only APIs remain appropriate.

## Impact

Affected areas include compiler-native collection member checking and lowering, standard `std.http` and `std.net` Zirk sources, collection and transport specs, core semantic documentation, handbook examples and indexes, feature status, migration guidance, and `../zirk-lang-site`. The change is a public API migration; repository tests and cross-target validation are required before claiming implementation complete.
