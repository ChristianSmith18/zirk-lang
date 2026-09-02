## ADDED Requirements

### Requirement: Object lowering

Lowering SHALL translate an object's construction to the abstract allocation operation followed by initializing its fields, and a field access to a read by offset.

#### Scenario: Construction
- **WHEN** `User(1)` is lowered
- **THEN** the type's abstract allocation is emitted
- **AND** the constructor's body initializes the fields

#### Scenario: Field access
- **WHEN** `u.id` is lowered
- **THEN** a read at the field's offset is emitted, with no lookup by name

### Requirement: Method call lowering

Lowering SHALL emit a direct call when the method is not overridable, and an indirect call through the type's table when it is.

Most calls are the first case, and paying an indirection for all of them would be paying for generality the program does not use.

#### Scenario: Non-overridden method
- **WHEN** no subclass overrides the called method
- **THEN** a direct call is emitted

#### Scenario: Overridden method
- **WHEN** some subclass overrides it
- **THEN** an indirect call through the type's table is emitted

#### Scenario: Call through a contract
- **WHEN** the receiver has an interface's type
- **THEN** it is dispatched through that interface's table

### Requirement: Safe access lowering

Lowering SHALL translate `expr?.member` to an explicit nullity check with two blocks: the present one accesses the member and the absent one produces the null value.

It reuses the mechanism `??` introduced in the previous phase; what arrives is the operator, not the machinery.

#### Scenario: Safe access
- **WHEN** `usuario?.nombre` is lowered
- **THEN** a nullity check with one block per outcome is produced

#### Scenario: Chained
- **WHEN** `a?.b?.c` is lowered
- **THEN** each link checks before accessing

### Requirement: Generic specialization

Lowering SHALL produce one function per combination of type arguments used, and reuse it when the combination repeats.

#### Scenario: One copy per combination
- **WHEN** a generic function is used with two different combinations
- **THEN** the IR contains two functions

#### Scenario: No duplicates
- **WHEN** the same combination is used several times
- **THEN** the IR contains a single function for it
