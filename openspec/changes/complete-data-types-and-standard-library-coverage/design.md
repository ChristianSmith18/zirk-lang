## Context

See `proposal.md` for the requested gaps. Existing collection capabilities already require hashable keys/elements and early diagnostics, but runtime coverage is narrower than the generic contract. The type algebra and temporal specs also contain broad normative behavior alongside explicit delivery exclusions; those must be made consistent. The status catalog, not a source file or an OpenSpec task count, remains the evidence owner for what is executable today.

The current target contract distinguishes native execution on Linux x86_64/aarch64, macOS x86_64/aarch64, and Windows x86_64 from object emission for nine compiler targets. Existing temporal implementation dependencies include pinned `chrono` and `chrono-tz`; current JSON uses ordinary `Object`/`List<Dynamic>` values and a bounded parser. The stable standard-package model requires importable public identities while allowing narrow compiler-owned runtime operations.

The source-audited pre-change collection eligibility baseline is recorded in
[`collection-support-matrix.md`](./collection-support-matrix.md). It is
deliberately not the final supported matrix: native byte-based key equality and
interpreter recursive value equality currently diverge for some aggregates.

## Goals / Non-Goals

**Goals:**

- Make the support boundary explicit at the type and component level so semantic acceptance always implies defined lowering, runtime representation, GC tracing, and observable behavior.
- Keep hashing, equality, cloning, and layout rules compositional and consistent across interpreter/native execution.
- Deliver the named temporal and JSON additions as bounded, stable public APIs.
- Make the collection package, target evidence, and public status line up with existing Zirk package and portability contracts.

**Non-Goals:**

- Raising the 4096-byte inline payload ceiling or promising unbounded inline payloads.
- Reimplementing the already delivered `Deque`, `Queue`, `Stack`, or `PriorityQueue` families as a separate feature; the package publishes their existing identities.
- Implicit structural hashing for mutable `Object` graphs, implicit remote schema retrieval, or process-global locale state.
- Treating object emission as native execution or claiming runtime support on hosts that CI does not execute.
- Implementing unrelated specified-only packages such as `std.crypto`, `std.process`, `std.testing`, or `std.reflect`.

## Decisions

### Derive `Hash` and `Equal` only for stable value compositions

Use one recursive capability descriptor for semantic eligibility, runtime equality, and hashing. The descriptor must define the same field order and canonical alternative identity for both operations. Immutable tuples, eligible algebraic payloads, nullable values, exact Decimal values, and immutable structural value shapes may derive capabilities when every nested field qualifies. Mutable text components are captured as immutable snapshots when inserted into `Map` or `Set`, matching the existing scalar text-key rule. Mutable `Object` references use stable identity or an explicit user implementation; the compiler does not hash mutable fields recursively.

Collection `Equal` is a total equivalence relation and need not be the language's partial IEEE `==` for NaN. Binary floats compare by canonical bits: positive and negative zero are one value, all NaN encodings of a given width are one value, and all other values (including signed infinities) compare by their exact width-specific bits. Exact decimals compare by numeric value after coefficient/scale normalization; `Decimal?` compares absence only with absence and delegates present values to Decimal equality. `String` and `Char` compare by NFC-normalized content captured at insertion, including when nested inside tuples, enum variants, nullable values, or a tagged union. Tuples compare fields in declaration order; enums compare the canonical variant discriminant before their active fields; unions compare canonical alternative tag before payload. Immutable structural value derivation is available only when the type declaration and every field have immutable value semantics. Mutable class/`Object` references use identity and are never recursively hashed by mutable contents.

Alternative considered: structurally hash every class/Object graph. Rejected because mutation can move a key to a different hash bucket after insertion. Rejecting all aggregate keys was also rejected because it would preserve the current gap rather than complete compositional value support.

### Drive collection eligibility from a shared type-layout matrix

