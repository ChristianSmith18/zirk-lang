## ADDED Requirements

### Requirement: Named-call binding is uniform across callable forms

The checker SHALL bind every named call argument to the parameter with the
matching label, regardless of whether the callee is a free function, method,
constructor, `super` method, or base constructor. It SHALL type-check each
argument against that bound parameter, reject a positional argument after the
first named argument, and preserve source evaluation order independently from
parameter binding order.

#### Scenario: Reordered method call

- **WHEN** a method declares `(prefix: String, suffix: String)` and is called
  with `suffix: "SUF", prefix: "PRE"`
- **THEN** the call is accepted and `prefix` binds to `"PRE"` while `suffix`
  binds to `"SUF"`

#### Scenario: Positional argument after a named argument

- **WHEN** a callable is invoked with `name: value, positional`
- **THEN** the checker emits a diagnostic requiring remaining arguments to be named

## MODIFIED Requirements

### Requirement: Constructor resolution

The checker SHALL reject constructor declarations whose parameter types and
accepted arities admit the same positional call, regardless of parameter
labels. It SHALL select among the remaining `construct` declarations by arity,
types, optional parameters, and names, SHALL allow reordering named arguments,
and SHALL reject genuinely ambiguous calls. It SHALL
retain the uniquely selected constructor for subsequent lowering rather than
resolving the overload again from only the source arity.

#### Scenario: Reordered named construction

- **WHEN** `User(name: "Cristian", id: 1)` matches
  `construct(id: UInt64, name: String)`
- **THEN** that signature is selected and each value binds by name

#### Scenario: Label and type select an overload

- **WHEN** `Point(s: "origin")` is called on constructors
  `construct(x: Int32)` and `construct(s: String)`
- **THEN** the `construct(s: String)` overload is selected

#### Scenario: Duplicate positional constructor signature

- **WHEN** a class declares `construct(x: Int32)` and `construct(y: Int32)`
- **THEN** the class declaration is rejected because parameter labels cannot
  distinguish a positional call such as `Point(1)`

#### Scenario: Ambiguous constructors

- **WHEN** more than one constructor accepts a call's arity, labels, and
  argument types
- **THEN** the checker rejects the construction as ambiguous
