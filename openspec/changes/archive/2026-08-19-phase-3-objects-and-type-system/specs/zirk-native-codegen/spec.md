## ADDED Requirements

### Requirement: Object layout

The backend SHALL translate a type with identity into a structure whose header precedes its fields, and whose inherited fields precede its own.

#### Scenario: Shared prefix
- **WHEN** a base class and a subclass are translated
- **THEN** the subclass's structure prefix matches the base's

#### Scenario: Inline value class
- **WHEN** a value class is a field of another declaration
- **THEN** it is translated with no intermediate pointer

### Requirement: Method tables

The backend SHALL emit a method table per type with virtual methods, and one per implemented interface.

#### Scenario: Stable index on inheritance
- **WHEN** a subclass inherits a virtual method
- **THEN** it occupies the same index as in its base's table

#### Scenario: Dispatch through an interface
- **WHEN** a method is called through an interface
- **THEN** the generated code looks up that interface's table in the descriptor and dispatches through it

### Requirement: Checked cast

The backend SHALL translate a checkable cast into a comparison of the value's type descriptor against the expected one, deferring to the runtime when it does not match.

#### Scenario: Cast that fails
- **WHEN** the descriptor does not correspond to the requested type
- **THEN** the program terminates with a diagnosed runtime error
- **AND** it does NOT incur undefined behavior

#### Scenario: Cast that needs no check
- **WHEN** the cast goes up the hierarchy, where the checker already proved it
- **THEN** the generated code includes no comparison at all
