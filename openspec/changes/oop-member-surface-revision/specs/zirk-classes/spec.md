# zirk-classes Delta Spec

## MODIFIED Requirements

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

### Requirement: Static members
A class MAY declare `static` methods and `static` fields. Static members SHALL be accessed as `ClassName.member`, SHALL NOT dispatch virtually, SHALL NOT appear in vtables or contract tables, and `this` SHALL be an error inside a static body. The usual visibility rules (`public`, `private`, `protected`) and mutability rules (`mut`/`inmut`) apply unchanged. Static members SHALL be allowed on classes; whether `record` or `enum` may declare them is left to a later decision.

#### Scenario: Static method call
- **WHEN** `class Counter { static make(): Counter { ... } }` is declared and `Counter.make()` is written
- **THEN** the call resolves statically to `Counter`'s method, with no receiver instance required

#### Scenario: Static field access
- **WHEN** `class Limits { static inmut max: Int32 = 100; }` is declared and `Limits.max` is read
- **THEN** the read resolves to the class-level field

#### Scenario: No receiver in static context
- **WHEN** a `static` method body references `this`
- **THEN** a diagnostic is emitted

#### Scenario: Static members are not virtual
- **WHEN** a subclass redeclares a `static` method of its base
- **THEN** no override relationship is created and calls resolve by the static type name used

### Requirement: Concrete inheritance and abstract implementation
A concrete class MAY extend at most one concrete class. An `abstract class` SHALL be a nominal set of required attributes and method signatures with no constructor, body, state allocation, or layout contribution, and concrete classes SHALL adopt it through `implements`. Interfaces and traits SHALL also use `implements`. A value statically typed as an `abstract class`, holding a concrete adopter instance, SHALL dispatch every call to the adopter's own implementation.

#### Scenario: Abstract class contract
- **WHEN** `class Circle implements Shape` supplies every attribute and method required by abstract class `Shape`
- **THEN** Circle is accepted where Shape is required without inheriting Shape storage

#### Scenario: Dynamic dispatch through an abstract-class-typed value
- **WHEN** a variable statically typed as abstract class `Shape` holds a `Circle` instance and a caller invokes a `Shape`-declared method on it
- **THEN** the call dispatches to `Circle`'s own implementation, the same way dispatch through a concrete base class already works

### Requirement: Explicit overriding and super dispatch
Only a method marked `#override` SHALL replace an inherited implementation: a concrete base-class method or a trait default. `#override` SHALL be written on its own member-marker line before the method and SHALL apply only to instance methods. Implementing a signature-only requirement (`abstract class` or `interface`) SHALL NOT use `#override`; writing it there SHALL produce a warning that the marker is unnecessary. Writing `#override` when no inherited implementation is replaced SHALL be an error. Public/protected instance methods SHALL dispatch virtually by default; private/static methods SHALL not. `super(...)` and `super.method()` SHALL address the concrete base, and `TraitName.super.method()` SHALL resolve a trait conflict.

#### Scenario: Missing override marker
- **WHEN** a subclass redeclares a compatible base method implementation without `#override`
- **THEN** compilation fails with both declarations identified

#### Scenario: Marker on a signature-only requirement
- **WHEN** a class implementing an `interface` writes `#override` above the implementing method
- **THEN** a warning is emitted indicating the marker is unnecessary and should be removed

#### Scenario: Marker that overrides nothing
- **WHEN** `#override` precedes a method that replaces no inherited implementation
- **THEN** compilation fails indicating nothing is overridden

#### Scenario: Marker on a non-method member
- **WHEN** `#override` precedes a field, constructor, nested class, or `static` member
- **THEN** a diagnostic is emitted indicating `#override` applies only to instance methods

### Requirement: Class declaration

The compiler SHALL support `class` with fields and methods, per `ZIRK_LANGUAGE_SPEC.md` section 7. Methods inside a class body SHALL NOT use `fn`: a method is `name(params): Return { ... }` and a field is `name: Type;`. Writing `fn` inside a class body SHALL produce a diagnostic with migration guidance.

#### Scenario: Class with fields and constructor
- **WHEN** `class User { public inmut id: Int32; construct(id: Int32) { this.id = id; } }` is declared
- **THEN** a `User` type is produced with one field and one constructor

#### Scenario: Method without `fn`
- **WHEN** a class declares `bump(): Int32 { return this.count; }`
- **THEN** a method is produced and `this.count` resolves normally

#### Scenario: `fn` inside a class body
- **WHEN** a class body contains `fn bump(): Int32 { ... }`
- **THEN** a diagnostic is emitted indicating methods no longer use `fn`

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

## ADDED Requirements

### Requirement: `mut` is not a method modifier
`mut` SHALL NOT be accepted before a method declaration inside any type body. Receiver mutability is a compiler-inferred property used by `inmut::strict` checking (see `zirk-memory-safety`), not a written modifier. `mut`/`inmut`/`inmut::strict` continue to apply to fields, variables, and parameters unchanged.

#### Scenario: `mut` before a method
- **WHEN** a class body contains `mut bump(): Int32 { ... }`
- **THEN** a diagnostic is emitted indicating `mut` no longer applies to methods and that mutation is inferred
