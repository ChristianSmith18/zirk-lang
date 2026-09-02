## MODIFIED Requirements

### Requirement: Class syntax

The parser SHALL recognize `class`, its fields and methods, `construct`, `this`, the visibility modifiers, `abstract`, `extends`, and `implements`, per `ZIRK_LANGUAGE_SPEC.md` section 7.

#### Scenario: Complete class
- **WHEN** `class User implements Serializable { public inmut id: Int32; construct(id: Int32) { this.id = id; } }` is parsed
- **THEN** a declaration is produced with one implemented contract, one field, and one constructor

#### Scenario: Combined inheritance and contracts
- **WHEN** `class Admin extends User implements Auditable, Clone { }` is parsed
- **THEN** a declaration is produced with one superclass and two contracts

#### Scenario: `construct` outside a class
- **WHEN** `construct` appears at the file's top level
- **THEN** a diagnostic is emitted indicating that a constructor belongs to a class

#### Scenario: Multiple constructors and named arguments
- **WHEN** a class declares several `construct` overloads and is constructed with reordered named arguments
- **THEN** the tree preserves all signatures and each argument's labels for semantic resolution
