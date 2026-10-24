# zirk-data-types Specification

## Purpose
Defines tuples, records, enums, unions, aliases, and their value semantics.

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

### Requirement: Closed data-only enums

Traditional and algebraic enums SHALL be closed data declarations and SHALL NOT contain user-defined methods. Traditional cases SHALL expose native `.name`, `.value`, and `to_string()` behavior without implicit mapping conversion or declaration order; the enum type itself SHALL expose the built-in static members `count`, `keys()`, `values()`, `from_name()`, and `from_value()` (see `enum-static-members`). Algebraic payloads SHALL be extracted only through exhaustive match.

#### Scenario: Enum method rejected

- **WHEN** an enum body declares a method `to_celsius()`
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

### Requirement: Example-driven documentation without semantic change

The `zirk-data-types` specification examples in the handbook SHALL illustrate the value/reference/unsafe spectrum for tuples, dynamic objects, classes, contracts, enums, unions, aliases, and built-in types, but SHALL NOT introduce new language semantics, new keywords, or unapproved type syntax.

#### Scenario: An object example shows reference semantics

- **WHEN** the dynamic object chapter is expanded
- **THEN** it includes a code example that demonstrates shared identity after assignment and explains how `is` compares object references

#### Scenario: A tuple example shows compile-time index

- **WHEN** the `tuple` chapter is expanded
- **THEN** it demonstrates `tuple[n]` and `tuple[-1]` as valid and `tuple[i]` with a runtime index as an error

### Requirement: Invalid examples match the compiler

Every invalid example in the new or expanded type chapters SHALL cite a diagnostic or controlled error that the current compiler or the normative spec defines, and SHALL NOT invent new error names.

#### Scenario: List mutation through an inmut::strict alias

- **WHEN** the `List` chapter shows an invalid example
- **THEN** the example names the expected borrow/alias error and, if the feature is not yet implemented, marks it with an implementation-status notice

### Requirement: Tuple members

A tuple value SHALL expose `length` (compile-time element count, `Int32`)
and `to_string()` (readable rendering) in addition to constant indexing
and destructuring.

#### Scenario: Tuple length

- **WHEN** `(1, "x", true).length` is read
- **THEN** the result is `3`

#### Scenario: Tuple rendering

- **WHEN** `(1, "x").to_string()` runs
- **THEN** the result is a `String` naming both elements

### Requirement: Traditional enum case members

A traditional-enum case SHALL expose `name` (declared case name,
`String`), `value` (explicit mapping or the case name by default), and
`to_string()` (the case name).

#### Scenario: Case name

- **WHEN** `Color.Red.name` is read on `enum Color { Red, Green }`
- **THEN** the result is `"Red"`

#### Scenario: Mapped value

- **WHEN** `Color.Red.value` is read on a `String`-mapped enum
- **THEN** the result is the mapped value

### Requirement: User-defined type categories include the renamed scalar families

The user-defined type categories documentation and diagnostics SHALL refer to `Decimal` as the exact base-ten scalar, `Dec` as its abbreviation, and `FloatN` as the IEEE 754 binary family. `Float` SHALL be an alias for `Float64`. The documentation SHALL recommend `Decimal` as the default fractional type and `FloatN` for performance-sensitive or hardware-integrated binary floating-point.

#### Scenario: Handbook describes `Decimal` as default

- **WHEN** a developer consults the built-in types reference
- **THEN** `Decimal` is described as the default exact fractional type and `FloatN` as the binary family

#### Scenario: Migration note for `Float64`

- **WHEN** a developer consults the type categories or migration sections
- **THEN** the documentation explains that the previous `Float64` is now `Float64` of the binary family and the previous `Float` is now `Decimal`

### Requirement: Type alias lowering

A `type` alias declared at module scope SHALL resolve to its underlying type throughout the compiler pipeline and SHALL be usable in executable programs without a `NOT_LOWERED` error.

#### Scenario: Alias in a variable declaration

- **WHEN** `type UserId = Int32;` and `mut x: UserId = 5;` are written
- **THEN** the variable `x` has type `Int32` and the program compiles and runs

#### Scenario: Alias to a generic type

- **WHEN** `type IntList = List<Int32>;` and `mut l: IntList = List<Int32>();` are written
- **THEN** the alias resolves to `List<Int32>` and `l` behaves as a `List<Int32>`

