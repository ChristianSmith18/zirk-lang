## ADDED Requirements

### Requirement: Collections have a stable importable standard package

The standard library SHALL publish `std.collections` as a stable logical import for the public collection types and documented operations. Its exports SHALL resolve to the same canonical type identities and behavior as the existing built-in spellings; the package SHALL NOT create wrapper types with incompatible layout, equality, tracing, or dispatch. The compiler and runtime MAY continue to own specialized collection operations behind this source-level package boundary.

#### Scenario: Import a collection through the standard package

- **WHEN** a program imports `List`, `Map`, `Set`, `Deque`, or another published collection type from `std.collections`
- **THEN** the name resolves without a separate library installation and has the same type identity as its existing built-in spelling

#### Scenario: Existing built-in collection spelling remains compatible

- **WHEN** existing source uses an unqualified built-in collection name without an explicit package import
- **THEN** it continues to compile with unchanged type identity and runtime behavior

#### Scenario: Package exports the delivered sequence families

- **WHEN** `Deque`, `Queue`, `Stack`, or `PriorityQueue` is imported from `std.collections`
- **THEN** it exposes the already delivered documented surface and does not report those families as unimplemented

### Requirement: Extended JSON APIs preserve one bounded ordinary-value model

The bundled `std.json` package SHALL expose generated codecs, configurable bounded parsing and serialization limits, JSON Pointer, RFC 9535 JSONPath, streaming reader and writer operations, pretty output, RFC 8785-compatible canonical output for its supported I-JSON domain, JSON Schema 2020-12 validation, and a separate JSON5 parser. These APIs SHALL materialize ordinary Zirk values and use typed errors; they SHALL NOT introduce an incompatible second public JSON object model. Network retrieval of schemas SHALL NOT occur implicitly. Limits SHALL have safe defaults and hard ceilings, and streaming SHALL report errors without publishing partial success.

#### Scenario: Generated codec validates a declared type

- **WHEN** a concrete data class `T` declares `implements JSONCodec<T>` without handwritten codec methods and decodes JSON into `T`
- **THEN** required properties, types, optionality, and nested codecs are checked without runtime field scanning or user-written per-field boilerplate

#### Scenario: Generated codec preserves and overrides immutable field defaults

- **WHEN** a derived class has an immutable field initializer and JSON omits or supplies that property
- **THEN** omission retains the initializer value, while a supplied value is decoded and assigned during construction without mutating the completed instance

#### Scenario: Manual codecs remain explicit

- **WHEN** a class implements `JSONCodec<U>` for a different type `U` and declares both codec methods
- **THEN** the compiler preserves the hand-written implementation; a partial implementation of only one codec method is rejected with a source diagnostic

#### Scenario: Generated codecs preserve algebraic enum tags, payloads, and generic substitutions

- **WHEN** a generated codec encodes or decodes a concrete algebraic enum field, including concrete generic instantiations and payloads nested in supported collections
- **THEN** it uses a validated `{"$variant":"Variant","$payload":{"field":value}}` object for payload alternatives, a tag-only object for nullary alternatives of that enum, and rejects unknown tags, missing payloads, or extra payload fields with `JsonConversionError`; traditional no-payload enums retain their variant-name string representation

#### Scenario: Caller configures limits within hard ceilings

- **WHEN** a caller supplies JSON input, depth, string, collection, or number limits above or below defaults
- **THEN** values within the hard ceiling are honored and attempts to exceed it fail with a typed limit error

#### Scenario: Pointer and path query select documented matches

- **WHEN** JSON Pointer or JSONPath is applied to an ordinary parsed JSON value
- **THEN** pointer resolution follows RFC 6901 and query selection follows RFC 9535, returning typed syntax/evaluation errors and deterministic bounded results

#### Scenario: Streaming reader reaches a configured limit

- **WHEN** a streaming parse reaches a configured byte, depth, token, or collection limit
- **THEN** it stops at a documented boundary, returns a typed error, and does not expose a partial document as success

#### Scenario: Canonical output rejects out-of-domain numbers

- **WHEN** canonical output receives a value outside the supported RFC 8785/I-JSON numeric domain
- **THEN** it returns a typed canonicalization error rather than silently rounding exact Zirk numeric data

#### Scenario: Schema validation resolves only authorized local references

- **WHEN** a schema uses a local reference or requests a remote reference without an explicit resolver
- **THEN** local validation is bounded and remote retrieval is rejected unless the caller supplies an explicit authorized resolver

#### Scenario: JSON5 syntax remains separate from strict JSON

- **WHEN** JSON5 input uses comments, unquoted identifier keys, single-quoted strings, trailing commas, or JSON5 numeric syntax
- **THEN** the JSON5 parser accepts valid JSON5, returns ordinary Zirk values with typed errors for invalid input, and strict `JSON.parse` retains strict JSON behavior
