## ADDED Requirements

### Requirement: In-depth per-type chapter contract
Every built-in type chapter SHALL explain purpose, construction/literals, inference, storage category, mutability, conversions, native operators, properties, methods, controlled errors, valid examples, invalid examples, and interactions when applicable. Chapter length SHALL follow semantic complexity rather than a fixed template size.

#### Scenario: Complex type receives deeper treatment
- **WHEN** `String`, `Float`, `ZonedDateTime`, or `Duration` has edge cases absent from `Boolean`
- **THEN** its chapter includes the additional explanations and examples instead of being constrained to the same size

### Requirement: Operator and API tables remain explanatory
Operator matrices and property/method catalogs SHALL state operand types, result types, mutation behavior, error conditions, and link to explanatory examples. A table SHALL NOT be the sole explanation of surprising behavior.

#### Scenario: Contextual cast reference entry
- **WHEN** the reference table lists `Float(a / b)`
- **THEN** it links to prose that explains operand conversion before evaluation and contrasts ordinary integer division

### Requirement: Type documentation contradiction audit
Completion SHALL include repository-wide checks for obsolete `Decimal*` naming, code-point-only `Char`, immutable or copy-on-write String claims, non-signed Duration claims, permissive strict aliases, and incompatible native operator examples.

#### Scenario: Superseded claim remains
- **WHEN** an audit finds a published page calling String immutable
- **THEN** the change remains incomplete until the page is corrected or explicitly scoped to a different value
