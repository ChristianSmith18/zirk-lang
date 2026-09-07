# Delta spec: zirk-classes

## MODIFIED Requirements

### Requirement: Attributes, defaults, and projection behavior
Classes SHALL expose attributes and ordinary methods, SHALL NOT expose a property declaration, and SHALL use conventional `get_`/`set_` methods when accessors are desired. An attribute MAY declare an explicit default initializer `field: Type = expr;`; the expression SHALL NOT reference `this` and SHALL be evaluated in declaration order during construction, before the constructor body, only for fields the constructor does not assign. Attributes without an initializer SHALL receive their type default. Reading a reference-valued attribute SHALL deep-clone an independent value, while a whole class variable SHALL share identity and an attribute place SHALL mutate original storage.

#### Scenario: Attribute getter is ordinary method
- **WHEN** a class defines `fn get_name(): String`
- **THEN** it is callable as an ordinary method, with no property syntax involved

#### Scenario: Field with explicit default
- **WHEN** `count: Int32 = 0;` is declared and a constructor does not assign `count`
- **THEN** the constructed instance has `count == 0`

#### Scenario: Constructor assignment wins over the default
- **WHEN** `count: Int32 = 0;` is declared and the constructor assigns `this.count = 5`
- **THEN** the constructed instance has `count == 5`

#### Scenario: Default cannot see the instance
- **WHEN** a field initializer references `this`
- **THEN** a diagnostic is emitted

## ADDED Requirements

### Requirement: Static members
A class MAY declare `static fn` methods and `static` fields. Static members SHALL be accessed as `ClassName.member`, SHALL NOT dispatch virtually, SHALL NOT appear in vtables or contract tables, and `this` SHALL be an error inside a static body. The usual visibility rules (`public`, `private`, `protected`) and mutability rules (`mut`/`inmut`) apply unchanged. Static members SHALL be allowed on classes; whether `record` or `enum` may declare them is left to a later decision.

#### Scenario: Static method call
- **WHEN** `class Counter { static fn make(): Counter { ... } }` is declared and `Counter.make()` is written
- **THEN** the call resolves statically to `Counter`'s method, with no receiver instance required

#### Scenario: Static field access
- **WHEN** `class Limits { static inmut max: Int32 = 100; }` is declared and `Limits.max` is read
- **THEN** the read resolves to the class-level field

#### Scenario: No receiver in static context
- **WHEN** a `static fn` body references `this`
- **THEN** a diagnostic is emitted

#### Scenario: Static members are not virtual
- **WHEN** a subclass redeclares a `static fn` of its base
- **THEN** no override relationship is created and calls resolve by the static type name used
