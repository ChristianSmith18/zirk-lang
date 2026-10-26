## ADDED Requirements

### Requirement: Editor analysis understands composite type expressions
The language server SHALL parse and report diagnostics for nested union/intersection expressions, singleton constants, structural object types, generic type operators, and flow-sensitive narrowing using the same compiler semantics as CLI analysis. Hover and inlay information SHALL show the most precise reliable type and preserve useful alias names. Completion SHALL expose only members valid on every current alternative or on the active narrowed alternative.

#### Scenario: Hover shows a narrowed alternative
- **WHEN** the cursor is inside a branch narrowed by a Boolean discriminant
- **THEN** hover reports the selected contract and its exact literal properties

#### Scenario: Completion uses common union members
- **WHEN** completion is requested on a value before narrowing
- **THEN** only members valid on every union alternative are offered

#### Scenario: Type-computation budget diagnostic is live
- **WHEN** an edit exceeds a documented type-evaluation budget
- **THEN** the server reports the current diagnostic and clears it after the expression is corrected

#### Scenario: Invalid source never returns stale narrowing
- **WHEN** an edit changes a discriminant or leaves the expression incomplete
- **THEN** hover and completion omit stale types from the previous snapshot
