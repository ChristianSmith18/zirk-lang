## ADDED Requirements

### Requirement: Promoted constructor parameters declare fields

A `construct` parameter carrying member modifiers SHALL declare the
equivalent attribute — same name, declared type, and written modifiers —
and SHALL assign `this.name = name` at the start of the constructor body,
after any `super(...)` call and after field initializers. The parameter
SHALL remain usable as an ordinary parameter inside the body, and the
resulting field SHALL be an ordinary field for member access, visibility,
dispatch, named construction arguments, and tool-provided symbol
information. A promoted `inmut` field SHALL be finalized by its generated
assignment under the same rules a hand-written constructor follows.

#### Scenario: Short and long forms are equivalent

- **WHEN** `construct(public name: String, public age: Int)` is used and
  the same class is written with explicit fields plus manual assignments
- **THEN** both forms produce instances indistinguishable in field layout,
  visibility, and behavior

#### Scenario: Modifier combinations

- **WHEN** `construct(private inmut id: UInt64)` is declared
- **THEN** the field is `private inmut` and readable only inside the class

#### Scenario: Promoted field access from another method

- **WHEN** a promoted field is read or written through `this.` inside a
  method other than its declaring constructor
- **THEN** a warning suggests declaring the attribute explicitly for
  legibility
- **AND** compilation still succeeds

#### Scenario: Parameter still usable in the body

- **WHEN** a promoted parameter is referenced inside its own constructor
  body
- **THEN** it reads the argument value like any other parameter

#### Scenario: Promoted field receives no separate type default

- **WHEN** a class promotes `name: String` and `User("Ana")` runs
- **THEN** `name` holds `"Ana"` — the parameter argument is the field's
  initializer, not a type default overwritten later

### Requirement: Promotion conflicts are compile-time errors

Promotion SHALL be rejected when it would make construction ambiguous or
duplicate a member: a promoted parameter naming an already-declared field
is an error, and promotion is an error in a class that declares more than
one `construct`, because no single constructor would own the fields. The
multi-constructor diagnostic SHALL advise keeping one constructor or
declaring the attributes explicitly.

#### Scenario: Field declared and promoted

- **WHEN** a class declares `name: String;` and its constructor promotes
  `public name: String`
- **THEN** compilation fails indicating the field is declared twice

#### Scenario: Promotion with multiple constructors

- **WHEN** a class declares two `construct` members and at least one uses
  promotion
- **THEN** compilation fails with a diagnostic naming the ambiguity and
  advising a single constructor or explicit attribute declarations

#### Scenario: Multiple constructors without promotion stay legal

- **WHEN** a class declares two `construct` members with distinct
  signatures and no promoted parameters
- **THEN** both constructors remain valid

### Requirement: Implicit default constructor

A class that declares no `construct` member SHALL behave as if it declared
`construct() {}`. Declaring any `construct` member SHALL remove the
implicit constructor entirely. The implicit constructor SHALL be invisible:
it SHALL NOT appear in symbol lists, outlines, or generated documentation,
and no tool SHALL materialize it into source.

#### Scenario: Instantiate a class without a declared constructor

- **WHEN** `class OmitConstruct {}` is declared and `OmitConstruct()` is
  written
- **THEN** an instance is produced whose fields hold their declared or
  type defaults

#### Scenario: Arguments to an implicit constructor

- **WHEN** `OmitConstruct(1)` is written for a class with no declared
  constructor
- **THEN** a diagnostic reports that no constructor takes one argument

#### Scenario: Declared constructor removes the implicit one

- **WHEN** `class A { construct(x: Int) { ... } }` is declared and `A()` is
  written
- **THEN** a diagnostic reports that no constructor takes zero arguments

#### Scenario: Implicit constructor in a subclass with a no-argument base

- **WHEN** `class B extends A {}` is declared and `A` has an accessible
  no-argument constructor
- **THEN** `B()` invokes the base constructor through an implicit `super()`
- **AND** a warning suggests writing the constructor and `super()`
  explicitly

#### Scenario: Implicit constructor cannot satisfy a base constructor

- **WHEN** `class B extends A {}` is declared and `A` only declares
  constructors that require arguments
- **THEN** writing `B()` produces a diagnostic at the instantiation site
  naming the unsatisfiable base construction

#### Scenario: Implicit constructor stays invisible

- **WHEN** a tool lists the members of `class OmitConstruct {}`
- **THEN** no constructor entry is shown

### Requirement: Sugar survives tooling unchanged

Source-level tooling SHALL preserve the sugared form: the formatter SHALL
NOT expand promoted parameters into field declarations nor materialize the
implicit constructor, and refactorings SHALL treat promoted fields as
fields declared at their parameter position.

#### Scenario: Formatting keeps the short form

- **WHEN** `zirk format` processes `construct(public name: String)` or
  `class Foo {}`
- **THEN** the output keeps the sugared form without synthesized members