### Requirement: Numeric conversions have defined boundary outcomes

Conversions between binary floating values, exact-decimal values, integers, and durations SHALL either produce a representable result under the language's specified rounding rule or a controlled language failure. Native and interpreted execution SHALL agree on the value or failure category.

#### Scenario: Float cannot be represented by the destination integer
- **WHEN** a conversion receives NaN, infinity, or a value outside the destination integer range
- **THEN** it produces a controlled conversion failure and never invokes an undefined backend conversion

#### Scenario: Conversion is representable
- **WHEN** a finite floating value lies within the destination range
- **THEN** native and interpreted execution apply the same documented rounding rule

### Requirement: Object is the open dynamic object value
`Object` SHALL represent the common reference-object view for dynamic object literals and class instances. Its string-keyed dynamic properties SHALL preserve their Zirk runtime types. It SHALL NOT be the universal root type of scalars, enums, tuples, or other values. A dynamic property read whose type cannot be proven statically SHALL produce a tagged `Dynamic` value that requires safe narrowing before typed use.

#### Scenario: Object accepts a class reference but not a scalar
- **WHEN** a class instance and an `Int32` are each assigned to `Object`
- **THEN** the class instance preserves its identity through the object view and the scalar assignment is rejected

#### Scenario: Dynamic read is safely narrowed
- **WHEN** a property is read from an `Object` without a statically known property type
- **THEN** the result preserves its runtime tag and cannot be used as a concrete type until a checked narrowing succeeds

### Requirement: Object literals support static and dynamic string keys
An object literal SHALL accept identifier-shaped keys without quotes, quoted string keys, and interpolated string keys. An interpolated key SHALL be evaluated as a string using Zirk's ordinary interpolation conversion. Computed property access SHALL accept a string key. Literal property values SHALL preserve their own inferred or expected types.

#### Scenario: Identifier key creates a string property
- **WHEN** `{ name: "Ada" }` is evaluated
- **THEN** it creates an `Object` with the string key `name` and a `String` value

#### Scenario: Interpolated key is evaluated
- **WHEN** `key` contains `custom_status` and `{ "{key}": null }` is evaluated
- **THEN** the object contains the key `custom_status` with a `Null` value

### Requirement: User-defined types use contracts for structural shape
The user-defined type model SHALL include classes with reference identity, contracts with structural property and operation requirements, enums with closed variants, unions, aliases, and open dynamic `Object` values. Scalars SHALL retain their own types and SHALL NOT be widened to `Object`.

#### Scenario: Structural declarations use contract
- **WHEN** a developer defines a named shape for an object
- **THEN** the declaration is a `contract`, not a record or interface

### Requirement: Enums retain component-wise cloning
An enum SHALL implement `Clone` when all associated values implement `Clone`. A dynamic `Object` SHALL NOT receive implicit value-copy semantics from this rule.

#### Scenario: Clone an enum with cloneable payloads
- **WHEN** every associated type of an enum implements `Clone` and an enum value is cloned
- **THEN** the clone contains independently cloned payload values

### Requirement: JSON materializes ordinary Zirk values
The JSON API SHALL parse JSON objects into ordinary `Object` values, arrays into ordinary Zirk collections of dynamic values, and JSON scalars into their corresponding Zirk scalar or `Null` values. The public parse result SHALL be `Dynamic`, not a second public object/tree model. Stringification SHALL serialize those values back to JSON, preserve exact JSON number behavior, and raise a catchable error for cycles or values that JSON cannot represent. Typed JSON codecs SHALL use contracts for structural object validation and SHALL NOT require retained decorators or runtime field scanning.

#### Scenario: Parsed JSON object is a dynamic object
- **WHEN** JSON containing `{"status":"ok"}` is parsed
- **THEN** the result is an `Object` whose `status` property can be read and narrowed to `String`

#### Scenario: Interpolated object key serializes
- **WHEN** an object literal uses an interpolated key and is passed to JSON stringification
- **THEN** the resulting JSON contains the evaluated string key

#### Scenario: Non-JSON runtime value is rejected
- **WHEN** stringification encounters a callable, resource, or cyclic object graph that JSON cannot represent
- **THEN** it raises a catchable JSON conversion error without returning partial JSON
