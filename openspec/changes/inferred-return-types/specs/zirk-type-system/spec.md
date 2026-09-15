## MODIFIED Requirements

### Requirement: Return coherence

The checker SHALL verify that the returned value matches the function's resolved return type — the declared `: T` annotation when present, or the type inferred from the body under `return-type-inference` when absent — and that every path of a non-`Void` function returns a value, considering that `if`, `match`, and blocks terminated by an infinite loop (`loop` without an exiting `break`) may form part of that path.

#### Scenario: Return of an incorrect type
- **WHEN** a function declared `Int32` returns a string
- **THEN** a diagnostic pointing to the returned expression is emitted

#### Scenario: Path without a return
- **WHEN** a non-`Void` function has an execution path that ends without returning
- **THEN** a diagnostic pointing to the end of that path is emitted

#### Scenario: Return with a value in a Void function
- **WHEN** a `Void` function returns a value
- **THEN** a diagnostic pointing to the expression is emitted

#### Scenario: Every `if` branch returns
- **WHEN** a non-`Void` function ends in an `if`/`else` where both branches return
- **THEN** the check succeeds without requiring an additional return afterward

#### Scenario: Inferred type checked like a declared one
- **WHEN** a function's return type is inferred as `Int32` and a later `return` yields `String`
- **THEN** the same return-mismatch diagnostic as with a declared `Int32` is emitted

### Requirement: Entrypoint

The checker SHALL require the existence of a `main` function whose resolved signature is `fn main(): Void`, per `ZIRK_RUNTIME_SPEC.md` section 2. The resolved signature is the declared one when `main` annotates its return type, or the inferred one when it does not; `fn main() { }` is therefore valid because it infers `Void`.

#### Scenario: Missing entrypoint
- **WHEN** the compiled file does not declare `main`
- **THEN** a diagnostic indicating that the entrypoint is missing is emitted

#### Scenario: Entrypoint with an incorrect signature
- **WHEN** `main` is declared with parameters or resolves to a return type other than `Void`
- **THEN** a diagnostic indicating the expected signature is emitted

#### Scenario: Unannotated entrypoint
- **WHEN** `main` is declared as `fn main() { }`
- **THEN** its inferred `Void` return satisfies the entrypoint signature
