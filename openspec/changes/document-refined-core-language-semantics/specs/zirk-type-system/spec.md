## ADDED Requirements

### Requirement: Projection copy and whole-reference aliasing
The checker SHALL classify reference expressions as whole references, projection reads, or places. Whole-reference assignment/passing/return/capture SHALL preserve aliasing; projection reads SHALL require deep Clone and produce independence; places SHALL preserve access to original storage. Destructuring, matching, callable capture, collection extraction, and generic T SHALL follow the same rule.

#### Scenario: Generic projection needs Clone
- **WHEN** a generic function returns `values[0]` for unconstrained T
- **THEN** the checker requires `T from Clone` or rejects extraction

### Requirement: Final callable and object typing supersedes delivery limits
The final language type system SHALL support structural `Fn` adaptation, escaping closures, compiler-managed capture environments, abstract-class implementation, explicit overrides, declared generic variance, normalized unions, constant tuple indexes, copied iteration, and exhaustive guard-free matching. Feature phasing MAY diagnose an undelivered construct but SHALL NOT describe the final construct as semantically forbidden.

#### Scenario: Phase-limited closure
- **WHEN** the current compiler phase does not yet implement escaping closures
- **THEN** its diagnostic identifies the delivery phase while documentation retains the final legal Fn semantics

### Requirement: Default initialization and immutable data
Every omitted attribute SHALL receive its type default. Construction MAY finalize an `inmut` attribute before the object becomes available. Records and tuples SHALL remain immutable values; enums SHALL remain closed data without user methods; collections SHALL enforce referent permissions and strict aliases.

#### Scenario: Omitted class attribute
- **WHEN** an Int32 class attribute has no initializer and construct does not replace it
- **THEN** its value is zero after construction
