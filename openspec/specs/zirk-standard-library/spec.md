# zirk-standard-library Specification

## Purpose

Defines the standard-library module inventory and the shared contracts for API
signatures, outcomes, permissions, cancellation, hostile-input limits,
portability, and documentation synchronization.

## Requirements

### Requirement: The standard library has one synchronized capability owner
The standard-library capability SHALL inventory every public `std` module and
shared API contract defined by `docs/ZIRK_STDLIB_SPEC.md`. Its handbook owner
pages SHALL use the same signatures, result variants, permissions,
cancellation behavior, safety limits, and platform guarantees.

#### Scenario: A module API changes
- **WHEN** a normative standard-library signature or behavior changes
- **THEN** its specialized spec, OpenSpec capability, handbook owner, indexes,
  and affected examples are updated in the same change

### Requirement: Result and stream outcomes remain distinct
Operations returning `Result<T,E>` SHALL use `Ok(T)` and `Error(E)`. A
stream-specific enum MAY use `Value(T)`, `End`, and `Error(E)` only when the
operation explicitly returns that enum, such as `ReadResult<T>` or
`ReceiveResult<T>`.

#### Scenario: HTTP request succeeds
- **WHEN** an HTTP operation returning `Result<HTTPResponse,HTTPError>` succeeds
- **THEN** documentation and implementations expose `Ok(response)`, not
  `Value(response)`

### Requirement: Privileged APIs preserve denial explicitly
Filesystem, network, process, environment, system, and secret APIs SHALL retain
permission, decoding, platform, and cancellation failures in their declared
typed `Result<T,E>` outcome. If a caller consumes that result in an ordinary
value context, contextual Result projection SHALL map every `Error(E)` to
`null`; this loss of detail SHALL be documented. The standard library SHALL
NOT add nullable/default helpers that duplicate projection and `??`. Callers
that need to distinguish denial or host failure SHALL directly match or
preserve an explicitly typed Result.

#### Scenario: Environment projection uses a fallback
- **WHEN** `Env.get<Int32>(name) ?? fallback` receives any `Error`
- **THEN** it evaluates to `fallback`
- **AND** a direct match is required to distinguish permission denial from
  absence or another host failure

### Requirement: Standard Result APIs avoid redundant extraction helpers
The standard library SHALL expose `Result<T,E>` variants and composition
operations without separate status, extraction, fallback, or unwrap helpers.
It SHALL remove `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`,
`get_or_else`, `unwrap`, and `unwrap_error`. Environment reads SHALL use
`Env.get<T>(name): Result<T, EnvironmentError>` as the single retrieval
operation for present, absent, parse-error, permission-error, and host-error
outcomes.

#### Scenario: Environment value is read without a helper method
- **WHEN** `inmut mode = Env.get("APP_MODE");` is evaluated
- **THEN** the inferred value is `String?`

#### Scenario: Detailed environment outcome remains available
- **WHEN** a caller declares `inmut result: Result<String, EnvironmentError> = Env.get("APP_MODE");`
- **THEN** the result's typed variants remain available for direct matching

#### Scenario: Result composition remains available
- **WHEN** a caller transforms a successful value or error using `map`, `map_error`, `and_then`, `or_else`, or `or_throw`
- **THEN** the operation composes the Result without projecting it

### Requirement: String write by index

`String` SHALL support `s[i] = c` where `i` is a valid grapheme index and `c` is a `Char` or a single-grapheme `String`. The assignment SHALL mutate the shared String referent, invalidate the grapheme cache, and preserve the String's invariants. Complete aliases of the same String SHALL observe the replacement. A write through `mut` or `inmut` SHALL be valid referent mutation; a write reachable through `inmut::strict` SHALL be rejected.

#### Scenario: Write a character
- **WHEN** `mut s = "abc"; s[1] = 'X';` is executed
- **THEN** `s` becomes `"aXc"`

#### Scenario: Shared alias observes an indexed write
- **WHEN** `mut text = "Hello, World!"; inmut alias = text; text[0] = "😃";` is executed
- **THEN** both `text` and `alias` produce `"😃ello, World!"`

