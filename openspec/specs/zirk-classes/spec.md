# zirk-classes Specification

## Purpose
TBD - created by archiving change document-refined-core-language-semantics. Update Purpose after archive.
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
A concrete class MAY extend at most one concrete class. An `abstract class` SHALL be a nominal set of required attributes and `abstract fn` signatures with no constructor, body, state allocation, or layout contribution, and concrete classes SHALL adopt it through `implements`. Interfaces and traits SHALL also use `implements`.

#### Scenario: Abstract class contract
- **WHEN** `class Circle implements Shape` supplies every attribute and `override fn` required by abstract class `Shape`
- **THEN** Circle is accepted where Shape is required without inheriting Shape storage

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

