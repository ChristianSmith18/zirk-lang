## Purpose

Defines when a function or method may omit its `: ReturnType` annotation, how
the checker infers the return type from the body, and where an explicit
annotation remains mandatory because there is no body to infer from.

## ADDED Requirements

### Requirement: Optional return type annotation

A `fn` declaration or a method of a `class`, `record`, or `trait` that has a
body MAY omit the `: ReturnType` annotation. When omitted, the checker SHALL
infer the return type from the body instead of emitting the missing-return-type
diagnostic.

An explicit annotation, when present, SHALL remain the contract: the body must
satisfy it exactly as before, and inference SHALL NOT widen, narrow, or
otherwise alter it.

#### Scenario: Omitted annotation infers `Void`

- **WHEN** `fn main() { }` is checked
- **THEN** the function's return type resolves to `Void`
- **AND** no missing-return-type diagnostic is emitted

#### Scenario: Omitted annotation infers a value type

- **WHEN** `fn double(n: Int32) { return n * 2; }` is checked
- **THEN** the function's return type resolves to `Int32`

#### Scenario: Explicit annotation still enforced

- **WHEN** `fn f(): Int32 { return "x"; }` is checked
- **THEN** a return-type-mismatch diagnostic is emitted against the returned
  expression, as if inference did not exist

#### Scenario: Omitted annotation on a method

- **WHEN** a `class` method with a body omits its return type
- **THEN** its return type is inferred under the same rules as a `fn`

### Requirement: Inference from `return` expressions

The inferred return type SHALL be the unification of the types of every
`return expr` in the body, using the same join rules that `if`-expression
branches already follow. Bare `return;` statements do not participate in the
unification. If the body contains no `return expr`, the inferred type SHALL be
`Void` when the body can complete normally, or `Never` when every path
diverges (throws, `fatalError`, or a `loop` without an exiting `break`).

Once inferred, the resolved type is enforced exactly like a declared
annotation: every `return` statement — bare or valued — is checked against it,
and the all-paths-return rule applies unchanged.

#### Scenario: Bare return against an inferred value type

- **WHEN** a body contains `return 5;` on one path and a bare `return;` on
  another
- **THEN** the inferred type is `Int32`
- **AND** the bare `return;` is diagnosed as returning `Void` where `Int32` is
  expected, identically to a declared `Int32` function

#### Scenario: Falling off the end against an inferred value type

- **WHEN** `fn f(c: Boolean) { if c { return 5; } }` is checked
- **THEN** the inferred type is `Int32`
- **AND** the not-every-path-returns diagnostic is emitted, identically to a
  declared `Int32` function

#### Scenario: Incompatible return types

- **WHEN** one path returns `Int32` and another returns `String`
- **THEN** a diagnostic naming the incompatible return types is emitted

#### Scenario: Always-diverging body

- **WHEN** a body consists solely of `throw E(...)` or `fatalError(...)`
- **THEN** the inferred return type is `Never`

### Requirement: Demand-driven inference with cycle detection

Inference SHALL run before any caller's body that needs the signature is
checked: when the checker needs the return type of a function whose annotation
is absent, it SHALL infer it on demand. A recursive or mutually recursive
cycle in which no member declares its return type SHALL be rejected with a
diagnostic naming the cycle and requiring an annotation on at least one
member.

#### Scenario: Forward call to an unannotated function

- **WHEN** `fn a() { return b(); } fn b() { return 1; }` is checked
- **THEN** `b`'s return type is inferred before `a`'s body needs it
- **AND** `a` resolves to `Int32`

#### Scenario: Recursive function without annotation

- **WHEN** `fn f(n: Int32) { return f(n - 1); }` is checked
- **THEN** a diagnostic is emitted stating the return type cannot be inferred
  through the recursion and an annotation is required

#### Scenario: Mutual recursion without annotation

- **WHEN** `fn a() { return b(); }` and `fn b() { return a(); }` are both
  unannotated
- **THEN** the same cycle diagnostic is emitted naming the cycle
- **AND** annotating one member resolves the diagnostic

#### Scenario: Recursion with annotation

- **WHEN** `fn f(n: Int32): Int32 { return n <= 1 ? 1 : n * f(n - 1); }` is
  checked
- **THEN** no inference diagnostic is emitted

### Requirement: Mandatory annotation where no body exists

Signatures without a body SHALL keep the return type annotation mandatory:
`interface` members, `abstract` methods, `extern "C" fn` declarations, and
`fn dec` decorator signatures. Omitting it there SHALL produce the
missing-return-type diagnostic.

#### Scenario: Interface member without return type

- **WHEN** an `interface` method omits its return type
- **THEN** the missing-return-type diagnostic is emitted

#### Scenario: Extern declaration without return type

- **WHEN** `extern "C" fn puts(s: Pointer<UInt8>)` omits its return type
- **THEN** the missing-return-type diagnostic is emitted

#### Scenario: Trait method with body infers

- **WHEN** a `trait` method with a default body omits its return type
- **THEN** its return type is inferred like any method with a body

### Requirement: Interaction with overrides and contracts

A method that overrides an inherited implementation or satisfies a contract
member MAY omit its annotation; the inferred type SHALL then be checked
against the overridden or required signature exactly as a declared type would
be.

#### Scenario: Inferred override mismatch

- **WHEN** a `#override` method omits its annotation and the inferred type
  differs from the inherited signature's return type
- **THEN** the same signature-mismatch diagnostic as with a declared type is
  emitted