#### Scenario: Inmut binding mutates its referent
- **WHEN** `inmut text = "abc"; text[1] = 'X';` is compiled and executed
- **THEN** compilation accepts the indexed referent mutation and `text` becomes `"aXc"`

#### Scenario: Strict reference rejects an indexed write
- **WHEN** `inmut::strict text = "abc"; text[1] = 'X';` is compiled
- **THEN** compilation reports a strict reachable-mutation diagnostic

#### Scenario: Write out of bounds
- **WHEN** `s[10] = 'X';` is executed on a shorter `String`
- **THEN** an `IndexOutOfBoundsError` is produced

### Requirement: String slicing

`String` SHALL support `s[start:end:step]` where `start`, `end`, and `step` are integer or defaulted values. The result SHALL be a new `String` containing the selected graphemes.

#### Scenario: Basic slice
- **WHEN** `"hello"[1:4]` is evaluated
- **THEN** the result is `"ell"`

#### Scenario: Slice with step
- **WHEN** `"abcdef"[::2]` is evaluated
- **THEN** the result is `"ace"`

### Requirement: String search methods

`String` SHALL provide `contains(pattern): Boolean`, `starts_with(pattern): Boolean`, `ends_with(pattern): Boolean`, and `search(pattern): Int32?` returning the first grapheme index of a match or `null`.

#### Scenario: Contains
- **WHEN** `"hello".contains("ell")` is evaluated
- **THEN** the result is `true`

#### Scenario: Search
- **WHEN** `"hello".search("ll")` is evaluated
- **THEN** the result is `2`

### Requirement: String split and trim

`String` SHALL provide `split(separator): List<String>` and `trim(): String`. `split` with an empty separator SHALL split into graphemes. `trim` SHALL remove leading and trailing whitespace.

#### Scenario: Split
- **WHEN** `"a,b,c".split(",")` is evaluated
- **THEN** the result is `List<String>` with `"a"`, `"b"`, `"c"`

#### Scenario: Trim
- **WHEN** `"  hello  ".trim()` is evaluated
- **THEN** the result is `"hello"`

### Requirement: String substring

`String` SHALL provide `substring(start, end): String` returning the graphemes from `start` (inclusive) to `end` (exclusive), with the same bounds semantics as slicing.

#### Scenario: Substring
- **WHEN** `"hello".substring(1, 4)` is evaluated
- **THEN** the result is `"ell"`

### Requirement: Char classification

`Char` SHALL provide `is_uppercase(): Boolean`, `is_lowercase(): Boolean`, `is_digit(): Boolean`, `is_letter(): Boolean`, `is_whitespace(): Boolean`, and `is_alphanumeric(): Boolean`.

#### Scenario: Digit check
- **WHEN** `'5'.is_digit()` is evaluated
- **THEN** the result is `true`

#### Scenario: Letter check
- **WHEN** `'@'.is_letter()` is evaluated
- **THEN** the result is `false`

### Requirement: Char normalization

`Char` SHALL provide `to_uppercase(): String` and `to_lowercase(): String` returning the Unicode upper/lower equivalent. Multi-grapheme results from normalization are allowed and return `String`.

#### Scenario: Uppercase
- **WHEN** `'a'.to_uppercase()` is evaluated
- **THEN** the result is `"A"`

### Requirement: The standard library uses the package and module model

Every public standard-library area SHALL have a stable logical package/module
identity and a published root surface compatible with ordinary package imports.
`std.io` SHALL expose `stdin`, `stdout`, and `stderr` as published modules.

#### Scenario: Standard I/O import
- **WHEN** a program imports `{ stdin, stdout, stderr } from std.io;`
- **THEN** all three names resolve through the standard package root
- **AND** the source does not depend on a compiler-only intrinsic spelling

### Requirement: Standard-library Rust bindings are compiler-owned

The public standard-library source SHALL express its API as ordinary Zirk
declarations without `native`, `intrinsic`, or ABI annotations. The compiler
MAY associate a fully qualified standard-library declaration with Rust runtime
behavior only when the declaration belongs to the official standard-library
sources. External packages SHALL NOT acquire that association by name alone.

