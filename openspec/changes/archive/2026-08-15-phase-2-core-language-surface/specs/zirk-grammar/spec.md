## MODIFIED Requirements

### Requirement: Conditional statement

The parser SHALL recognize `if` and `else` as a statement, with bodies always in braces. When both branches are present, `if`/`else` SHALL also be admitted as an expression, per `ZIRK_LANGUAGE_SPEC.md` section 5.

Whether it is a statement or an expression is not decided by the parser: it is decided by type checking, based on whether the usage requires a value and both branches are type-compatible.

#### Scenario: Simple conditional
- **WHEN** `if x > 0 { }` is parsed
- **THEN** a conditional statement with no alternative branch is produced

#### Scenario: Conditional with an alternative
- **WHEN** `if x > 0 { } else { }` is parsed
- **THEN** a conditional statement with both branches is produced

#### Scenario: Chaining
- **WHEN** `if a { } else if b { } else { }` is parsed
- **THEN** a conditional whose alternative branch is another conditional is produced

#### Scenario: Body without braces
- **WHEN** `if x > 0 return;` is parsed
- **THEN** a diagnostic stating that the body must be in braces is emitted

#### Scenario: Use as an expression
- **WHEN** `mut result = if x > 0 { "positive" } else { "not positive" };` is parsed
- **THEN** a declaration whose initializer is the conditional is produced

### Requirement: Constructs outside the subset

The parser SHALL emit a specific diagnostic for constructs that exist in the language but are not yet implemented, distinguishing them from syntax errors.

#### Scenario: Construct from a later phase
- **WHEN** `class`, `try`, `task`, `parallel`, or `thread` is parsed
- **THEN** the diagnostic SHALL name the construct
- **AND** it SHALL state that it is not implemented yet
- **AND** it SHALL NOT be reported as an unexpected token

#### Scenario: `init.zrk` out of scope
- **WHEN** an `init.zrk` file is found
- **THEN** the diagnostic states that declarative project configuration arrives in a later phase

## ADDED Requirements

### Requirement: Loops

The parser SHALL recognize `for` with initialization/condition/increment, `for ... in` over an iterable expression, `while`, and `loop`, together with `break` and `continue`, per `ZIRK_LANGUAGE_SPEC.md` section 5.

#### Scenario: `for` with all three clauses
- **WHEN** `for (mut i = 0; i < 10; i++) { }` is parsed
- **THEN** a loop with initialization, condition, and increment is produced

#### Scenario: `for ... in`
- **WHEN** `for x in 0..10 { }` is parsed
- **THEN** a loop that iterates the variable `x` over the range is produced

#### Scenario: `while`
- **WHEN** `while x > 0 { }` is parsed
- **THEN** a conditional loop is produced

#### Scenario: `loop`
- **WHEN** `loop { break; }` is parsed
- **THEN** an unconditional loop whose body contains `break` is produced

#### Scenario: `break` and `continue` outside a loop
- **WHEN** `break;` or `continue;` is parsed outside any loop
- **THEN** a diagnostic stating that they are only valid inside a loop is emitted

### Requirement: Optional, named, variadic parameters and default values

The parser SHALL recognize parameters marked with `?` for optional ones, default values after `=`, and a variadic parameter prefixed with `...`, per `ZIRK_LANGUAGE_SPEC.md` section 6.

#### Scenario: Optional parameter
- **WHEN** `fn greet(name?: String): Void { }` is parsed
- **THEN** a parameter marked as optional is produced

#### Scenario: Default value
- **WHEN** `fn greet(name: String = "world"): Void { }` is parsed
- **THEN** a parameter with a default-value expression is produced

#### Scenario: Variadic parameter
- **WHEN** `fn sum(...values: Int32): Int32 { }` is parsed
- **THEN** a variadic parameter is produced
- **AND** a variadic that is not the last parameter produces a diagnostic

#### Scenario: Named arguments in a call
- **WHEN** `greet(name: "Ann")` is parsed
- **THEN** a call with a named argument is produced

### Requirement: Closures and lambdas

The parser SHALL recognize lambda expressions of the form `(parameters): ReturnType => expression` or `(parameters): ReturnType => { ... }`, per `ZIRK_LANGUAGE_SPEC.md` section 6.

