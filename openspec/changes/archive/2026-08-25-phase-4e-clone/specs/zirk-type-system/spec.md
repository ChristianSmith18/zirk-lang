## MODIFIED Requirements

### Requirement: Projection copy and whole-reference aliasing
The checker SHALL classify reference expressions as whole references, projection reads, or places. Whole-reference assignment/passing/return/capture SHALL preserve aliasing; projection reads SHALL require deep Clone and produce independence; places SHALL preserve access to original storage. Destructuring, matching, callable capture, collection extraction, and generic T SHALL follow the same rule.

#### Scenario: Generic projection needs Clone
- **WHEN** a generic function returns `values[0]` for unconstrained T
- **THEN** the checker requires `T from Clone` or rejects extraction

### Requirement: Object reference strictness

For class references, `mut` SHALL allow reassigning and mutating the object,
`inmut` SHALL prevent only reassignment, and `inmut::strict` SHALL prevent
reachable mutation. A strict reference SHALL NOT become a mutable alias
nor be acquired while a mutable alias remains accessible.

#### Scenario: Mutable clone from a strict reference
- **WHEN** a strict object implements `Clone` and is cloned into a `mut` binding
- **THEN** the independent clone can be mutated without altering the original object
