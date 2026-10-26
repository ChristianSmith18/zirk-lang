## ADDED Requirements

### Requirement: Semantic queries expose canonical composite types
Compiler-backed IDE queries SHALL expose stable canonical identities and display information for unions, intersections, singleton values, aliases, generic substitutions, structural properties, and the type selected by flow narrowing. Query results SHALL distinguish source alias spelling from canonical equivalence and SHALL belong to the analyzed snapshot.

#### Scenario: Alias and target share canonical identity
- **WHEN** a query inspects a transparent alias and its target
- **THEN** both identify the same canonical type while the alias spelling remains available for display

#### Scenario: Singleton type query preserves its value
- **WHEN** the checker infers a Date, String, numeric, or user-defined singleton
- **THEN** the query result carries its owning type and canonical constant value

#### Scenario: Query after control-flow narrowing
- **WHEN** a symbol is queried within a narrowed branch
- **THEN** the query returns the branch-specific type for that source snapshot

#### Scenario: Stale snapshot is rejected
- **WHEN** a query references an earlier document generation after a newer analysis supersedes it
- **THEN** the service does not return an outdated canonical or narrowed type