Define a single declarative matrix for `Map` key, `Map` value, and `Set` element eligibility, including storage, Hash, Equal, ABI, and GC tracing requirements. Semantic analysis uses this matrix to accept or reject source forms before IR. Lowering, interpreter, native descriptors, and runtime hash-table operations consume the same canonical layout description rather than maintaining independent hand-written combinations.

The support corpus covers every leaf family and each composition boundary, then targeted nested combinations (for example nullable tuple text, enum containing union, union stored in Map value and Set element). It does not claim every mathematically possible product without evidence. Any omitted combination remains rejected with a stable diagnostic until its row is implemented and verified.

### Preserve the active union tag and keep the payload ceiling

Carry the canonical variant tag through portable IR, interpreter values, and native ABI descriptors wherever a value can be narrowed, matched, dispatched, cloned, hashed, or compared. Intersections remain views of the same underlying identity. Payloads over 4096 bytes remain unsupported for inline storage; any alternative indirect representation requires a separate design and is not introduced here. Every unsupported representation fails in semantic analysis before backend emission.

Alternative considered: infer a union variant from payload shape at runtime. Rejected because overlapping structural alternatives make that ambiguous and would diverge from the checker’s canonical identity.

The audited current-stage matrix is maintained in `algebraic-support-matrix.md`. It distinguishes checker admission from evidence for IR, interpreter, native execution, containers, and GC; rows still marked incomplete must not be advertised as delivered.

### Keep one generic contract from declaration through dispatch

Check generic bodies and defaults once under their constraints, retain substitutions in portable IR, and specialize/share only where ABI and behavior remain equivalent. Reuse that substitution path for generic enum dispatch, contract method defaults, and nested contract arguments. Interpreter and native tests use the same source corpus and expected branch/payload results.

Alternative considered: add separate generic-enum and generic-trait dispatch shortcuts in each backend. Rejected because it would allow type substitutions and diagnostics to drift between the checker, interpreter, and LLVM path.

### Publish `std.collections` as a source-level facade over canonical built-ins

Choose an importable `std.collections` package as the stable public discovery and import surface. Its exports resolve to the existing compiler-owned collection type identities and operations; they do not wrap values in duplicate collection types. Keep existing built-in spellings source-compatible while the standard package becomes the documented canonical import path.

Comparison: a compiler-only public surface keeps resolver work smaller and matches today's built-in spellings, but it makes collection imports and discovery inconsistent with Zirk's standard package model and ties the public API to compiler-specific names. An importable facade gives users a stable, searchable import and keeps API ownership visible in standard-library sources, at the cost of implementing and testing canonical re-exports. Choose the facade because the repository's standard-library contract already requires stable logical package identities. Preserve the built-in implementation underneath so the package does not create duplicate types or reimplement specialized storage. A wholly source-implemented collection library is a separate alternative and is rejected because it would duplicate optimized storage and GC/ABI behavior.

### Use versioned zone and locale data with explicit policies

