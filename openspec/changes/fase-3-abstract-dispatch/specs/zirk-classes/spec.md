## MODIFIED Requirements

### Requirement: Concrete inheritance and abstract implementation
A concrete class MAY extend at most one concrete class. An `abstract class` SHALL be a nominal set of required attributes and `abstract fn` signatures with no constructor, body, state allocation, or layout contribution, and concrete classes SHALL adopt it through `implements`. Interfaces and traits SHALL also use `implements`. A value statically typed as an `abstract class`, holding a concrete adopter instance, SHALL dispatch every call to the adopter's own override.

#### Scenario: Abstract class contract
- **WHEN** `class Circle implements Shape` supplies every attribute and `override fn` required by abstract class `Shape`
- **THEN** Circle is accepted where Shape is required without inheriting Shape storage

#### Scenario: Dynamic dispatch through an abstract-class-typed value
- **WHEN** a variable statically typed as abstract class `Shape` holds a `Circle` instance and a caller invokes a `Shape`-declared method on it
- **THEN** the call dispatches to `Circle`'s own override, the same way dispatch through a concrete base class already works
