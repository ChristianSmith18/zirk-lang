# Delta spec: zirk-grammar

## MODIFIED Requirements

### Requirement: Data type syntax

The parser SHALL recognize `record`, enum variants with associated data,
unions `A | B`, and aliases with `type`.

#### Scenario: Enum with associated data
- **WHEN** `enum Shape { Circle(Int32), Rect(Int32, Int32) }` is parsed
- **THEN** two variants with one and two associated types are produced

#### Scenario: Traditional enum mapping
- **WHEN** `enum Direction { North -> "N", South }` is parsed
- **THEN** `North` retains its explicit mapping and `South` remains without an explicit mapping

#### Scenario: Pattern with destructuring
- **WHEN** `match s { Shape.Circle(r) => r, _ => 0 }` is parsed
- **THEN** the pattern binds a name to the associated value

#### Scenario: Union
- **WHEN** `mut x: Int32 | String;` is parsed
- **THEN** a type with two members is produced

#### Scenario: Alias
- **WHEN** `type Id = Int32;` is parsed
- **THEN** an alias declaration is produced
