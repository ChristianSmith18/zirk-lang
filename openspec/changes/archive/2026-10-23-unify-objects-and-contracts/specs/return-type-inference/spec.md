## REMOVED Requirements

### Requirement: Optional return type annotation
**Reason**: The method owner list includes records and omits contract operation signatures.
**Migration**: Inference remains available for functions, class methods, and trait methods with bodies; contract operations remain signatures with explicit return types.

### Requirement: Mandatory annotation where no body exists
**Reason**: The requirement names interface members instead of contract operations.
**Migration**: Annotate every contract operation, abstract method, extern declaration, and decorator signature that has no body.

## ADDED Requirements

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
