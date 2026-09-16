## ADDED Requirements

### Requirement: Constructor parameter promotion syntax

The parser SHALL accept member modifiers — `public`, `private`,
`protected`, `mut`, `inmut`, and `inmut::strict`, in any valid member
combination — on parameters of a `construct` declaration inside a `class`
body, and SHALL mark each modified parameter as promoted. A `construct`
parameter without modifiers SHALL remain an ordinary parameter. The parser
SHALL reject member modifiers on parameters of methods, free functions,
lambdas, and `extern` declarations with a targeted diagnostic, and SHALL
reject a promoted variadic parameter.

#### Scenario: Promoted parameters parse

- **WHEN** source contains `construct(public name: String, private inmut id: UInt64)`
- **THEN** both parameters are recorded with their modifiers and marked as
  promoted

#### Scenario: Ordinary parameter stays ordinary

- **WHEN** a `construct` parameter carries no modifier
- **THEN** it binds only inside the constructor body and declares no field

#### Scenario: Optional and default promoted parameters

- **WHEN** source contains `construct(public name?: String)` or
  `construct(public name: String = "unknown")`
- **THEN** the promoted parameter keeps its optional or default contract

#### Scenario: Modifier on a method parameter

- **WHEN** a method parameter carries `public`, `private`, `protected`,
  `mut`, `inmut`, or `inmut::strict`
- **THEN** a targeted diagnostic explains that member modifiers apply only
  to `construct` parameters

#### Scenario: Promoted variadic parameter

- **WHEN** a `construct` parameter is both variadic and promoted
- **THEN** a diagnostic rejects the promotion

### Requirement: Promotion is restricted to class constructors

Promoted parameters SHALL be accepted only on `construct` members of a
`class` declaration. Writing a promoted parameter on a `record`, `enum`,
`interface`, `trait`, or `abstract class` member — or on any other
declaration — SHALL produce a targeted diagnostic naming where promotion
is allowed.

#### Scenario: Promotion on a record member

- **WHEN** source attempts promotion inside a `record` declaration
- **THEN** a diagnostic indicates promotion applies only to `class`
  constructors
