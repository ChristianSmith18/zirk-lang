## MODIFIED Requirements

### Requirement: `for ... in` over the minimal iteration protocol

The checker SHALL accept `for ... in` over integer ranges using exclusive, inclusive, ascending, descending, positive-step, negative-step, and braced dynamic operands. It SHALL reject non-integer range operands and unbraced variable/expression bounds. The loop binding SHALL have the range element type.

#### Scenario: Iteration over a descending range
- **WHEN** `for i in 2..0 { }` is written
- **THEN** `i` has integer type and the loop produces `2` and `1`

#### Scenario: Invalid range operand
- **WHEN** a range bound expression evaluates to `String`
- **THEN** a type diagnostic is emitted requiring an integer

### Requirement: Value, enum, array, iteration, and generator semantics

Array and List literals SHALL be contextually typed. An unannotated `[...]` SHALL infer `Array<T>`; an expected `List<T>` SHALL select `List<T>`. Range elements and range constructor arguments SHALL expand before element-type unification and collection allocation. Fixed declarations `T[n]` SHALL produce an `Array<T>` with exactly `n` slots.

#### Scenario: Contextual list typing
- **WHEN** `inmut xs: List<Int32> = [0..3]` is written
- **THEN** the literal type is `List<Int32>` and its length is `3`

#### Scenario: Fixed array typing
- **WHEN** `inmut xs: Int32[6];` is declared
- **THEN** `xs` has fixed `Array<Int32>` type and length `6`
