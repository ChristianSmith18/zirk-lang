## ADDED Requirements

### Requirement: Structural unions and intersections have complete N-ary value semantics
Structural object types SHALL compose with unions and intersections at any finite arity. A structural union SHALL preserve which canonical alternative was selected whenever that distinction is needed for safe narrowing or dispatch. A structural intersection SHALL describe the combined requirements over the same underlying value and SHALL NOT imply a new nominal identity. Assignment, contextual construction, and type identity SHALL be independent of source order and duplicate alternatives.

#### Scenario: N-ary structural union preserves the selected alternative
- **WHEN** a value satisfying one member of a three-or-more-member structural union is constructed and passed through a typed value boundary
- **THEN** the selected canonical alternative remains available for narrowing and member projection

#### Scenario: Structural union order does not select a different shape
- **WHEN** aliases describe the same structural union with reordered or repeated alternatives
- **THEN** they have the same canonical union identity and behave identically at construction and use sites

#### Scenario: Contextual object construction selects one alternative
- **WHEN** an object literal is checked against a structural union and its properties satisfy exactly one alternative
- **THEN** the object is accepted and retains that alternative's precise property types

#### Scenario: Ambiguous contextual object construction is rejected
- **WHEN** an object literal satisfies multiple alternatives and no contextual evidence uniquely selects one
- **THEN** compilation reports an ambiguity instead of selecting by declaration order

#### Scenario: Structural intersection retains object identity
- **WHEN** one object is viewed through an intersection of two compatible structural types
- **THEN** reads and writes use the combined static requirements while the runtime object identity remains unchanged

#### Scenario: Incompatible required structure cannot be constructed
- **WHEN** a value is assigned to an intersection whose required member types or mutability rules cannot hold simultaneously
- **THEN** the assignment is rejected with the conflicting structural requirements identified