#### Scenario: Expression lambda
- **WHEN** `(a: Int32, b: Int32): Int32 => a + b` is parsed
- **THEN** a lambda whose body is the sum expression is produced

#### Scenario: Block lambda
- **WHEN** `(): Void => { stdout.println("ok"); }` is parsed
- **THEN** a lambda whose body is a block of statements is produced

### Requirement: `match`

The parser SHALL recognize `match` both in expression and statement position, with arms of the form `pattern => body`, per `ZIRK_LANGUAGE_SPEC.md` section 5.

The patterns admitted this phase are: literals, constructors of an `enum` without associated data, binding variables, and the `_` wildcard.

#### Scenario: `match` as a statement
- **WHEN** `match direction { Direction.North => stdout.println("North"); _ => {} }` is parsed
- **THEN** a `match` statement with its arms is produced

#### Scenario: `match` as an expression
- **WHEN** `mut text = match direction { Direction.North => "North"; _ => "other" };` is parsed
- **THEN** a declaration whose initializer is the `match` is produced

#### Scenario: Minimal `enum` declaration
- **WHEN** `enum Direction { North, South, East, West }` is parsed
- **THEN** an enum declaration with four constructors without associated data is produced

#### Scenario: `match with` out of scope
- **WHEN** `match with` is parsed
- **THEN** the diagnostic states that `Resource<E>` and `match with` arrive in a later phase

### Requirement: Nullability

The parser SHALL recognize `T?` as a type annotation, `null` as a literal, and `??` as the null-coalescing operator, per `ZIRK_LANGUAGE_SPEC.md` section 4.

`?.` is recognized but rejected with the phase diagnostic (decision D8): it requires members to access, and no type in this phase has any.

#### Scenario: Nullable type
- **WHEN** `mut name: String? = null;` is parsed
- **THEN** a declaration with type `String?` and a null initializer is produced

#### Scenario: Null coalescing
- **WHEN** `name ?? "anonymous"` is parsed
- **THEN** an expression with a fallback is produced

#### Scenario: Precedence of `??`
- **WHEN** `a ?? b || c` is parsed
- **THEN** the tree represents `(a ?? b) || c`, because `??` binds tighter than the logical operators

#### Scenario: `?.` deferred to Phase 3
- **WHEN** `user?.name` is parsed
- **THEN** the not-implemented-construct diagnostic is emitted, pointing at Phase 3

### Requirement: Compound assignment and increment

The parser SHALL recognize `+=`, `-=`, `*=`, `/=`, `%=`, `++`, and `--` in **statement** position, expanding them to the tree of the equivalent assignment, per the design's decision D9.

#### Scenario: Compound assignment
- **WHEN** `total += 5;` is parsed
- **THEN** the produced tree is equivalent to that of `total = total + 5;`

#### Scenario: Increment
- **WHEN** `i++;` or `++i;` is parsed
- **THEN** the produced tree is equivalent to that of `i = i + 1;`

#### Scenario: Increment in expression position
- **WHEN** `mut x = i++;` is parsed
- **THEN** a diagnostic stating that increment is only admitted as a statement in this phase is emitted

### Requirement: Modules within a crate

The parser SHALL recognize `share` as a declaration modifier, `import { names } from "path"` with quoted local paths and unquoted standard modules, and `use` to enable globals, per `ZIRK_LANGUAGE_SPEC.md` section 10.

#### Scenario: Shared declaration
- **WHEN** `share class User {}` is parsed
- **THEN** the diagnostic corresponding to `class` is emitted, which remains out of scope this phase
- **AND** `share fn greet(): Void {}` is indeed accepted, marking the function as shared

#### Scenario: Local import
- **WHEN** `import { User, Role } from "./domain/user";` is parsed
- **THEN** an import with a local path and two names is produced

#### Scenario: Import with an alias
- **WHEN** `import { Role -> DomainRole } from "./domain/user";` is parsed
- **THEN** the imported name is exposed under the alias

#### Scenario: `init.zrk` is not imported from code
- **WHEN** an `import` of project configuration is found
- **THEN** the `init.zrk`-out-of-scope diagnostic is emitted
