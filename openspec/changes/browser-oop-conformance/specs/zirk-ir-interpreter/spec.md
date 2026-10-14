## ADDED Requirements

### Requirement: Object-oriented programs execute with native-equivalent behavior

The interpreter SHALL execute the delivered object-oriented surface — classes with constructors, fields, `this`, visibility, single inheritance with `super` and `#override`, abstract classes, interfaces, traits with default implementations, static members, static nested, `inner` and local classes, generic classes and methods, and objects held in collections — with the same normalized output, completion category and runtime failure category as the native pipeline. A construct SHALL be documented as browser-supported only when a shared conformance fixture exercises it in both engines.

#### Scenario: Override selected by runtime type
- **WHEN** an interpreted program calls an overridden method through a variable declared with the superclass type
- **THEN** the override of the runtime class executes
- **AND** the output matches native execution

#### Scenario: Interface and trait dispatch
- **WHEN** an interpreted program calls an interface method and a trait default method on instances of different classes
- **THEN** each call executes the implementation selected by the class's contract table
- **AND** a default implementation runs only for classes that do not override it

#### Scenario: Abstract class instantiated through a subclass
- **WHEN** an interpreted program constructs a concrete subclass of an abstract class and calls an abstract method through the abstract type
- **THEN** the subclass implementation executes and the output matches native execution

#### Scenario: Polymorphic collection
- **WHEN** an interpreted program stores instances of several subclasses in a `List<Supertype>` and calls the same method on each element
- **THEN** each element dispatches to its own implementation in iteration order

#### Scenario: Identity versus structural equality
- **WHEN** an interpreted program compares two distinct class instances with equal field values using `is` and `==`
- **THEN** `is` is false and the equality result matches native execution

#### Scenario: Static and nested classes
- **WHEN** an interpreted program reads and writes a static member and instantiates a static nested class, an `inner` class that captures its enclosing instance, and a local class
- **THEN** each observes the same state and produces the same output as native execution

#### Scenario: Exception thrown from a method
- **WHEN** a method of an interpreted object throws a typed exception that a caller catches by supertype
- **THEN** the handler runs with the original exception identity and the output matches native execution

### Requirement: Unsupported object-model constructs fail as capability diagnostics

When a verified module uses an object-model construct that the browser interpreter does not execute, preflight SHALL report a structured capability diagnostic that names the construct, and the entrypoint SHALL NOT execute. The interpreter SHALL NOT surface such a case as an internal failure.

#### Scenario: Construct lowers to an excluded instruction
- **WHEN** a program uses an object-model feature whose lowering requires a browser-excluded instruction
- **THEN** the run result is a capability failure that identifies the feature and its source location
- **AND** no output from the program is produced

### Requirement: Browser object-model coverage is fixture-backed and discoverable

The repository SHALL keep a checked-in catalog that lists each documented object-model construct with its browser verdict and the conformance fixture that evidences it, and the support matrix and Feature Status SHALL derive their object-model claims from that catalog.

#### Scenario: Construct without a passing fixture
- **WHEN** a documented object-model construct has no passing shared fixture
- **THEN** the catalog marks it unclaimed
- **AND** public documentation does not describe it as browser-supported

#### Scenario: Catalog drift
- **WHEN** a fixture named by the catalog is removed or fails in either engine
- **THEN** repository verification fails and names the construct and fixture