Retain the pinned IANA time-zone source already represented by `chrono-tz`; expose its data version. Bundle a pinned [Unicode CLDR](https://unicode.org/reports/tr35/tr35-dates.html) data provider for temporal localization, using ICU4X-compatible data and APIs so formatting is deterministic across hosts, including browser/WASM builds. Do not defer locale choice to OS process settings.

Accept IANA identifiers from the pinned database and expose the canonical identifier after resolving database links. Unknown identifiers return `TimeZoneError.UnknownIdentifier`; absent rule data returns `TimeZoneError.DataUnavailable`. Neither case falls back to a host database or silently degrades to a fixed offset. Fixed offsets have their own zone identity and never compare as named IANA zones.

Local wall-time resolution has two explicit dimensions: overlaps select `Reject`, `EarlierInstant`, or `LaterInstant`; gaps select `Reject`, `ShiftForwardByGap`, or `ShiftBackwardByGap`. APIs that construct a wall time in a zone require these choices or use `Reject` for both. Invalid dates and arithmetic outside the supported instant range return typed errors. Calendar month arithmetic clamps to the last valid day of the target month; `Period` still requires a date and zone context.

Define `TimeShift` as one immutable tagged type that preserves both existing temporal uses. Its clock form, produced by `Time ± Duration`, carries the wrapped `Time` and signed day offset. Its calendar form carries a `Period`, exact `Duration`, and explicit overlap/gap policies. Applying the calendar form in a zone applies the `Period` first, resolves the resulting local time under those policies, and then applies exact elapsed time on the instant timeline. Fields from the inactive form are null; applying the clock form to a `ZonedDateTime` is a typed form mismatch. This keeps clock carry, month/day changes, elapsed time, and DST resolution distinct while retaining one public type identity.

Accept syntactically valid BCP 47 tags only when the pinned CLDR data supports them. Malformed tags return `LocaleError.InvalidTag`; valid but unavailable tags or locale data return `LocaleError.UnsupportedLocale` or `LocaleError.DataUnavailable`. Do not silently substitute the process locale or an implicit fallback locale.

Alternative considered: rely on each host OS for zone and locale data. Rejected because outputs and rule versions would vary by machine, and browser/WASM targets cannot rely on the same host databases. A manually maintained Zirk locale database is rejected in favor of versioned upstream data.

### Expose checked Unix-unit factories for `Instant`

Use `Int64` for Unix seconds and milliseconds and `Int128` for Unix nanoseconds. Each static factory returns `Result<Instant, ArithmeticOverflowError>` after checking both unit scaling and the supported timeline range. This keeps source units explicit, preserves subsecond precision, and reports out-of-range values through Zirk's ordinary typed-error channel instead of wrapping or trapping.

### Extend `std.json` without changing its ordinary-value model

Keep strict `JSON.parse` behavior and current defaults (16 MiB input, nesting depth 128) as the compatibility baseline. Add a typed limits object whose values may be tightened or relaxed only within documented hard ceilings; all parser, writer, codec, query, schema, and streaming paths share these ceilings. Generated `JSONCodec<T>` code is compile-time generated and substitutes nested codecs without runtime reflection.

Use the existing contract conformance syntax as the derivation request: a concrete data class `T implements JSONCodec<T>` with neither codec method declared receives compiler-generated `decode(Dynamic): T` and `encode(T): Dynamic` methods. Generation consumes the resolved class and field types at compile time; it does not inspect object members at runtime. A class that implements `JSONCodec<U>` where `U` is a different type remains an ordinary hand-written codec, preserving the current `PersonCodec implements JSONCodec<Person>` pattern. Reject partial implementations (only one of `decode`/`encode`) rather than silently combining generated and user-written behavior. Generated nested fields substitute their resolved `JSONCodec` implementation recursively, and unsupported field types produce a source diagnostic naming the field path before lowering.

Generated codecs encode traditional enums with no associated payload as their variant-name string, preserving the existing simple-enum representation. Algebraic enums use a discriminated object: `{"$variant":"Variant","$payload":{"field":value}}`; a nullary alternative in an enum that has payload alternatives omits `$payload`. The generated JSON Schema requires the active tag and exactly the declared payload fields, and both directions recursively use the generated codec rules for associated values. Generic enum payload types are substituted from each concrete field instantiation before schema generation or codec expansion; target-specific generic dispatch evidence remains separately tracked.

Use [RFC 6901](https://www.rfc-editor.org/rfc/rfc6901.html) for JSON Pointer and [RFC 9535](https://www.rfc-editor.org/rfc/rfc9535.html) for JSONPath. Use [JSON Schema Draft 2020-12 Core and Validation](https://json-schema.org/specification) with local references by default; remote references require an explicit caller-supplied resolver. Provide incremental reader/writer operations with explicit async suspension where needed; synchronous iteration never hides suspension and errors never expose a partial value as success.

Pretty output is for presentation. Canonical output follows [RFC 8785 JCS](https://www.rfc-editor.org/rfc/rfc8785.html) and therefore accepts only its defined I-JSON numeric/string domain; exact Zirk numbers that cannot be represented without rounding return a typed error. JSON5 is a separate parser following [the JSON5 specification](https://spec.json5.org/) whose comments, relaxed keys/strings, trailing commas, and numeric forms do not loosen strict JSON. Both parsers return ordinary Zirk dynamic/object/list/scalar values and preserve exact numeric tokens until a checked conversion is requested.

The exact extension choices, stream/error rules, shared defaults and hard
ceilings, and representative per-extension corpus are frozen in
`json-support-matrix.md` and
`crates/zirk-runtime-core/tests/fixtures/json/profile-cases.json`. The local
vectors establish the profile surface; RFC JCS Appendix I and the official
JSON5 suite remain required implementation-time corpus supplements.

Alternative considered: one permissive parser mode that silently changes strict JSON, or one canonicalizer that rounds arbitrary exact numbers. Rejected because either choice would change existing callers' security/compatibility assumptions or produce signatures with lossy data.

### Report evidence per pipeline stage and host

For the five native CI hosts, run executable compiler corpus cases and collect GC/interpreter/native parity evidence. For all nine compiler targets, perform object emission and report unsupported capabilities explicitly. A status row records the exact matrix, revision, and whether evidence is source checking, lowering, interpreter execution, object emission, linking, or native execution. Float128 Windows support must be decided against the normative scalar contract: implement and execute it if the contract promises it; otherwise declare the exact Windows exclusion and test its pre-backend diagnostic.

### Synchronize implementation status and public docs after evidence exists

Update the canonical spec, type/stdlib status catalog, current limitations, handbook examples, and public index only after executable evidence is available. Commit the source docs before website sync; review generated content and the website-owned status catalog separately, with an audit date when evidence changes.

## Risks / Trade-offs

- [Recursive aggregate hashing could observe mutation or violate equal-hash coherence] → Restrict automatic derivation to stable value semantics, snapshot mutable text at insertion, and use property-based coherence tests over every supported composition row.
- [The explicit eligibility matrix can become another duplicated source of truth] → Make semantic diagnostics, lowering verification, and test case generation consume the same matrix and add a consistency check for unlisted admitted types.
- [Retaining generic and union metadata may increase IR size or specialization count] → Reuse canonical descriptors and specialize only when ABI/observable behavior requires it; measure representative compile-size and runtime cases.
- [Pinned time/locale data increases binary size and requires updates] → Select and document a bounded data set, publish its versions, measure native/WASM size, and update through a reviewed dependency/data refresh.
- [JCS cannot serialize every exact Zirk number without loss] → Return a typed domain error and document that ordinary stringify and canonical stringify have different contracts.
- [Schema validation, JSONPath, and streaming can consume excessive CPU or memory] → Apply shared hard limits, explicit result/step budgets, no implicit network resolver, and cancellation-aware async APIs.
- [Windows Float128 behavior may depend on backend or runtime support] → Gate status on end-to-end Windows execution or a tested source-level exclusion, never object generation alone.
- [Adding a package facade can create duplicate collection identities if resolver exports are wrong] → Assert imported and built-in names intern to the same canonical types and exercise source compatibility in compiler/interpreter/native tests.

## Migration Plan

1. Land the type/layout contracts, diagnostics, generated support matrix, and interpreter/native behavior while keeping unsupported rows rejected.
2. Add the `std.collections` facade and new temporal/JSON APIs with compatibility tests for existing built-in names, civil time types, strict JSON parsing, and manual codecs.
3. Enable a type/composition row only after its semantic, interpreter, native, target, and GC evidence passes; preserve the 4096-byte payload ceiling.
4. Publish the revised status catalog and limitations, validate runnable examples with the current compiler, commit the source documentation, synchronize and review `zirk-lang-site`, and commit the site separately.
5. If a release must be rolled back, restore the prior standard-package exports and feature gates together; do not leave a public type marked delivered when its supporting runtime row has been disabled.

## Open Questions

None. The collection distribution choice, `TimeShift` order, JSON standards/profile, locale data strategy, evidence matrix, and Float128 Windows decision have explicit resolution paths and acceptance conditions in the decisions and tasks above.
