# zirk-data-types Specification

## Purpose
Defines tuples, records, value classes, algebraic and mapped enums, unions,
aliases, and their value semantics.
## Requirements
### Requirement: Standard text utilities preserve native text semantics
`std.text` SHALL provide reusable `StringBuilder`, compile-checked literal
formatting with `:name`, `:0`, `\:` and typed `|format` specifiers, fallible
dynamic formatting, and linear-time `Regex`. Regex literals SHALL use
`re'pattern'`; dynamic patterns SHALL parse through a typed result. Unicode
normalization SHALL be explicit and String-facing sequence behavior SHALL remain
grapheme-based.

#### Scenario: Literal format uses an incompatible formatter
- **WHEN** a literal applies a date formatter to a numeric argument
- **THEN** compilation reports the incompatible type-format contract

#### Scenario: Dynamic regex is invalid
- **WHEN** `Regex.parse(pattern)` receives an invalid runtime pattern
- **THEN** it returns `Result.Error(RegexError)` rather than throwing or
  compiling an unsafe backtracking program

### Requirement: JSON values and typed codecs are explicit
The standard library SHALL name every JSON public type with the uppercase
`JSON` acronym and SHALL provide an exact-number `JSONValue` tree plus
`JSON.parse`, `JSON.stringify`, `JSON.decode<T>`, `JSON.to_value`, and
`JSON.from_value<T>` without a redundant `JSON.encode`. Typed conversion SHALL
use compile-generated or explicitly implemented ordinary `JSONCodec<T>` values
and SHALL NOT require retained decorator metadata or runtime field scanning.

#### Scenario: Typed value is stringified
- **WHEN** an eligible `User` value is passed to `JSON.stringify`
- **THEN** the compiler resolves or synthesizes its typed codec and the runtime
  emits valid JSON without reflective field discovery

### Requirement: JSON processing is strict and bounded
JSON parsing SHALL require valid UTF-8 standard JSON, reject duplicate keys,
NaN, infinity, comments and trailing commas, preserve exact number spelling,
and enforce configurable safe limits for input size, nesting, strings,
collections and numbers. Streaming SHALL preserve the same limits and SHALL NOT
hide task suspension behind synchronous iteration.

#### Scenario: Object repeats a member name
- **WHEN** a JSON object contains the same key twice
- **THEN** parsing returns `JSONDuplicateKeyError` without a keep-first or
  keep-last mode

#### Scenario: Stream exceeds a limit
- **WHEN** a JSON reader reaches its configured byte or nesting limit
- **THEN** it returns `JSONLimitError`, stops consuming, and does not publish a
  partial document as success

### Requirement: Cryptographic values preserve algorithm and secrecy
The standard library SHALL expose algorithm-bound secret, key, nonce, digest,
signature and sealed-message types under canonical `Crypto` and equivalent
direct imports. Secrets and private/symmetric keys SHALL redact from output,
SHALL NOT implement ordinary `Clone`, `to_string`, or equality, and SHALL
require explicit fallible duplication and deliberate private export.

#### Scenario: Key is used with the wrong algorithm
- **WHEN** a `SymmetricKey<AES_256_GCM>` is passed to another AEAD family
- **THEN** type checking rejects the category/algorithm mismatch

### Requirement: Standard crypto exposes safe constructions only
The standard profile SHALL use system CSPRNG, modern SHA-2/SHA-3 hashing,
Argon2id password storage, HKDF/HMAC, authenticated AES-256-GCM and
ChaCha20-Poly1305, typed classical signing/key establishment, and finalized
post-quantum ML-KEM/ML-DSA/SLH-DSA options. It SHALL exclude MD5, SHA-1, raw RSA,
unauthenticated encryption, arbitrary curves/parameters, untyped nonces and
algorithm dispatch from untrusted names.

#### Scenario: AEAD nonce is omitted
- **WHEN** `Crypto.AES_256_GCM.seal` is called without a nonce
- **THEN** the implementation generates a unique nonce and returns it in the
  versioned sealed-message envelope

#### Scenario: Ciphertext authentication fails
- **WHEN** key, nonce, associated data, tag, or ciphertext is incorrect
- **THEN** open returns only `AuthenticationFailed` and releases no plaintext

### Requirement: Crypto profiles and providers fail closed
Crypto profiles SHALL be selected at build time as Standard or FIPS, SHALL
reject disallowed primitives without silent substitution, and SHALL require
official test vectors, version/provider pinning, equivalent results and
side-channel guarantees for native/hardware implementations. Pure CPU crypto
and system CSPRNG SHALL require no project authority; effectful key storage,
filesystem, HSM/KMS, or network access SHALL require its owning permission.

#### Scenario: Non-FIPS primitive appears in FIPS build
- **WHEN** a FIPS-profile project references a primitive unavailable to that
  profile
- **THEN** compilation fails with migration guidance rather than selecting a
  different runtime algorithm

### Requirement: Tuple value semantics
Tuple types SHALL use `Tuple(T...)`, values SHALL use `(v...)`, and heterogeneous element access SHALL use a compile-time constant index `tuple[n]` including normalized negative indexes. Tuples SHALL be immutable values, SHALL NOT support slicing or one-element/empty forms, and SHALL derive capabilities component-wise when requested.

#### Scenario: Tuple index type
- **WHEN** `(200, "OK")[1]` is checked
- **THEN** the result type is String and the extracted String is independent

