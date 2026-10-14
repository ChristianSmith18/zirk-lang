## MODIFIED Requirements

### Requirement: Function declaration

The parser SHALL recognize function declarations with the form from `ZIRK_LANGUAGE_SPEC.md` section 6: `fn`, name, list of typed parameters, an optional return type after `:`, and a body between braces. The `: ReturnType` annotation is optional when a body is present; its absence marks the signature for return-type inference by the checker rather than producing a diagnostic.

#### Scenario: Function without parameters

- **WHEN** `fn main(): Void { }` is parsed
- **THEN** a function declaration named `main`, with no parameters and return type `Void`, is produced

#### Scenario: Function with parameters

- **WHEN** `fn add(a: Int32, b: Int32): Int32 { return a + b; }` is parsed
- **THEN** a function with two typed parameters and return type `Int32` is produced

#### Scenario: Omitted return type with a body

- **WHEN** `fn main() { }` is parsed
- **THEN** a function declaration with no declared return type is produced
- **AND** no missing-return-type diagnostic is emitted

#### Scenario: Missing return type

- **WHEN** a signature without a body omits the return type, as in an `interface` member, an `abstract` method, or an `extern "C" fn`
- **THEN** a diagnostic pointing to where the type was expected is emitted
- **AND** the help indicates that a signature without a body must declare its return type