#### Scenario: Official I/O implementation
- **WHEN** `stdout.write` imported from `std.io::stdout` is compiled for a native target
- **THEN** the compiler may lower that known standard declaration to the runtime's native output implementation
- **AND** the source signature remains an ordinary Zirk function

#### Scenario: External same-name function
- **WHEN** an external package declares `write(text: String)`
- **THEN** it remains an ordinary Zirk function
- **AND** the compiler does not bind it to the standard runtime implementation

### Requirement: Standard-library convenience opening is deterministic

Published convenience members of an imported standard module SHALL be available
unqualified by default when unambiguous and SHALL remain selectable through
`module.member`. The standard library SHALL document collisions and the
qualification form for every convenience member.

#### Scenario: Unqualified standard call
- **WHEN** `{ stdout } from std.io` is imported and no `println` conflicts
- **THEN** `println("ready")` is accepted
- **AND** it is equivalent to `stdout.println("ready")`

### Requirement: Network, HTTP, and environment packages ship with the toolchain

The standard library SHALL bundle the official `std.net`, `std.http`, and
`std.environment` Zirk source packages and resolve them through their stable
logical imports on every supported native target. These packages SHALL enter
the ordinary module graph, and compiler-owned host behavior SHALL bind only to
declarations from the official bundled sources and their fully qualified
identities.

#### Scenario: Compile without a separately installed standard library
- **WHEN** a project imports `std.http` or `std.environment` using the installed Zirk toolchain
- **THEN** imports resolve without a checkout-relative path or separate library installation
- **AND** the same source package identity is used across supported native targets

#### Scenario: An external package imitates a standard declaration
- **WHEN** an external package declares a function with the same short name as a standard host-bound function
- **THEN** the function remains an ordinary package function and receives no compiler-owned host binding

### Requirement: Standard library source stays inspectable around host bindings

Official `.zrk` package sources SHALL contain the public APIs and all portable library policy, validation, composition, and error mapping that can reasonably execute in Zirk. Compiler-owned host bindings SHALL be narrow operations requiring operating-system access or an audited protocol engine, SHALL be documented with their input validation, permission boundary, limits, cancellation, and resource ownership, and SHALL resolve only by the official package identity. The toolchain SHALL NOT introduce source-level native or ABI annotations for these packages.

#### Scenario: Inspect portable HTTP policy
- **WHEN** a developer opens the bundled `std.http` sources
- **THEN** redirect policy, header/body validation, routing behavior, and typed error mapping are visible as Zirk code
- **AND** transport calls cross only the documented host binding boundary

#### Scenario: Inspect native networking APIs
- **WHEN** a developer opens the bundled `std.net` sources
- **THEN** public types, API signatures, and portable validation are visible as Zirk code
- **AND** operating-system socket and audited TLS/QUIC engine operations are explicit host boundaries

#### Scenario: External package imitates a host binding
- **WHEN** an external package declares the same short method name as a standard host-bound operation
- **THEN** it remains an ordinary Zirk implementation and receives no runtime association

#### Scenario: Review a new host binding
- **WHEN** a standard-library change introduces a new host binding
- **THEN** its official source identity, host necessity, limits, permissions, cancellation, and ownership contract are reviewed and tested

### Requirement: Expanded networking packages publish target-specific capabilities

The bundled `std.net`, `std.http`, and `std.environment` packages SHALL publish the documented second-delivery APIs through their stable logical imports. Native and browser targets SHALL expose only capabilities they can implement securely; unsupported target operations SHALL produce explicit capability errors. The package indexes, API references, examples, and status catalog SHALL identify delivered protocols, required permissions, resource limits, target support, and known exclusions consistently.

#### Scenario: Import a new native transport API
- **WHEN** an application imports an implemented UDP, local IPC, interface, TLS, or HTTP/3 API on a supported native target
- **THEN** it resolves through the bundled package without a separate standard-library installation

#### Scenario: Import a native-only API in a browser target
- **WHEN** browser code references an API that requires a native socket or listener
- **THEN** the target reports an explicit capability diagnostic rather than resolving to an unsafe or fake implementation

