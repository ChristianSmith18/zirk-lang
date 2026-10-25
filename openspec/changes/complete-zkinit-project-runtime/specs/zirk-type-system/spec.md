## MODIFIED Requirements

### Requirement: Entrypoint

The checker SHALL NOT require a source-level function named `main`. For an application project, the manifest-selected entry file SHALL be checked as an executable top-level body; for an explicit source path, that file SHALL be checked as the selected entry. Declarations SHALL be checked across the complete static import graph before runtime execution. A source-level `fn main` SHALL be type-checked as an ordinary function and SHALL receive no special signature rule.

#### Scenario: Entry file has no `main` function
- **WHEN** the selected application entry contains valid top-level statements and no `main` function
- **THEN** type checking succeeds

#### Scenario: Main has an ordinary signature
- **WHEN** a source file declares a function named `main` with parameters or a non-`Void` result
- **THEN** it is checked by ordinary function rules without an entrypoint diagnostic

#### Scenario: Imported function named main is not the application entry
- **WHEN** a dependency exports `main` but the selected application entry does not call it
- **THEN** the dependency function is not invoked implicitly

#### Scenario: Unannotated main function
- **WHEN** a function named `main` omits its return type
- **THEN** its return type is inferred under ordinary function rules and is not used to select the application entry

#### Scenario: Entry contains an invalid top-level expression
- **WHEN** a selected entry contains a top-level statement that violates ordinary type rules
- **THEN** the checker reports the corresponding source diagnostic before runtime startup
