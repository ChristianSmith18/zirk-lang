## MODIFIED Requirements

### Requirement: Collection literal construction

`Array<T>` SHALL be constructible from an element listing (`Array(e0, e1, …)` or `[e0, e1, …]` with an `Array<T>` context), and `List<T>` SHALL be constructible from `List(e0, e1, …)` or `[e0, e1, …]` with a `List<T>` context. Any range element in either form SHALL expand in place before the collection is allocated. An unannotated `[...]` literal SHALL default to `Array<T>`.

#### Scenario: Array literal with a range
- **WHEN** `inmut a: Array<Int32> = [0..3]` is constructed
- **THEN** `a` contains `0`, `1`, and `2`

#### Scenario: List literal with a range
- **WHEN** `inmut l: List<Int32> = [0..3]` is constructed
- **THEN** `l` contains `0`, `1`, and `2` and remains resizable

### Requirement: Range<T> generic range and iteration

`Range<T>` SHALL represent a finite arithmetic sequence from `start` toward `end`. `start..end` SHALL exclude the endpoint and `start..=end` SHALL include it. An optional step SHALL use the colon form `start..end:step` or `start..=end:step`; the legacy second-`..` step form SHALL be rejected. Omitted steps SHALL be inferred from bound direction. Positive steps SHALL add their magnitude and negative steps SHALL subtract their magnitude. Zero steps SHALL produce `InvalidStepError`. Range construction SHALL require braces around every non-literal bound or step expression. `Range<T>` SHALL remain iterable and range values SHALL be usable in collection expansion.

#### Scenario: Numeric range
- **WHEN** `for i in 0..5 { stdout.println(i); }` is executed
- **THEN** it prints `0` through `4`

#### Scenario: Descending range infers a negative step
- **WHEN** `for i in 2..0 { stdout.println(i); }` is executed
- **THEN** it prints `2`, then `1`

#### Scenario: Range with a negative step
- **WHEN** `for i in 10..0:-2 { stdout.println(i); }` is executed
- **THEN** it prints `10`, `8`, `6`, `4`, and `2`

#### Scenario: Interpolated bound
- **WHEN** `inmut end: Int32 = 3;` and `for i in 0..{end} { }` are written
- **THEN** the bound is evaluated once and the loop iterates `0`, `1`, and `2`

#### Scenario: Range constructor expansion
- **WHEN** `List(0..=2)` is constructed
- **THEN** it is equivalent to `List(0, 1, 2)`

## REMOVED Requirements

### Requirement: Range builder methods

**Reason**: The colon-step range syntax makes `.step(...)` and `.reverse()` redundant and their coexistence creates conflicting direction semantics.

**Migration**: Replace `r.step(n)` with a range written as `start..end:n`, and replace `r.reverse()` with a range whose bounds and step are written in descending order.
