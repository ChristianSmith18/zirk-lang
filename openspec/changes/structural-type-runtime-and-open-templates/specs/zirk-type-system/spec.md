## ADDED Requirements

### Requirement: Structural composite relations and member access remain sound
The type checker SHALL solve structural union and intersection relations compositionally for any finite number of members. A union source SHALL satisfy a destination only when every possible alternative is assignable; a union destination SHALL accept a source when at least one alternative is satisfied. An intersection destination SHALL require every constituent, and an intersection source SHALL provide only the combined properties and operations proven by all constituents without inventing nominal identity. Reads, writes, and callable members SHALL respect property optionality, mutability, and function variance.

#### Scenario: N-ary union source is checked branch by branch
- **WHEN** a value of `A | B | C` is assigned to a structural destination
- **THEN** the assignment succeeds only if A, B, and C each satisfy that destination

#### Scenario: Intersection source exposes combined required members
- **WHEN** a value has type `HasName & HasId`
- **THEN** both `name` and `id` are available with their declared types through the same reference

#### Scenario: Conflicting intersection property is not treated as either member alone
- **WHEN** two intersection constituents require incompatible types or incompatible mutable capabilities for the same property
- **THEN** the checker rejects construction or access that would violate either constituent and explains the conflict

#### Scenario: Union read type joins compatible common properties
- **WHEN** all alternatives provide a readable property with different compatible types
- **THEN** reading the property yields the normalized union of the property result types

#### Scenario: Union writes satisfy every alternative
- **WHEN** a common property is writable through a union value
- **THEN** a write is permitted only when the value is accepted by every possible destination and every alternative permits that mutation

#### Scenario: Common callable member does not create overloads
- **WHEN** a callable property is accessed before a union is narrowed
- **THEN** it is callable only through an argument/result contract safe for every alternative; incompatible signatures remain unavailable until narrowing

#### Scenario: Contract type arguments and method type arguments compose
- **WHEN** a generic method on `Contract<T>` refers both to its owner parameter `T` and its own method parameter `K`
- **THEN** member resolution substitutes the receiver's `T` first, infers `K` from the call arguments second, and checks constraints and indexed types against the composed substitution

#### Scenario: Contract implementation preserves method-generic constraints
- **WHEN** a class implements a parameterized contract whose method declares its own constrained type parameter
- **THEN** the implementation must provide a compatible generic signature and the checker rejects missing, weaker, or conflicting method constraints at the implementation

#### Scenario: Contextual structural literal has no unique solution
- **WHEN** a literal or generic relation produces zero or multiple valid structural candidates
- **THEN** the checker emits a focused mismatch or ambiguity diagnostic rather than choosing an arbitrary candidate
