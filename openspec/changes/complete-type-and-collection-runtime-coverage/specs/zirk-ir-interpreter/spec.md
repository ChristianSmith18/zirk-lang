## ADDED Requirements

### Requirement: Interpreter executes every admitted composite and collection layout
The verified-IR interpreter SHALL execute every composite representation and generic collection layout admitted by semantic analysis using its canonical type and active-variant metadata. It SHALL preserve the same scalar bits, equality and hashing outcomes, insertion and priority ordering, reference identity, checked errors, and observable mutation semantics as native execution. Invalid or unsupported IR SHALL be rejected by verification; the interpreter SHALL NOT guess a layout or coerce an aggregate to a host word.

#### Scenario: Aggregate Map and Set values match native execution
- **WHEN** a fixture stores and retrieves tuples, nullable values, payload enums, unions, and nested managed references through `Map` or `Set`
- **THEN** interpreter and native runs produce equivalent values, membership outcomes, iteration order, and failures

#### Scenario: Deque, priority queue, queue, and stack match native behavior
- **WHEN** the same ordered operation sequence runs against each new collection family in both engines
- **THEN** returned values, empty outcomes, stable ties, and final iteration order are equivalent

#### Scenario: Composite alternatives match through all value boundaries
- **WHEN** a union or intersection is constructed, assigned, passed, returned, captured, stored in a collection, narrowed, and matched
- **THEN** both engines preserve alternative identity, payload values, and object identity

#### Scenario: Invalid layout is rejected before interpretation
- **WHEN** verified IR contains a collection element or composite payload whose type layout does not match its operation
- **THEN** verification rejects the module before interpreter execution
