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

### Requirement: Enums algebraicos

Un `enum` SHALL admitir variantes con valores asociados, extendiendo el enum sin datos de la fase anterior.

Un caso tradicional sin mapping SHALL exponer como valor observable su nombre exacto y NO SHALL recibir un índice numérico implícito. Un caso MAY declarar un mapping de cadena o numérico compatible mediante `->`.

#### Scenario: Variante con datos
- **WHEN** se declara `enum Shape { Circle(Int32), Rect(Int32, Int32) }`
- **THEN** `Shape.Circle(3)` construye un valor

#### Scenario: Enum sin datos sigue funcionando
- **WHEN** se declara `enum Direction { North, South }`
- **THEN** compila igual que en la fase anterior

#### Scenario: Constructor con aridad incorrecta
- **WHEN** se construye una variante con más o menos valores de los declarados
- **THEN** se emite un diagnóstico que indica la aridad esperada

### Requirement: Destructuring en `match`

Un patrón SHALL poder extraer los valores asociados de una variante, ligándolos a nombres dentro del brazo.

#### Scenario: Extraer los datos de una variante
- **WHEN** se escribe `match s { Shape.Circle(r) => r, Shape.Rect(w, h) => w * h }`
- **THEN** `r`, `w` y `h` están ligados en su brazo con el tipo declarado

#### Scenario: Patrón con aridad incorrecta
- **WHEN** un patrón de variante liga menos nombres de los que la variante declara
- **THEN** se emite un diagnóstico que indica la aridad

#### Scenario: Exhaustividad con datos asociados
- **WHEN** un `match` sobre un enum algebraico omite una variante y no tiene `_`
- **THEN** se emite el diagnóstico de exhaustividad nombrando la variante faltante

### Requirement: Records

Un `record` SHALL ser inmutable y tener semántica estructural, según `ZIRK_LANGUAGE_SPEC.md` sección 7.

#### Scenario: Igualdad estructural
- **WHEN** se comparan dos records con los mismos valores en sus campos
- **THEN** `==` produce `true` aunque sean instancias distintas

#### Scenario: Inmutabilidad
- **WHEN** se asigna a un campo de un record
- **THEN** se emite un diagnóstico indicando que un record no se modifica

### Requirement: Value classes

Una value class NO SHALL tener identidad observable y SHALL poder almacenarse inline.

#### Scenario: Sin identidad
- **WHEN** se aplica el operador de identidad a dos value classes con el mismo contenido
- **THEN** se emite un diagnóstico indicando que no tienen identidad observable

#### Scenario: Almacenada sin indirección
- **WHEN** una value class es campo de otra declaración
- **THEN** ocupa su espacio dentro de ella, sin puntero intermedio

### Requirement: Uniones y alias

`A | B` SHALL declarar una unión, y `type` SHALL declarar un alias.

#### Scenario: Valor de una unión
- **WHEN** una variable se declara `Int32 | String`
- **THEN** admite valores de cualquiera de los dos

#### Scenario: Uso sin discriminar
- **WHEN** se usa un valor de unión donde se espera uno de sus miembros
- **THEN** se emite un diagnóstico
- **AND** la ayuda indica discriminarlo con `match`

#### Scenario: Alias
- **WHEN** se declara `type Id = Int32;`
- **THEN** `Id` e `Int32` son intercambiables
