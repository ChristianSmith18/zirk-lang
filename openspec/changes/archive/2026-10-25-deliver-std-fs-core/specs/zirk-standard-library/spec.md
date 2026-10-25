## MODIFIED Requirements

### Requirement: The standard library has one synchronized capability owner
The standard-library capability SHALL inventory every public `std` module and shared API contract defined by `docs/ZIRK_STDLIB_SPEC.md`. Its handbook owner pages SHALL use the same signatures, result variants, permissions, cancellation behavior, safety limits, and platform guarantees. The bundled `std.fs` and `std.path` packages SHALL be included in this inventory, and their status SHALL distinguish the synchronous bounded and progressive operations delivered by this change from directory, link, watcher, lock, and asynchronous operations deferred to a later change.

#### Scenario: A module API changes
- **WHEN** a normative standard-library signature or behavior changes
- **THEN** its specialized spec, OpenSpec capability, handbook owner, indexes, and affected examples are updated in the same change

#### Scenario: Filesystem module delivery is documented
- **WHEN** `std.fs` or `std.path` is delivered
- **THEN** the standard-library spec, filesystem capability, handbook pages, indexes, implementation status, and examples describe the same supported operations, permissions, limits, and target exclusions

#### Scenario: Deferred filesystem operations are not reported as delivered
- **WHEN** the standard-library status is published after this change
- **THEN** directory traversal and mutation, links, temporary paths, locks, watchers, and asynchronous filesystem operations remain identified as deferred until their companion change is implemented
