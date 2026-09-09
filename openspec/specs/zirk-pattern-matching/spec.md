# zirk-pattern-matching Specification

## Purpose
Defines exhaustive guard-free matching, supported pattern families, binding
copy semantics, reachability, and match-expression typing.
## Requirements
### Requirement: Exhaustive guard-free matching
Every statement or expression match over a closed domain SHALL be exhaustive. Patterns SHALL NOT contain guards. Expression branch results SHALL unify with Never compatible with every result; statement match SHALL produce Void.

#### Scenario: Missing statement enum case
- **WHEN** a statement match omits one closed enum case and has no wildcard
- **THEN** compilation fails with the missing case identified

#### Scenario: Guard rejected
- **WHEN** a match branch appends `if condition` to a pattern
- **THEN** parsing fails and conditional logic must be placed in the branch body

### Requirement: Pattern forms and reachability
Match SHALL support compatible values, types, enum variants, union alternatives, full-match regex literals, comma-grouped alternatives, wildcards, and nested record/payload patterns. A demonstrably unreachable later pattern SHALL be an error; uncertain regex overlap SHALL preserve source order without a false diagnostic.

#### Scenario: Catch-all shadows branch
- **WHEN** `_` precedes a later literal branch
- **THEN** compilation fails because the later branch is unreachable

### Requirement: Pattern binding copy semantics
Every attribute, tuple element, or enum payload bound by a pattern SHALL be an independent logical value and SHALL require deep Clone for reference data. Records and tuples MAY be destructured directly when irrefutable; algebraic enums SHALL only be unpacked inside match. Ordered collection patterns and record patterns MAY contain one final `...name` rest binding that collects the remaining elements or fields.

#### Scenario: Enum destructuring outside match
- **WHEN** `inmut Ready(document) = state` is declared
- **THEN** compilation fails and recommends exhaustive match
