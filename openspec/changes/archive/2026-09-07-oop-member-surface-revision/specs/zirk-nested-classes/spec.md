# zirk-nested-classes Delta Spec

## ADDED Requirements

### Requirement: Nested classes are static by default
A class body SHALL accept a `class` declaration as a member. A nested class declared without `inner` SHALL be a static nested class: an ordinary class whose canonical name is qualified by its enclosing class (`Outer.Nested`), with no reference to and no access to any enclosing instance. Nested classes SHALL be permitted to arbitrary depth, and member visibility modifiers SHALL apply to the nested class itself.

#### Scenario: Static nested class
- **WHEN** `class Outer { class Nested { } }` is declared and `Outer.Nested` is named in a type or construction position
- **THEN** `Outer.Nested` resolves as a normal class with no enclosing-instance state

#### Scenario: Nested class does not see the enclosing instance
- **WHEN** a method of a static nested class references a field of its enclosing class without qualification
- **THEN** a diagnostic is emitted, since there is no enclosing instance

#### Scenario: Nested class visibility
- **WHEN** a `private class` member of `Outer` is named from outside `Outer`
- **THEN** a diagnostic is emitted naming the member and its visibility

### Requirement: `inner` classes capture the enclosing instance
A nested class declared `inner class` SHALL carry a hidden `outer` reference to its enclosing class instance: its object layout SHALL include a hidden `outer` field and every constructor SHALL take a hidden leading enclosing-instance parameter. An inner class's methods SHALL be able to reference `outer` and access `private`/`protected` members of the enclosing class, and the enclosing class SHALL be able to access private members of its inner classes. An `inner` class SHALL NOT declare `static` members. `inner` SHALL be rejected on any declaration that is not a class member.

#### Scenario: Inner class reads the enclosing instance
- **WHEN** `class Outer { x: Int32; inner class Inner { f(): Int32 { return outer.x; } } }` is declared
- **THEN** `f` resolves `outer.x` to the enclosing instance's field

#### Scenario: Inner construction requires an enclosing instance
- **WHEN** `Inner()` is constructed outside `Outer` without providing an enclosing instance
- **THEN** a diagnostic is emitted indicating that an `Outer` instance is required

#### Scenario: Static member inside inner class
- **WHEN** an `inner class` declares a `static` field or method
- **THEN** a diagnostic is emitted

### Requirement: Local classes in bodies
A `class` declaration SHALL be a valid statement inside a function, method, or constructor body. A local class SHALL be scoped to its enclosing block, SHALL have a canonical name mangled with its enclosing function, and SHALL NOT capture enclosing local variables in this version. A local class SHALL obey all ordinary class rules (`extends`, `implements`, `#override`, `final`).

#### Scenario: Local class in a function body
- **WHEN** `fn f(): Void { class Tmp implements Nameable { name(): String { return "t"; } } let x: Nameable = Tmp(); }` is compiled
- **THEN** `Tmp` is visible only inside `f` and satisfies `Nameable` normally

#### Scenario: Local class used outside its function
- **WHEN** code outside `f` names the local class `Tmp`
- **THEN** a name-resolution diagnostic is emitted

#### Scenario: Local class captures an enclosing local
- **WHEN** a local class body references a local variable of the enclosing function
- **THEN** a diagnostic is emitted indicating local classes do not capture enclosing locals

### Requirement: Anonymous classes are not part of the language
Construction syntax with an attached class body (`Contract() { ... }`) SHALL be rejected with a diagnostic pointing to the idiomatic replacements: a local class for named multi-method implementations and a closure for single-method contracts.

#### Scenario: Anonymous class expression
- **WHEN** `Nameable() { name(): String { return "x"; } }` is parsed
- **THEN** a diagnostic is emitted recommending a local class or a closure