### Requirement: Record value semantics
Records SHALL be nominal immutable values with named-only construction, omitted type defaults, methods without mutation, no custom constructor/inheritance, structural field equality, and explicit component-wise derivation.

#### Scenario: Omitted record fields
- **WHEN** `Settings()` omits Int32, Boolean, and String attributes
- **THEN** they contain `0`, `false`, and `""` respectively

### Requirement: Closed data-only enums
Traditional and algebraic enums SHALL be closed data declarations and SHALL NOT contain user-defined methods. Traditional cases SHALL expose native `.name`, `.value`, `to_string()`, `from_name()`, and `from_value()` behavior without implicit mapping conversion or declaration order. Algebraic payloads SHALL be extracted only through exhaustive match.

#### Scenario: Enum method rejected
- **WHEN** an enum body declares `fn to_celsius()`
- **THEN** compilation fails and domain behavior must be expressed by an external function with match

### Requirement: Normalized union semantics
Union order and duplicates SHALL not affect identity; `T | Never` and `T | T` SHALL normalize to T, `T | Null` to `T?`, and a subtype alternative subsumed by its supertype SHALL be removed. Only common compatible capabilities SHALL be callable before narrowing.

#### Scenario: Subsumed union member
- **WHEN** Dog extends Animal and `Animal | Dog` is formed
- **THEN** the type normalizes to Animal

### Requirement: Algebraic enums

An `enum` SHALL support variants with associated values, extending the dataless enum from the previous phase.

A traditional case without a mapping SHALL expose its exact name as its observable value and SHALL NOT receive an implicit numeric index. A case MAY declare a compatible string or numeric mapping via `->`.

#### Scenario: Variant with data
- **WHEN** `enum Shape { Circle(Int32), Rect(Int32, Int32) }` is declared
- **THEN** `Shape.Circle(3)` constructs a value

#### Scenario: Dataless enum still works
- **WHEN** `enum Direction { North, South }` is declared
- **THEN** it compiles the same as in the previous phase

#### Scenario: Constructor with incorrect arity
- **WHEN** a variant is constructed with more or fewer values than declared
- **THEN** a diagnostic is emitted indicating the expected arity

### Requirement: Destructuring in `match`

A pattern SHALL be able to extract the associated values of a variant, binding them to names within the arm.

#### Scenario: Extracting a variant's data
- **WHEN** `match s { Shape.Circle(r) => r, Shape.Rect(w, h) => w * h }` is written
- **THEN** `r`, `w`, and `h` are bound in their arm with the declared type

#### Scenario: Pattern with incorrect arity
- **WHEN** a variant pattern binds fewer names than the variant declares
- **THEN** a diagnostic is emitted indicating the arity

#### Scenario: Exhaustiveness with associated data
- **WHEN** a `match` over an algebraic enum omits a variant and has no `_`
- **THEN** the exhaustiveness diagnostic is emitted naming the missing variant

### Requirement: Records

A `record` SHALL be immutable and have structural semantics, per `ZIRK_LANGUAGE_SPEC.md` section 7.

#### Scenario: Structural equality
- **WHEN** two records with the same values in their fields are compared
- **THEN** `==` produces `true` even though they are distinct instances

#### Scenario: Immutability
- **WHEN** a record's field is assigned to
- **THEN** a diagnostic is emitted indicating that a record cannot be modified

### Requirement: Value classes

A value class SHALL NOT have observable identity and SHALL be storable inline.

#### Scenario: No identity
- **WHEN** the identity operator is applied to two value classes with the same content
- **THEN** a diagnostic is emitted indicating that they have no observable identity

#### Scenario: Stored without indirection
- **WHEN** a value class is a field of another declaration
- **THEN** it occupies its space within it, with no intermediate pointer

### Requirement: Unions and aliases

`A | B` SHALL declare a union, and `type` SHALL declare an alias.

#### Scenario: Value of a union
- **WHEN** a variable is declared `Int32 | String`
- **THEN** it accepts values of either type

#### Scenario: Use without discrimination
- **WHEN** a union value is used where one of its members is expected
- **THEN** a diagnostic is emitted
- **AND** the help indicates discriminating it with `match`

#### Scenario: Alias
- **WHEN** `type Id = Int32;` is declared
- **THEN** `Id` and `Int32` are interchangeable

### Requirement: Pointer.from accepts record and value class lvalues
`Pointer.from(place)` SHALL accept a `place` that is an lvalue of a `record` or `value class` slot or field, provided the final pointee type is FFI-safe.

#### Scenario: Pointer.from(record field)
- **WHEN** the program contains `Pointer.from(record_instance.x)` inside `unsafe`
- **THEN** the compiler lowers it to a chain of pointer-to-field operations ending at `Pointer<T>`

#### Scenario: Pointer.from(value class field)
- **WHEN** the program contains `Pointer.from(value_class_instance.y)` inside `unsafe`
- **THEN** the compiler lowers it to a pointer into the inline value-type storage

#### Scenario: Pointer.from rejects temporary value
- **WHEN** the program contains `Pointer.from(Foo().x)` or `Pointer.from(makePoint().y)`
- **THEN** the compiler reports an error because the root is not an lvalue

#### Scenario: Pointer.from on nested record field
- **WHEN** the program contains `Pointer.from(point.inner.x)` where `inner` is a `record` field
- **THEN** the compiler builds a `Pointer<Value>` to `inner` and then `PointerFromField` to `x`

