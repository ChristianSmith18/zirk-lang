# zirk-classes Specification

## Purpose
Defines class attributes, construction, inheritance, virtual dispatch,
abstract requirements, overrides, and object-oriented conformance.
## Requirements
### Requirement: Attributes, defaults, and projection behavior
Classes SHALL expose attributes and ordinary methods, SHALL NOT expose a property declaration, and SHALL use conventional `get_`/`set_` methods when accessors are desired. Omitted attributes SHALL receive their type default. Reading a reference-valued attribute SHALL deep-clone an independent value, while a whole class variable SHALL share identity and an attribute place SHALL mutate original storage.

#### Scenario: Attribute getter is ordinary method
- **WHEN** a class declares `fn get_name(): String` and a caller reads the name
- **THEN** the caller invokes `user.get_name()` and no `user.name` property dispatch is synthesized

#### Scenario: Nested reference projection
- **WHEN** `mut name = user.name` is evaluated and `name` is mutated
- **THEN** `user.name` remains unchanged because extraction deep-cloned the String

### Requirement: Concrete inheritance and abstract implementation
A concrete class MAY extend at most one concrete class. An `abstract class` SHALL be a nominal set of required attributes and `abstract fn` signatures with no constructor, body, state allocation, or layout contribution, and concrete classes SHALL adopt it through `implements`. Interfaces and traits SHALL also use `implements`. A value statically typed as an `abstract class`, holding a concrete adopter instance, SHALL dispatch every call to the adopter's own override.

#### Scenario: Abstract class contract
- **WHEN** `class Circle implements Shape` supplies every attribute and `override fn` required by abstract class `Shape`
- **THEN** Circle is accepted where Shape is required without inheriting Shape storage

#### Scenario: Dynamic dispatch through an abstract-class-typed value
- **WHEN** a variable statically typed as abstract class `Shape` holds a `Circle` instance and a caller invokes a `Shape`-declared method on it
- **THEN** the call dispatches to `Circle`'s own override, the same way dispatch through a concrete base class already works

### Requirement: Explicit overriding and super dispatch
Only `override fn` SHALL replace a base, abstract, interface, or trait method. Public/protected instance methods SHALL dispatch virtually by default; private/static methods SHALL not. `super(...)` and `super.method()` SHALL address the concrete base, and `TraitName.super.method()` SHALL resolve a trait conflict.

#### Scenario: Missing override marker
- **WHEN** a subclass redeclares a compatible base method without `override fn`
- **THEN** compilation fails with both declarations identified

### Requirement: Object casts and identity
`is` SHALL compare reference identity, `==` SHALL require equality capability, `as` SHALL perform a checked related-type cast with controlled failure, and `as?` SHALL return nullable absence on failure. Unrelated casts SHALL fail at compile time.

#### Scenario: Optional downcast
- **WHEN** a User reference is evaluated with `as? Admin`
- **THEN** the result is the same Admin reference when compatible or `null` otherwise

### Requirement: Class declaration

The compiler SHALL support `class` with fields and methods, per `ZIRK_LANGUAGE_SPEC.md` section 7.

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

A class SHALL extend at most one class. Classes SHALL be inheritable by default, and `final` SHALL NOT exist.

#### Scenario: Subclass inherits fields and methods
- **WHEN** `class Admin extends User` does not declare `id`
- **THEN** an instance of `Admin` has the `id` field from `User`

#### Scenario: Multiple class inheritance
- **WHEN** a class attempts to extend two classes
- **THEN** a diagnostic is emitted indicating that class inheritance is single

#### Scenario: Inheritance cycle
- **WHEN** two classes extend each other
- **THEN** a diagnostic is emitted showing the cycle

### Requirement: Method overriding

A subclass SHALL be able to override a method of its superclass with the same signature, and the call SHALL resolve to the runtime type's definition.

#### Scenario: Dispatch to the override
- **WHEN** a variable declared with the base type holds an instance of the subclass and the overridden method is called
- **THEN** the subclass's method is executed

#### Scenario: Override with a different signature
- **WHEN** a subclass overrides a method while changing parameters or return type
- **THEN** a diagnostic is emitted showing both signatures

### Requirement: Abstract classes and methods

`abstract` SHALL be supported on classes and methods. An abstract class SHALL NOT be instantiated, and a concrete class SHALL implement every abstract method it inherits.

#### Scenario: Instantiating an abstract class
- **WHEN** an instance of an `abstract` class is constructed
- **THEN** a diagnostic is emitted indicating that it is not instantiable

#### Scenario: Unimplemented abstract method
- **WHEN** a concrete class inherits an `abstract` method and does not implement it
- **THEN** a diagnostic is emitted naming the method

#### Scenario: Abstract method with a body
- **WHEN** an `abstract` method declares a body
- **THEN** a diagnostic is emitted pointing to the body
