# zirk-memory-safety

## MODIFIED Requirements

### Requirement: Strategy-neutral automatic memory
Zirk SHALL reclaim unreachable managed memory including cycles without exposing GC, ownership, regions, moves, or reference counting as mandatory source semantics, and representation changes MUST preserve identity and observable lifetime. This guarantee covers all runtime-allocated opaque handles, including `String` and `Char` values.

#### Scenario: Runtime moves an object
- **WHEN** the runtime relocates a live managed object
- **THEN** safe references remain valid and `is` observes the same identity

#### Scenario: A string becomes unreachable
- **WHEN** a `String` value produced by concatenation, conversion, or slicing is no longer reachable from any root
- **THEN** the collector reclaims both the handle and its bytes

#### Scenario: A character becomes unreachable
- **WHEN** a `Char` value produced by `String[index]` or `Char` literal materialization is no longer reachable from any root
- **THEN** the collector reclaims the character object
