## MODIFIED Requirements

### Requirement: Iteration via contract

`for ... in` SHALL use the `Iterable<T>` contract for arrays, lists, strings, and ranges. Range iteration SHALL expose the normalized sequence produced by the new colon-step and inferred-direction rules; no `.step()` or `.reverse()` method dispatch SHALL be required for range iteration.

#### Scenario: Range iteration through Iterable
- **WHEN** `for i in 0..=2 { }` is written
- **THEN** the range supplies an `Iterable<Int32>` sequence containing `0`, `1`, and `2`

#### Scenario: Expanded collection remains iterable
- **WHEN** `inmut values: List<Int32> = [0..3]` is iterated
- **THEN** iteration yields `0`, `1`, and `2` in order
