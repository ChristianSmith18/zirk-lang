# zirk-decorators Delta Spec

## MODIFIED Requirements

### Requirement: Overrides do not inherit decorator applications
An inherited method that is not overridden SHALL retain its existing decorated wrapper. A newly overriding method (written with the `#override` member marker) SHALL NOT inherit decorator applications automatically and SHALL require explicit reapplication when the behavior is desired.

#### Scenario: Decorated method is overridden without annotation
- **WHEN** a subclass uses `#override` without repeating its base method's decorator
- **THEN** the override contains no newly inherited wrapper while the base implementation remains decorated

## ADDED Requirements

### Requirement: `#` member markers are not decorators
`#name` member markers (`#override`, and future built-in markers) SHALL be a distinct syntactic surface from `@name` decorator applications. A `#` marker SHALL NOT be resolved through `fn dec` lookup, SHALL NOT participate in decorator expansion, and SHALL NOT require the decorator pipeline to exist.

#### Scenario: Marker resolves without decorators
- **WHEN** `#override` appears on a method in a program that declares no decorators
- **THEN** compilation proceeds normally with no decorator resolution attempted
