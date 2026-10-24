# return-type-inference

## Purpose

Defines when a function or method may omit its `: ReturnType` annotation, how
the checker infers the return type from the body, and where an explicit
annotation remains mandatory because there is no body to infer from.

## Requirements

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

### Requirement: Return inference applies to executable object methods
Functions, class methods, and trait methods with bodies MAY omit return annotations under the existing inference rules. The checker SHALL unify every returned expression using existing branch-join rules, infer `Void` when a body completes without a value return, and infer `Never` when every path diverges. Explicit annotations SHALL remain binding. Contract operations and abstract or external signatures without bodies SHALL require an explicit return type. Object literals SHALL infer the dynamic `Object` type unless a contract or other expected type supplies a more specific structural view.

#### Scenario: Object literal has no expected contract type
- **WHEN** an object literal is returned without a declared result type or contract context
- **THEN** its inferred type is `Object`

#### Scenario: Contract operation requires a return type
- **WHEN** a contract operation omits its return type
- **THEN** the checker emits the missing-return-type diagnostic

#### Scenario: Incompatible return expressions
- **WHEN** one return path produces `Int32` and another produces `String`
- **THEN** a diagnostic identifies incompatible inferred return types

#### Scenario: Recursive inference requires an annotation
- **WHEN** mutually recursive functions have no return annotation and no non-recursive return evidence
- **THEN** inference reports the cycle and requires an explicit annotation on at least one function
