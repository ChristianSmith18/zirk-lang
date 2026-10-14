## ADDED Requirements

### Requirement: Collection `length` is usable in loop-bound comparisons

Every collection `length` member SHALL return a non-negative integer type that participates in the language's numeric comparison rules, so that the idiomatic `for mut i = 0; i < xs.length; i++` loop compiles and runs without explicit casts or counter-type annotations. The declared `length` types across `Array`, `List`, `Map`, and `Set` SHALL be consistent: either all signed or all unsigned, and the handbook SHALL document the same type for each.

#### Scenario: Index loop over an array
- **WHEN** `for mut i = 0; i < courses.length; i++ { stdout.println(courses[i]); }` is compiled
- **THEN** no type-mismatch diagnostic is emitted and each element is printed in order

#### Scenario: Index loop over a list
- **WHEN** the same loop pattern iterates a `List<T>` by index
- **THEN** it compiles and runs identically to the array case

#### Scenario: Documented `length` types agree
- **WHEN** the handbook entries for `Array`, `List`, `Map`, and `Set` are compared
- **THEN** they declare `length` with a consistent signedness across all four families
