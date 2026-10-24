# zirk-classes Specification

## Purpose
Defines class attributes, construction, inheritance, virtual dispatch,
abstract requirements, overrides, and object-oriented conformance.
## Requirements

### Requirement: Attributes, defaults, and projection behavior

Classes SHALL expose attributes and ordinary methods, SHALL NOT expose a property declaration, and SHALL use conventional `get_`/`set_` methods when accessors are desired. An attribute MAY declare an explicit default initializer `field: Type = expr;`; the expression SHALL NOT reference `this` and SHALL be evaluated in declaration order during construction, before the constructor body, only for fields the constructor does not assign. Attributes without an initializer SHALL receive their type default. Reading a reference-valued attribute SHALL deep-clone an independent value, while a whole class variable SHALL share identity and an attribute place SHALL mutate original storage.

#### Scenario: Attribute getter is ordinary method

- **WHEN** a class declares `get_name(): String` and a caller reads the name
- **THEN** the caller invokes `user.get_name()` and no `user.name` property dispatch is synthesized

#### Scenario: Nested reference projection

- **WHEN** `mut name = user.name` is evaluated and `name` is mutated
- **THEN** `user.name` remains unchanged because extraction deep-cloned the String

#### Scenario: Field with explicit default

- **WHEN** `count: Int32 = 0;` is declared and a constructor does not assign `count`
- **THEN** the constructed instance has `count == 0`

#### Scenario: Constructor assignment wins over the default

- **WHEN** `count: Int32 = 0;` is declared and the constructor assigns `this.count = 5`
- **THEN** the constructed instance has `count == 5`

#### Scenario: Default cannot see the instance

- **WHEN** a field initializer references `this`
- **THEN** a diagnostic is emitted

### Requirement: Object casts and identity

`is` SHALL compare reference identity, `==` SHALL require equality capability, `as` SHALL perform a checked related-type cast with controlled failure, and `as?` SHALL return nullable absence on failure. Unrelated casts SHALL fail at compile time.

#### Scenario: Optional downcast

- **WHEN** a User reference is evaluated with `as? Admin`
- **THEN** the result is the same Admin reference when compatible or `null` otherwise

### Requirement: Class declaration

The compiler SHALL support `class` with fields and methods, per `ZIRK_LANGUAGE_SPEC.md` section 7. Methods inside a class body SHALL NOT use `fn`: a method is `name(params): Return { ... }` and a field is `name: Type;`. Writing `fn` inside a class body SHALL produce a diagnostic with migration guidance.

#### Scenario: Class with fields and constructor

- **WHEN** `class User { public inmut id: Int32; construct(id: Int32) { this.id = id; } }` is declared
- **THEN** a `User` type is produced with one field and one constructor

#### Scenario: Field without modifiers

- **WHEN** a class declares `name: String;`
- **THEN** the field is equivalent to `public mut name: String;`

#### Scenario: Multiple constructors

- **WHEN** a class declares two `construct` with distinct effective signatures
- **THEN** both are valid constructors and remain available for resolution

#### Scenario: Instantiation without `new`

- **WHEN** `mut u = User(1);` is written
- **THEN** an instance is constructed
- **AND** writing `new User(1)` emits a diagnostic indicating that `new` does not exist

#### Scenario: Field without a type

- **WHEN** a field is declared without a type annotation
- **THEN** a diagnostic is emitted pointing to the field

#### Scenario: Method without `fn`

- **WHEN** a class declares `bump(): Int32 { return this.count; }`
- **THEN** a method is produced and `this.count` resolves normally

#### Scenario: `fn` inside a class body

- **WHEN** a class body contains `fn bump(): Int32 { ... }`
- **THEN** a diagnostic is emitted indicating methods no longer use `fn`

### Requirement: `this` designates the current instance

Within a method or constructor, `this` SHALL refer to the instance on which it is invoked.

#### Scenario: Accessing a field via `this`

- **WHEN** a constructor writes `this.id = id;`
- **THEN** it assigns to the field, not to the parameter

#### Scenario: `this` outside a class

- **WHEN** `this` appears in a top-level function
- **THEN** a diagnostic is emitted indicating that there is no instance

### Requirement: Member visibility

A member SHALL support `public`, `private`, or `protected`, with `public` as the default, per `ZIRK_LANGUAGE_SPEC.md` section 7.

#### Scenario: Private member accessed from outside

- **WHEN** a `private` field is accessed from outside its class
- **THEN** a diagnostic is emitted naming the member and its visibility

#### Scenario: Protected member accessed from a subclass

- **WHEN** a subclass accesses a `protected` member of its superclass
- **THEN** the access is valid

#### Scenario: Default visibility

- **WHEN** a member is declared without a modifier
- **THEN** it is `public`

### Requirement: Single inheritance

A class SHALL extend at most one class. Classes SHALL be inheritable by default, and `final class` SHALL mark a class as non-inheritable.

#### Scenario: Subclass inherits fields and methods

- **WHEN** `class Admin extends User` does not declare `id`
- **THEN** an instance of `Admin` has the `id` field from `User`

#### Scenario: Multiple class inheritance

- **WHEN** a class attempts to extend two classes
- **THEN** a diagnostic is emitted indicating that class inheritance is single

#### Scenario: Inheritance cycle

- **WHEN** two classes extend each other
- **THEN** a diagnostic is emitted showing the cycle

#### Scenario: Extending a final class

- **WHEN** `class Sub extends Final` where `Final` is declared `final class`
- **THEN** a diagnostic is emitted indicating `Final` cannot be extended

