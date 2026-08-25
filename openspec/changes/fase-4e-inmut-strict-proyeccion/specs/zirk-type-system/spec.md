## MODIFIED Requirements

### Requirement: Multiple bindings and simultaneous assignment are atomic at the language level
Comma-grouped declarations SHALL apply their declared type and binding
permission to every name and SHALL require initializer arity to match when an
initializer list is present. Missing initializers SHALL use the declared type's
default independently for every binding. Simultaneous assignment SHALL require
equal source and destination arity, evaluate every source exactly once from
left to right before any destination write, type-check values positionally,
reject duplicate destinations, and then commit writes from left to right.
Rebinding `inmut` or `inmut::strict`, or mutating a projection through an
`inmut::strict` referent, SHALL be rejected.

#### Scenario: Swap observes original values
- **WHEN** `left` is `3`, `right` is `4`, and source executes
  `left, right = right, left`
- **THEN** `left` becomes `4` and `right` becomes `3` without either source
  observing an earlier destination write

#### Scenario: Strict reference projection is a destination
- **WHEN** a simultaneous assignment attempts to write an element through an
  `inmut::strict` collection reference
- **THEN** compilation rejects the write before evaluating an executable update

### Requirement: Reference mutability and strict aliases
For reference types, `mut` SHALL permit binding reassignment and referent mutation, `inmut` SHALL prohibit reassignment but permit referent mutation, and `inmut::strict` SHALL prohibit both. A strict reference SHALL NOT yield a mutable alias or be acquired while an accessible mutable alias exists; an independent `clone()` MAY be mutable.

#### Scenario: Inmut String element update
- **WHEN** an `inmut String` binding assigns a valid `Char` to one element
- **THEN** the shared referenced String is updated while binding reassignment remains prohibited

#### Scenario: Strict-to-mutable alias
- **WHEN** code assigns an `inmut::strict String` reference to a `mut` binding without cloning
- **THEN** type checking rejects the alias
