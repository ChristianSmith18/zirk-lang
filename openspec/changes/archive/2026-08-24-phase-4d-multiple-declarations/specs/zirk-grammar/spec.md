## MODIFIED Requirements

### Requirement: Comma-grouped declarations and assignments are explicit
The grammar SHALL accept a comma-separated list of simple binding names before
one shared type annotation in a `mut`, `inmut`, or `inmut::strict` declaration.
It SHALL accept an optional comma-separated initializer list and simultaneous
assignment to a comma-separated list of assignable places. These forms SHALL
remain distinct from tuple construction and destructuring patterns.

#### Scenario: Shared-type declaration
- **WHEN** source declares `mut first, second: String;`
- **THEN** the AST records two mutable bindings with the shared `String` type

#### Scenario: Simultaneous swap
- **WHEN** source assigns `left, right = right, left;`
- **THEN** the AST records one simultaneous assignment with two destinations
  and two source expressions

#### Scenario: Assignment arity mismatch
- **WHEN** source assigns `left, right = right, left, extra;`
- **THEN** parsing preserves both arities so semantic analysis can emit a
  targeted count-mismatch diagnostic