#### Scenario: Review the published capability matrix
- **WHEN** a user consults the standard-library index or feature status page
- **THEN** the same protocol, target, permission, and remaining-work status appears in the handbook and companion website

### Requirement: Standard APIs use one outcome contract per operation
The standard library SHALL NOT publish a nullable or default-returning helper that duplicates an operation's `Result<T, E>` lookup or failure outcome. Callers MAY consume that Result through contextual projection and `??`, or preserve its typed error with an explicit Result context or `match`. A nullable-only API MAY remain when absence is its complete outcome and there is no parallel Result operation. When an operation has multiple successful states, its Result payload SHALL encode those states with a non-null domain value rather than using `T?` to distinguish success from absence.

#### Scenario: A Result lookup is consumed as nullable
- **WHEN** a caller uses an array, list, or map `get` in a value context expecting a nullable element
- **THEN** it receives the element or `null` without a separate `get_or_null` method

#### Scenario: A Result lookup retains diagnostic detail
- **WHEN** a caller needs to distinguish an absent element or key from other outcomes
- **THEN** it uses an explicit Result context or `match` and receives the typed error

#### Scenario: A nullable-only search remains available
- **WHEN** a sequence search such as `find(predicate)` has no match and no Result-returning equivalent
- **THEN** its nullable result remains the single API for that operation

#### Scenario: Multiple successful states remain distinct from failure
- **WHEN** a standard API can successfully report either a value or a domain state such as end-of-stream
- **THEN** its `Result` payload uses a non-null domain type to represent those successful states, while `Error` retains operational failure

### Requirement: Output functions accept heterogeneous Stringifiable arguments

`stdout.print`, `stdout.println`, `stderr.print` and `stderr.println` SHALL be variadic over `Stringifiable`. They SHALL render the arguments left to right, join them with one space, and write the result; `println` SHALL add one line break and `print` SHALL add none. A call with no arguments SHALL write only the line break for `println` and nothing for `print`. The standard-library source SHALL perform the joining and call `to_string()` itself, delegating only single-text writes to the runtime.

#### Scenario: Mixed argument types

- **WHEN** a program calls `stdout.println("count", 2, true, label)` where `label` is a class whose `to_string()` returns `items`
- **THEN** the output is `count 2 true items` followed by one line break

#### Scenario: Nullable argument

- **WHEN** a program calls `stdout.println("done", 3.5, null)`
- **THEN** the output is `done 3.5 <Null>`

#### Scenario: Argument with no text form

- **WHEN** a `Void` expression is passed to `stdout.println`
- **THEN** compilation fails with a diagnostic that there is no conversion from `Void` to `Stringifiable`

### Requirement: std.io stream API has one normative owner

The standard-library inventory and handbook SHALL describe the delivered
`std.io` stdin, stdout, and stderr APIs using the signatures, `ReadResult`,
`IoError`, UTF-8, EOF, output ordering, and browser exclusions defined by the
standard-streams capability.

#### Scenario: Stream API changes
- **WHEN** a public `std.io` stream signature or behavior changes
- **THEN** the standard-streams specification, `ZIRK_STDLIB_SPEC.md`, handbook,
  examples, and status evidence are updated together

### Requirement: Bundled filesystem APIs have one documented status

The bundled `std.fs` and `std.path` packages SHALL be included in the standard
library inventory. Their status SHALL distinguish the synchronous bounded and
progressive operations delivered by the core filesystem change from directory,
link, watcher, lock, and asynchronous operations deferred to a later change.
Their handbook pages, indexes, examples, and target exclusions SHALL agree
with the filesystem capability specification.

#### Scenario: Filesystem module delivery is documented
- **WHEN** `std.fs` or `std.path` is delivered
- **THEN** the standard-library specification, filesystem capability,
  handbook pages, indexes, implementation status, and examples describe the
  same supported operations, permissions, limits, and target exclusions

#### Scenario: Deferred filesystem operations are not reported as delivered
- **WHEN** standard-library status is published after the core filesystem
  change
- **THEN** directory traversal and mutation, links, temporary paths, locks,
  watchers, and asynchronous filesystem operations remain identified as
  deferred until their companion change is implemented

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
