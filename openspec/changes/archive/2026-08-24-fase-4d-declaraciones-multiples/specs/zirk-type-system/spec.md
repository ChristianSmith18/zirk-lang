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
