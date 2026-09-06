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

### Requirement: Specialization during lowering

Lowering SHALL produce one copy per combination of type arguments actually used.

This is what `ZIRK_LANGUAGE_SPEC.md` section 7 calls specializing where appropriate, and it is what allows storing records inline instead of behind a pointer.

#### Scenario: Two instantiations, two copies
- **WHEN** `identity` is used with `Int32` and with `String`
- **THEN** the IR contains one function for each

#### Scenario: Repeated instantiation
- **WHEN** the same combination of types is used multiple times
- **THEN** the IR contains a single copy