### Requirement: Method overriding

A subclass SHALL be able to override a method of its superclass with the same signature, written with the `#override` marker, and the call SHALL resolve to the runtime type's definition. A `final` method SHALL NOT be overridden.

#### Scenario: Dispatch to the override

- **WHEN** a variable declared with the base type holds an instance of the subclass and the overridden method is called
- **THEN** the subclass's method is executed

#### Scenario: Override with a different signature

- **WHEN** a subclass overrides a method while changing parameters or return type
- **THEN** a diagnostic is emitted showing both signatures

#### Scenario: Overriding a final method

- **WHEN** a subclass declares `#override` on a method that is `final` in the base
- **THEN** a diagnostic is emitted indicating the base method is `final`

### Requirement: Abstract classes and methods

`abstract` SHALL be supported on classes and methods. An abstract class SHALL NOT be instantiated, and a concrete class SHALL implement every abstract method it inherits — with a plain method, without `#override`, since no implementation is replaced.

#### Scenario: Instantiating an abstract class

- **WHEN** an instance of an `abstract` class is constructed
- **THEN** a diagnostic is emitted indicating that it is not instantiable

#### Scenario: Unimplemented abstract method

- **WHEN** a concrete class inherits an `abstract` method and does not implement it
- **THEN** a diagnostic is emitted naming the method

#### Scenario: Abstract method with a body

- **WHEN** an `abstract` method declares a body
- **THEN** a diagnostic is emitted pointing to the body

#### Scenario: `final` on an abstract class

- **WHEN** a class is declared `abstract final class`
- **THEN** a diagnostic is emitted indicating the modifiers are contradictory

### Requirement: `mut` is not a method modifier

`mut` SHALL NOT be accepted before a method declaration inside any type body. Receiver mutability is a compiler-inferred property used by `inmut::strict` checking (see `zirk-memory-safety`), not a written modifier. `mut`/`inmut`/`inmut::strict` continue to apply to fields, variables, and parameters unchanged.

#### Scenario: `mut` before a method

- **WHEN** a class body contains `mut bump(): Int32 { ... }`
- **THEN** a diagnostic is emitted indicating `mut` no longer applies to methods and that mutation is inferred

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

### Requirement: Classes implement structural contracts
Classes SHALL be able to implement multiple contracts by providing compatible required properties and operations. Contract implementation SHALL not alter class inheritance, abstract-class conformance, or trait default conflict rules. Implementing a contract operation SHALL NOT require `#override` unless it also replaces an inherited implementation.

#### Scenario: Class implements a contract
- **WHEN** a class implements a contract and declares every required member
- **THEN** the class is accepted through that contract type

#### Scenario: Contract implementation does not require override marker
- **WHEN** a class supplies a contract operation without replacing an inherited implementation
- **THEN** the implementation is accepted without `#override`

### Requirement: Static members remain class-only
Classes MAY declare static methods and fields. Static members SHALL be accessed through the class name, SHALL NOT dispatch virtually or appear in instance contract tables, and SHALL NOT use `this`. Visibility and mutability rules SHALL apply as for other class members.

#### Scenario: Static method resolves on the class
- **WHEN** `Counter.make()` calls a declared static method
- **THEN** dispatch is static and requires no instance

#### Scenario: Static body rejects this
- **WHEN** a static method references `this`
- **THEN** the checker emits a diagnostic

### Requirement: Abstract classes remain nominal requirements
An abstract class SHALL define nominal required attributes and abstract method signatures without constructor, method body, state allocation, or layout contribution. A concrete subclass SHALL implement those requirements and SHALL dispatch calls through an abstract-class view to its concrete implementation. Classes MAY extend one concrete class.

#### Scenario: Concrete subclass satisfies abstract requirements
- **WHEN** a subclass implements every abstract method and required attribute
- **THEN** it can be used through the abstract-class type

### Requirement: Override markers select inherited implementations
`#override` SHALL be required only when replacing an inherited class implementation or trait default. Implementing a contract signature without replacing inherited behavior SHALL NOT require the marker. Inherited calls through `super` and explicit trait-default selection SHALL retain their existing behavior.

#### Scenario: Contract method needs no override marker
- **WHEN** a class supplies a contract operation that has no inherited implementation
- **THEN** it compiles without `#override`

### Requirement: Class instances support dynamic properties
Class instances SHALL support dynamic properties in addition to declared class fields. Unknown named access SHALL resolve to a dynamic property, and computed string access SHALL read or write dynamic properties. Dynamic property addition, replacement, and removal SHALL follow the instance reference's mutability and SHALL be disallowed after contract conformance seals the instance's key set. Declared class fields SHALL retain their declared types and visibility rules.

#### Scenario: Mutable class instance adds a dynamic property
- **WHEN** a mutable class instance receives a key not declared as a class field
- **THEN** that key is stored as a dynamic property without changing the class's declared layout or members

#### Scenario: Declared field keeps its declared type
- **WHEN** a dynamic property and a declared class field share the same spelling
- **THEN** declared-member resolution takes precedence and enforces the class field type

#### Scenario: Unknown member reads a dynamic property
- **WHEN** a class instance has a dynamic property not declared in its class
- **THEN** unknown-member access reads that property as `Dynamic` rather than producing a compile-time member-not-found error

#### Scenario: Contract seals class instance keys
- **WHEN** a class instance is successfully checked against a contract
- **THEN** its existing dynamic key set cannot be changed through any alias
