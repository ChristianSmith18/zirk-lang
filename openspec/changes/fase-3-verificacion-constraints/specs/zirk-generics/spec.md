# Delta spec: zirk-generics

## MODIFIED Requirements

### Requirement: Scope of generics in this phase

The checker SHALL NOT support associated types or higher-kinded type
parameters. Declared variance (`in T` / `out T`) is part of the language and
SHALL be verified per "Declared generic variance".

None of the remaining exclusions are in `ZIRK_LANGUAGE_SPEC.md`, and
supporting them would fix semantics that the spec does not fix.

#### Scenario: Unsupported construct
- **WHEN** an associated type or a higher-kinded type parameter is written
- **THEN** a diagnostic indicating that it is not part of the language is emitted
