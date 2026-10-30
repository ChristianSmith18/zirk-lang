# JSON Profiles and Conformance Matrix

This matrix freezes the intended behavior for task 1.7. It records future
conformance requirements separately from behavior already implemented by
`std.json`; a prepared fixture does not imply an API or implementation exists.

## Compatibility baseline and shared limits

| Concern | Frozen contract | Evidence / corpus |
|---|---|---|
| Strict JSON | `JSON.parse` and `JSON.stringify` keep their current strict behavior and ordinary `Dynamic`/`Object`/`List<Dynamic>` value model. Duplicate object names, malformed input, cycles during encoding, and documents above the existing input/depth defaults fail with `JsonConversionError`. Numeric tokens remain exact until a checked conversion is requested. | Existing `json_stringify_dynamic.zrk` E2E fixture; `zirk-runtime-core/src/json.rs` duplicate-key and default-limit tests; `strict` cases in `crates/zirk-runtime-core/tests/fixtures/json/profile-cases.json`. |
| Configurable limits | Existing defaults remain 16 MiB per document and nesting depth 128. Callers may choose lower or higher limits only up to hard ceilings: 64 MiB document bytes, depth 512, 16 MiB per decoded string, 1,000,000 members/elements per container, and 4,096 bytes per numeric token. All length limits count UTF-8 bytes except nesting depth and container item counts. | `limits` cases in the shared profile corpus; implementation and allocation-boundary tests are task 7.2. |
| Query and schema budgets | JSONPath query text is limited to 64 KiB, evaluation to 1,000,000 selector/filter steps, output to 100,000 nodes, and estimated selected JSON UTF-8 representation to 64 MiB before cloning results. Schema validation limits recursive evaluation to depth 256 and 10,000 local reference resolutions per document. Exceeding a budget returns a typed limit error, not a partial result. | `jsonpath` and `schema` cases in the shared profile corpus; enforcement tests are tasks 7.4 and 7.7. |

## Normative extension profiles

| Extension | Normative profile | Frozen choices and error behavior | Corpus |
|---|---|---|---|
| JSON Pointer | [RFC 6901](https://www.rfc-editor.org/rfc/rfc6901.html), Unicode string form. URI-fragment parsing is outside this API. | Empty pointer selects the root. Tokens decode `~1` before `~0`; any other `~` escape is invalid. Object member names compare exact Unicode code points without normalization. Array indexes use canonical non-negative decimal notation (`0` or a nonzero digit followed by digits); `-`, leading zeroes, overflow, wrong-container access, and missing members return typed lookup errors. | `pointer` corpus includes root, object, array, empty member, slash/tilde escapes (including `~01`), malformed escapes, missing paths, bad indexes, and scalar traversal. |
| JSONPath | [RFC 9535](https://www.rfc-editor.org/rfc/rfc9535.html), including its standard selectors, filter grammar, and registered standard functions. | Results are an ordered node list with RFC-defined traversal and duplicate behavior. A valid query with no matches returns an empty list; malformed syntax, unknown functions, budget exhaustion, and invalid function argument types are distinct typed errors. User-defined function extensions and non-RFC path syntax are excluded. | `jsonpath` corpus includes root/name/index/wildcard/descendant/slice/filter/function queries, empty results, malformed syntax, and limit exhaustion. |
| JSON Schema | Draft 2020-12 Core and Validation vocabularies, with the standard applicator vocabulary used by Validation. | Local references resolve only within the supplied schema resource set. Remote references require an explicit resolver supplied by the caller; validation never performs implicit network access. `format` is annotation-only unless the caller explicitly selects the Format-Assertion vocabulary. Unsupported required vocabularies and unresolved references are typed errors. Evaluation obeys the shared schema depth/reference budgets. | `schema` corpus includes valid/invalid values, required/type/range/items/properties constraints, local `$ref`, recursion, format annotation/assertion, unsupported vocabularies, unresolved and remote references, and budget exhaustion. |
| Canonical JSON | [RFC 8785 JCS](https://www.rfc-editor.org/rfc/rfc8785.html), constrained to its I-JSON input domain. | No whitespace; object properties sort by UTF-16 code units; array order is preserved. Strings are not Unicode-normalized. Duplicate names, invalid Unicode, non-finite numbers, and exact Zirk numeric values outside the JCS/I-JSON domain return typed errors rather than being rounded. Ordinary `stringify` remains distinct and keeps exact-number behavior. | `jcs` corpus includes RFC primitive/property-order vectors, escaping, array ordering, negative zero, duplicate names, invalid Unicode, and out-of-domain exact integers/decimals. Official RFC 8785 Appendix I number vectors are a required supplement in task 7.6. |
| JSON5 | The complete grammar in the [JSON5 specification](https://spec.json5.org/), exposed by a separate parser. | Comments, IdentifierName keys, single/double quoted strings, line continuations, trailing commas, hexadecimal numbers, leading/trailing decimal points, explicit plus signs, `Infinity`, and `NaN` follow the specification. Duplicate object names follow object-initializer semantics: the last value wins. Strict `JSON.parse` accepts none of the JSON5-only syntax. Results use ordinary Zirk values; non-finite values map to their corresponding floating values. | Runtime-core runs all 83 valid and 31 invalid cases in the upstream [`json5/json5-tests`](https://github.com/json5/json5-tests) corpus, pinned at revision `ceb24d4080137d70833f86c25659c1331b80a387` with its MIT license. Local profile cases additionally verify limits, typed errors, strict-parser separation, and native/interpreter parity. |
| Streaming | Incremental reader/writer APIs over the same strict JSON value model and shared limits. | Async reads/writes suspend explicitly. A reader publishes a value only after the complete root document validates; a writer reports success only after the full document is committed to its sink. Cancellation, I/O failure, invalid input, and limit exhaustion are typed errors; no partial document is reported as success. Backpressure is bounded to one 64 KiB chunk outstanding per stream. | `streaming` corpus covers chunk boundaries inside UTF-8, escapes, numbers and delimiters; cancellation, read/write failure, backpressure, and no-partial-success behavior. |

## Corpus ownership and implementation status

The shared JSON fixture manifest is
`crates/zirk-runtime-core/tests/fixtures/json/profile-cases.json`. It gives
each profile stable case IDs and expected outcomes so implementation tasks can
run the same cases through the interpreter and native APIs. Strict JSON and
JSON5 cases execute in runtime-core; JSON5 also has full upstream valid/invalid
corpus coverage and a native/interpreter parity case. RFC 8785 Appendix I and
RFC 9535 conformance vectors are also covered by dedicated runtime-core tests.
Generated codecs, streaming, and JSON Schema remain implementation work under
tasks 7.1, 7.5, and 7.7.

The explicit separation protects compatibility: adding JSON5, limits, queries,
canonical output, schema validation, or streaming must not change ordinary
`JSON.parse`, `JSON.stringify`, exact numeric tokens, or existing manual codecs.
