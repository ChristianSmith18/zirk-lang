## ADDED Requirements

### Requirement: Class declaration

The compiler SHALL admit `class` with fields and methods, per `ZIRK_LANGUAGE_SPEC.md` section 7.

#### Scenario: Class with fields and constructor
- **WHEN** `class User { public inmut id: Int32; construct(id: Int32) { this.id = id; } }` is declared
- **THEN** a `User` type is produced with one field and one constructor

#### Scenario: Field with no modifiers
- **WHEN** a class declares `name: String;`
- **THEN** the field is equivalent to `public mut name: String;`

#### Scenario: Multiple constructors
- **WHEN** a class declares two `construct` with distinct effective signatures
- **THEN** both are valid constructors and are available for resolution

#### Scenario: Instantiation without `new`
- **WHEN** `mut u = User(1);` is written
- **THEN** an instance is constructed
- **AND** writing `new User(1)` emits a diagnostic stating that `new` does not exist

#### Scenario: Field with no type
- **WHEN** a field is declared without a type annotation
- **THEN** a diagnostic pointing at the field is emitted

### Requirement: `this` designates the current instance

Inside a method or constructor, `this` SHALL refer to the instance it is invoked on.

#### Scenario: Field access via `this`
- **WHEN** a constructor writes `this.id = id;`
- **THEN** it assigns to the field, not to the parameter

#### Scenario: `this` outside a class
- **WHEN** `this` appears in a top-level function
- **THEN** a diagnostic stating that there is no instance is emitted

### Requirement: Member visibility

A member SHALL admit `public`, `private`, or `protected`, with `public` as default, per `ZIRK_LANGUAGE_SPEC.md` section 7.

#### Scenario: Private member from outside
- **WHEN** a `private` field is accessed from outside its class
- **THEN** a diagnostic naming the member and its visibility is emitted

#### Scenario: Protected member from a subclass
- **WHEN** a subclass accesses a `protected` member of its superclass
- **THEN** the access is valid

#### Scenario: Default visibility
- **WHEN** a member is declared with no modifier
- **THEN** it is `public`

### Requirement: Single inheritance

A class SHALL extend at most one class. Classes SHALL be inheritable by default, and `final` SHALL NOT exist.

#### Scenario: Subclass inherits fields and methods
- **WHEN** `class Admin extends User` does not declare `id`
- **THEN** an instance of `Admin` has `User`'s `id` field

#### Scenario: Multiple class inheritance
- **WHEN** a class attempts to extend two classes
- **THEN** a diagnostic stating that class inheritance is single is emitted

#### Scenario: Inheritance cycle
- **WHEN** two classes extend each other
- **THEN** a diagnostic showing the cycle is emitted

### Requirement: Method overriding

A subclass SHALL be able to override a method of its superclass with the same signature, and the call SHALL resolve to the type's definition at runtime.

#### Scenario: Dispatch to the override
- **WHEN** a variable declared with the base type holds an instance of the subclass and the overridden method is called
- **THEN** the subclass's method runs

#### Scenario: Override with a different signature
- **WHEN** a subclass overrides a method changing parameters or return type
- **THEN** a diagnostic showing both signatures is emitted

### Requirement: Abstract classes and methods

`abstract` SHALL be admitted on classes and methods. An abstract class SHALL NOT be instantiated, and a concrete class SHALL implement every abstract method it inherits.

#### Scenario: Instantiating an abstract class
- **WHEN** an instance of an `abstract` class is constructed
- **THEN** a diagnostic stating that it is not instantiable is emitted

#### Scenario: Unimplemented abstract method
- **WHEN** a concrete class inherits an `abstract` method and does not implement it
- **THEN** a diagnostic naming the method is emitted

#### Scenario: Abstract method with a body
- **WHEN** an `abstract` method declares a body
- **THEN** a diagnostic pointing at the body is emitted
