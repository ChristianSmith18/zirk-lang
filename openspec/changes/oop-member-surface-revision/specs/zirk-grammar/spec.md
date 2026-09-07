# zirk-grammar Delta Spec

## MODIFIED Requirements

### Requirement: Class syntax

The parser SHALL recognize `class`, its fields and methods, `construct`, `this`, the visibility modifiers, `abstract`, `final`, `extends`, `implements`, and nested class members, per `ZIRK_LANGUAGE_SPEC.md` section 7. A method inside a type body SHALL be written `name(params): Return { ... }` without `fn`; a field remains `name: Type;`. The parser SHALL disambiguate a member after its modifiers and identifier by lookahead: `(` or `<` opens a method, `:` opens a field, `class` opens a nested class.

#### Scenario: Complete class
- **WHEN** `class User implements Serializable { public inmut id: Int32; construct(id: Int32) { this.id = id; } }` is parsed
- **THEN** a declaration with one implemented contract, one field, and one constructor is produced

#### Scenario: Method without `fn`
- **WHEN** `class C { greet(): String { return "hi"; } }` is parsed
- **THEN** a method member is produced

#### Scenario: `fn` inside a type body
- **WHEN** `class C { fn greet(): String { return "hi"; } }` is parsed
- **THEN** a diagnostic is emitted indicating methods no longer use `fn`

#### Scenario: Function-typed field is not a method
- **WHEN** `class C { handler: fn(Int32): Void; }` is parsed
- **THEN** a field whose type is a function type is produced, not a method

#### Scenario: Combined inheritance and contracts
- **WHEN** `class Admin extends User implements Auditable, Clone { }` is parsed
- **THEN** a declaration with one superclass and two contracts is produced

#### Scenario: `construct` outside a class
- **WHEN** `construct` appears at the top level of the file
- **THEN** a diagnostic indicating that a constructor belongs to a class is emitted

#### Scenario: Multiple constructors and named arguments
- **WHEN** a class declares several `construct` and is constructed with reordered named arguments
- **THEN** the tree retains all signatures and each argument's labels for semantic resolution

#### Scenario: Nested class member
- **WHEN** `class Outer { class Inner { } inner class Bound { } }` is parsed
- **THEN** two nested class members are produced, the second marked `inner`

#### Scenario: Local class statement
- **WHEN** `fn f(): Void { class Local { } }` is parsed
- **THEN** a class declaration statement inside the body is produced

### Requirement: Contract syntax

The parser SHALL recognize `interface` and `trait` with their methods — written without `fn` — and allow a body only in those of a `trait`.

#### Scenario: Interface
- **WHEN** `interface Serializable { serialize(): String; }` is parsed
- **THEN** a declaration with a signature without a body is produced

#### Scenario: Trait with implementation
- **WHEN** `trait Greet { hello(): String { return "hello"; } }` is parsed
- **THEN** a declaration whose method has a body is produced

## ADDED Requirements

### Requirement: Member marker syntax
The parser SHALL recognize `#name` member markers on their own prefix position before a class member's modifiers. `#override` SHALL be the only defined marker in this change; an unrecognized `#name` SHALL produce a diagnostic. Markers SHALL apply only to instance methods.

#### Scenario: Override marker
- **WHEN** a class member begins with `#override` followed by a method header
- **THEN** the method is marked as an override

#### Scenario: Unknown marker
- **WHEN** a class member begins with `#unknown`
- **THEN** a diagnostic is emitted indicating `#override` is the only defined member marker

#### Scenario: Marker on a field
- **WHEN** `#override` precedes a field declaration
- **THEN** a diagnostic is emitted indicating markers apply only to methods

### Requirement: `final` modifier positions
The parser SHALL accept `final` before `class` and in the member-modifier sequence before a method. `final` in any other position (fields, constructors, `interface`, `trait`, `record`, parameters, variables) SHALL produce a targeted diagnostic.

#### Scenario: Final class and method
- **WHEN** `final class A { }` and `class B { final m(): Void { } }` are parsed
- **THEN** the final flags are recorded on the class and the method

#### Scenario: Final field rejected
- **WHEN** a class body contains `final x: Int32;`
- **THEN** a diagnostic is emitted indicating attributes use `inmut`, not `final`
