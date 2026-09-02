# zirk-grammar

## Purpose

Defines the grammar of the language and the construction of the syntax tree, together with the errors it reports.

The parser decides whether a program is well *formed*, not whether it makes *sense*: that belongs to the type system.
## Requirements
### Requirement: Function declaration

The parser SHALL recognize function declarations with the form from `ZIRK_LANGUAGE_SPEC.md` section 6: `fn`, name, list of typed parameters, return type after `:`, and a body between braces.

#### Scenario: Function without parameters
- **WHEN** `fn main(): Void { }` is parsed
- **THEN** a function declaration named `main`, with no parameters and return type `Void`, is produced

#### Scenario: Function with parameters
- **WHEN** `fn add(a: Int32, b: Int32): Int32 { return a + b; }` is parsed
- **THEN** a function with two typed parameters and return type `Int32` is produced

#### Scenario: Missing return type
- **WHEN** a function is declared without a return type
- **THEN** a diagnostic pointing to where the type was expected is emitted
- **AND** the help indicates that the return type is mandatory at this phase

### Requirement: Variable declaration

The parser SHALL recognize declarations with `mut` and `inmut`, with an optional type annotation when there is an initializer.

#### Scenario: Variable with an explicit type
- **WHEN** `mut count: Int32 = 0;` is parsed
- **THEN** a mutable declaration with type `Int32` and an initializer is produced

#### Scenario: Variable with an inferred type
- **WHEN** `mut count = 0;` is parsed
- **THEN** a mutable declaration without a type annotation is produced

#### Scenario: Declaration without an initializer or a type
- **WHEN** `mut count;` is parsed
- **THEN** a diagnostic indicating that the type or the initializer is missing is emitted

### Requirement: Expressions and precedence

The parser SHALL build expressions respecting the conventional precedence and associativity of the operators from `ZIRK_LANGUAGE_SPEC.md` section 4.

From highest to lowest precedence: exponentiation (`**`, right-associative); unary (`!`, `-`); multiplicative (`*`, `/`, `%`); additive (`+`, `-`); comparison (`<`, `<=`, `>`, `>=`); equality (`==`, `!=`); conjunction (`&&`); disjunction (`||`).

#### Scenario: Multiplicative precedence over additive
- **WHEN** `1 + 2 * 3` is parsed
- **THEN** the tree represents `1 + (2 * 3)`

#### Scenario: Left associativity
- **WHEN** `10 - 4 - 3` is parsed
- **THEN** the tree represents `(10 - 4) - 3`

#### Scenario: Parentheses alter precedence
- **WHEN** `(1 + 2) * 3` is parsed
- **THEN** the tree represents the sum as the left operand of the product

#### Scenario: Conjunction over disjunction
- **WHEN** `a || b && c` is parsed
- **THEN** the tree represents `a || (b && c)`

### Requirement: Conditional statement

The parser SHALL recognize `if` and `else` as a statement, with bodies between braces except in the effect form that governs a single statement. When both branches are present, `if`/`else` SHALL also be accepted as an expression, per `ZIRK_LANGUAGE_SPEC.md` section 5.

Whether it is a statement or an expression is not decided by the parser: it is decided by type checking, based on whether the use requires a value and both branches are type-compatible.

Parentheses around the condition SHALL be optional.

#### Scenario: Simple conditional
- **WHEN** `if x > 0 { }` is parsed
- **THEN** a conditional statement without an alternative branch is produced

#### Scenario: Conditional with an alternative
- **WHEN** `if x > 0 { } else { }` is parsed
- **THEN** a conditional statement with both branches is produced

#### Scenario: Chaining
- **WHEN** `if a { } else if b { } else { }` is parsed
- **THEN** a conditional whose alternative branch is another conditional is produced

#### Scenario: Body without braces
- **WHEN** `if closed return;` is parsed
- **THEN** a conditional whose single branch is that statement is produced

#### Scenario: Alternative branch on a body without braces
- **WHEN** an `if` without braces is followed by `else`
- **THEN** a diagnostic indicating that the braceless form governs a single statement is emitted

#### Scenario: Condition in parentheses
- **WHEN** `if (x > 0) { }` is parsed
- **THEN** the same tree as without parentheses is produced

#### Scenario: Use as an expression
- **WHEN** `mut result = if x > 0 { "positive" } else { "not positive" };` is parsed
- **THEN** a declaration whose initializer is the conditional is produced

### Requirement: Function calls and return

The parser SHALL recognize calls with positional arguments and the `return` statement.

#### Scenario: Call with arguments
- **WHEN** `add(1, 2)` is parsed
- **THEN** a call with two arguments is produced

#### Scenario: Return with a value
- **WHEN** `return a + b;` is parsed
- **THEN** a return whose expression is the sum is produced

#### Scenario: Return without a value
- **WHEN** `return;` is parsed
- **THEN** a return without an expression is produced

### Requirement: Optional semicolon

The parser SHALL allow omitting the semicolon when there is no ambiguity, per `ZIRK_LANGUAGE_SPEC.md` section 1.

#### Scenario: Statements without a semicolon
- **WHEN** statements separated by newlines and without `;` are parsed
- **THEN** the resulting tree is equivalent to that of the same statements with `;`

### Requirement: Constructs outside the subset

The parser SHALL emit a specific diagnostic for constructs that exist in the language but are not yet implemented, distinguishing them from syntax errors.

This phase removes from that list `class`, `construct`, `this`, `record`, `type`, `public`, `private`, `protected`, `abstract`, `implements`, `extends`, `from`, `as`, and `is`.

#### Scenario: Construct from a later phase
- **WHEN** `try`, `task`, `parallel`, or `thread` is parsed
- **THEN** the diagnostic SHALL name the construct
- **AND** SHALL indicate that it is not implemented yet
- **AND** SHALL NOT be reported as an unexpected token

#### Scenario: `init.zrk` out of scope
- **WHEN** an `init.zrk` file is found
- **THEN** the diagnostic indicates that declarative project configuration arrives in a later phase

### Requirement: Location on every node

Every node of the tree SHALL expose the source span that originated it.

A node without a location cannot produce the diagnostic that `ZIRK_COMPILER_SPEC.md` section 8 requires.

#### Scenario: Span of any node
- **WHEN** any node of the produced tree is inspected
- **THEN** it exposes its start and end location in the source

### Requirement: Loops

The parser SHALL recognize `for` with initialization/condition/increment, `for ... in` over an iterable expression, `while`, `do ... while`, and `loop`, together with `break` and `continue`, per `ZIRK_LANGUAGE_SPEC.md` section 5.

Parentheses around the header SHALL be optional in all these forms. The canonical form omits them, and both SHALL produce the same tree.

#### Scenario: `for` with all three clauses
- **WHEN** `for mut i = 0; i < 10; i++ { }` is parsed
- **THEN** a loop with initialization, condition, and increment is produced

#### Scenario: `for` with parentheses
- **WHEN** `for (mut i = 0; i < 10; i++) { }` is parsed
- **THEN** the same tree as without parentheses is produced

#### Scenario: `for ... in`
- **WHEN** `for x in 0..10 { }` is parsed
- **THEN** a loop that iterates the variable `x` over the range is produced

#### Scenario: `while`
- **WHEN** `while x > 0 { }` or `while (x > 0) { }` is parsed
- **THEN** a conditional loop is produced in both cases

#### Scenario: `do ... while`
- **WHEN** `do { poll(); } while pending;` is parsed
- **THEN** a post-condition loop whose body precedes its condition is produced

#### Scenario: `loop`
- **WHEN** `loop { break; }` is parsed
- **THEN** an unconditional loop whose body contains `break` is produced

#### Scenario: `break` and `continue` outside a loop
- **WHEN** `break;` or `continue;` is parsed outside any loop
- **THEN** a diagnostic indicating that they are only valid inside a loop is emitted

### Requirement: Optional, named, variadic parameters and default values

The parser SHALL recognize parameters with a `?` sign for optional ones, default values after `=`, and a variadic parameter prefixed with `...`, per `ZIRK_LANGUAGE_SPEC.md` section 6.

#### Scenario: Optional parameter
- **WHEN** `fn greet(name?: String): Void { }` is parsed
- **THEN** a parameter marked as optional is produced

#### Scenario: Default value
- **WHEN** `fn greet(name: String = "world"): Void { }` is parsed
- **THEN** a parameter with a default-value expression is produced

#### Scenario: Variadic parameter
- **WHEN** `fn sum(...values: Int32): Int32 { }` is parsed
- **THEN** a variadic parameter is produced
- **AND** a variadic parameter that is not the last one produces a diagnostic

#### Scenario: Named arguments in the call
- **WHEN** `greet(name: "Ana")` is parsed
- **THEN** a call with one named argument is produced

### Requirement: Closures and lambdas

The parser SHALL recognize lambda expressions with the form `(parameters): ReturnType => expression` or `(parameters): ReturnType => { ... }`, per `ZIRK_LANGUAGE_SPEC.md` section 6.

#### Scenario: Expression lambda
- **WHEN** `(a: Int32, b: Int32): Int32 => a + b` is parsed
- **THEN** a lambda whose body is the sum expression is produced

#### Scenario: Block lambda
- **WHEN** `(): Void => { stdout.println("ok"); }` is parsed
- **THEN** a lambda whose body is a block of statements is produced

### Requirement: `match`

The parser SHALL recognize `match` both in expression and statement position, with arms of the form `pattern => body`, per `ZIRK_LANGUAGE_SPEC.md` section 5.

The patterns accepted at this phase are: literals, `enum` constructors without associated data, binding variables, and the `_` wildcard.

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
- **THEN** the diagnostic indicates that `Resource<E>` and `match with` arrive in a later phase

### Requirement: Nullability

The parser SHALL recognize `T?` as a type annotation, `null` as a literal, `??` as null coalescing, and `?.` as safe access, per `ZIRK_LANGUAGE_SPEC.md` section 4.

`?.` is no longer deferred: the previous phase postponed it because no type had members, and classes bring them.

#### Scenario: Nullable type
- **WHEN** `mut name: String? = null;` is parsed
- **THEN** a declaration with type `String?` and a null initializer is produced

#### Scenario: Null coalescing
- **WHEN** `name ?? "anonymous"` is parsed
- **THEN** an expression with a fallback is produced

#### Scenario: Precedence of `??`
- **WHEN** `a ?? b || c` is parsed
- **THEN** the tree represents `(a ?? b) || c`

#### Scenario: Safe access
- **WHEN** `user?.name` is parsed
- **THEN** a safe-access expression on the member is produced

### Requirement: Compound assignment and increment

The parser SHALL recognize `+=`, `-=`, `*=`, `/=`, `%=`, `**=`, `++`, and `--` in statement position, expanding them to the tree of the equivalent assignment.

`++` and `--` SHALL also be accepted in expression position, preserving the conventional prefix/postfix semantics from `ZIRK_LANGUAGE_SPEC.md` section 4: the postfix form evaluates to the previous value and the prefix form to the already-incremented value. Both SHALL require an assignable, mutable place.

#### Scenario: Compound assignment
- **WHEN** `total += 5;` is parsed
- **THEN** the produced tree is equivalent to that of `total = total + 5;`

#### Scenario: Increment
- **WHEN** `i++;` or `++i;` is parsed
- **THEN** the produced tree is equivalent to that of `i = i + 1;`

#### Scenario: Postfix increment in expression position
- **WHEN** `mut x = i++;` is parsed
- **THEN** `x` receives the value of `i` prior to the increment

#### Scenario: Prefix increment in expression position
- **WHEN** `mut x = ++i;` is parsed
- **THEN** `x` receives the already-incremented value of `i`

#### Scenario: Increment on a non-assignable place
- **WHEN** the operand of `++` is not an assignable, mutable place
- **THEN** a diagnostic pointing to the operand is emitted

### Requirement: Modules within a crate

The parser SHALL recognize `share` as a declaration modifier, `import { names } from "path"` with quoted local paths and unquoted standard modules, and `use` to enable globals, per `ZIRK_LANGUAGE_SPEC.md` section 10.

#### Scenario: Shared declaration
- **WHEN** `share class User {}` is parsed
- **THEN** the diagnostic corresponding to `class`, which remains out of scope at this phase, is emitted
- **AND** `share fn greet(): Void {}` is indeed accepted, marking the function as shared

#### Scenario: Local import
- **WHEN** `import { User, Role } from "./domain/user";` is parsed
- **THEN** an import with a local path and two names is produced

#### Scenario: Import with alias
- **WHEN** `import { Role -> DomainRole } from "./domain/user";` is parsed
- **THEN** the imported name is exposed under the alias

#### Scenario: `init.zrk` is not imported from code
- **WHEN** an `import` of project configuration is found
- **THEN** the `init.zrk` out-of-scope diagnostic is emitted

### Requirement: Complete range and slice forms
The parser SHALL accept `start..end`, `start..=end`, descending bounds, `.step(distance)`, `.reverse()`, interpolated bounds such as `0..{number}`, and slices `[start:end:step]` with omitted or negative components.

#### Scenario: Descending stepped range
- **WHEN** source contains `10..=0.step(2)`
- **THEN** it represents an inclusive descending range with distance two

#### Scenario: Reverse slice
- **WHEN** source contains `values[::-1]`
- **THEN** it represents a slice with omitted bounds and negative step

### Requirement: Match alternatives, regex, and nested destructuring
The parser SHALL group alternative patterns with commas followed by one `=>` body, SHALL accept regex literals as patterns, and SHALL compose enum payload and record destructuring as `UserCreated({ id, name })`.

#### Scenario: Multiline alternatives
- **WHEN** `200`, `201`, and `204` appear on separate lines before one `=>` body
- **THEN** all three patterns select that body

#### Scenario: Nested enum payload pattern
- **WHEN** source contains `UserCreated({ id, name }) => handle(id, name)`
- **THEN** the enum payload is destructured and both names bind in the branch

### Requirement: Function and constructor surface
The parser SHALL accept lambdas with or without leading `fn`, SHALL require a type on every optional parameter, SHALL accept multiple `construct` declarations, SHALL accept reordered named construction arguments, and SHALL parse `fn gen` with `yield`.

#### Scenario: Optional fn lambda
- **WHEN** source contains `fn(a: Int32): Int32 => a + 1`
- **THEN** it produces the same lambda form as `(a: Int32): Int32 => a + 1`

#### Scenario: Multiple constructors
- **WHEN** a class contains two `construct` declarations with different parameter lists
- **THEN** both constructor signatures are retained for semantic resolution

#### Scenario: Generator declaration
- **WHEN** source contains `fn gen numbers(): Int32 { yield 1; }`
- **THEN** it produces a generator function whose declared type is its yielded element type

### Requirement: Traditional enum mappings
The parser SHALL accept traditional enum cases without mappings and cases mapped with `->` to compatible string or numeric values.

#### Scenario: String-mapped case
- **WHEN** source contains `North -> "N";`
- **THEN** the enum case retains `"N"` as its explicit observable mapping

### Requirement: Contextual constructor expressions
The grammar SHALL retain the complete contained operator tree of `Float(expression)` and `String(expression)` so semantic analysis can apply explicit deep contextual conversion before evaluating compatible contained arithmetic or concatenation operators.

#### Scenario: Nested contextual arithmetic
- **WHEN** `Float((a + 1) / (b * 2))` is parsed
- **THEN** the constructor contains the entire nested arithmetic tree rather than an already-evaluated integer result

### Requirement: Native String repetition syntax
The grammar SHALL accept multiplication and compound multiplication between a String expression and an integer expression, leaving type checking to enforce operand types, non-negative counts, mutability for `*=`, and allocation bounds.

#### Scenario: Compound String repetition
- **WHEN** `laugh *= 3` is parsed
- **THEN** it is represented as a compound assignment whose semantic result is String repetition

### Requirement: Temporal construction and composition syntax
The grammar SHALL accept ordinary typed constructors, named components, method calls, ISO strings, and the documented temporal operator combinations without introducing a single ambiguous all-purpose Date literal.

#### Scenario: Named Duration construction
- **WHEN** `Duration(hours: 2, minutes: 30)` is parsed
- **THEN** the constructor retains both named temporal components

#### Scenario: Date and Time composition
- **WHEN** `date + time` is parsed
- **THEN** it remains a binary operation for type-directed `DateTime` composition

### Requirement: Optional parentheses in control headers

The parser SHALL allow optional parentheses around the header of every control structure — `if`, `while`, `for`, `for ... in`, `do ... while`, and `match` — producing the same tree with and without them.

The form without parentheses is canonical. No structure SHALL require them.

This rule was not written in any normative source, and that absence is what led to requiring them in the traditional `for`.

#### Scenario: Header without parentheses
- **WHEN** `while pending { }` is parsed
- **THEN** parsing succeeds

#### Scenario: Header with parentheses
- **WHEN** `while (pending) { }` is parsed
- **THEN** parsing succeeds and produces the same tree

#### Scenario: `for ... in` with parentheses
- **WHEN** `for (x in 0..10) { }` is parsed
- **THEN** the same tree as `for x in 0..10 { }` is produced

#### Scenario: `match` with parentheses
- **WHEN** `match (value) { }` is parsed
- **THEN** the same tree as `match value { }` is produced

### Requirement: Ternary expression

The parser SHALL recognize the ternary expression `condition ? when_true : when_false`, right-associative, per level 16 of the precedence table.

The ternary is the preferred compact form for choosing a short value, and SHALL NOT replace `if` in expression position: both coexist.

#### Scenario: Simple ternary
- **WHEN** `mut label = active ? "yes" : "no";` is parsed
- **THEN** the initializer is a ternary expression with its three parts

#### Scenario: Nested ternary
- **WHEN** `a ? b : c ? d : e` is parsed
- **THEN** the nesting groups to the right

#### Scenario: Ternary without an alternative branch
- **WHEN** `:` and its expression are missing
- **THEN** a diagnostic pointing to the incomplete ternary is emitted

### Requirement: Refined core syntax surface
The parser SHALL recognize `Fn(P...) => R` and `Function(P...) => R`, `override fn`, abstract classes adopted with `implements`, combined `from A & B` constraints, `in/out` generic variance, Tuple type/index syntax, collection literals, `as?`, and the accepted guard-free pattern forms. It SHALL reject property declarations, standalone override, enum user methods, match guards, and enum destructuring bindings.

#### Scenario: Callable type annotation
- **WHEN** source contains `mut print: Fn(String) => Void = stdout.println`
- **THEN** the parser produces a mutable binding whose annotation is a callable type

#### Scenario: Removed property syntax
- **WHEN** source declares `property name: String`
- **THEN** it receives a targeted diagnostic recommending an attribute plus `get_name`/`set_name` methods

### Requirement: Slice omission syntax
The parser SHALL preserve independently omitted start, end, and step components in `[:]`, `[::]`, `[n:]`, `[:w]`, `[n:w]`, `[::k]`, and reverse forms so semantic analysis can apply direction-sensitive defaults.

#### Scenario: Fully omitted slice
- **WHEN** `[::]` is parsed
- **THEN** start, end, and step are represented as omitted rather than fabricated source literals

### Requirement: Failure and resource syntax is unambiguous
The grammar SHALL accept `throws T | U` after a return type, `throw expression`, exact `throw;` inside a catch, `try` followed by pattern-shaped `catch Type(binding)` clauses and optional `finally`, and `match acquisition with binding` including grouped acquisitions. Historical `catch<Type> name` SHALL be rejected with migration guidance.

#### Scenario: Typed catch parses
- **WHEN** source contains `catch NetworkError.Timeout(duration) { retry(duration); }`
- **THEN** the parser produces a typed variant catch pattern without a guard

### Requirement: Permission manifests use requires and permissions
The manifest grammar SHALL accept library `requires`, application `permissions`, operation-specific scopes, and `during: build | runtime | both`. It SHALL reject a top-level `compile_permissions` block with guidance to move the phase into the relevant grant.

#### Scenario: Build-only filesystem grant
- **WHEN** `init.zrk` grants a filesystem read operation with `during: build`
- **THEN** the manifest AST preserves the operation, scope, and phase separately

### Requirement: Named argument shorthand is explicit
The call grammar SHALL accept `name: expression` as an explicit named argument
and `name:` as shorthand for `name: name`. The shorthand SHALL accept only a
simple identifier, SHALL remain reorderable with other named arguments, and
SHALL NOT reinterpret a reordered bare positional identifier by matching its
spelling to a parameter.

#### Scenario: Reordered shorthand arguments
- **WHEN** source calls `client.get(timeout:, url:)`
- **THEN** the AST records named arguments equivalent to
  `client.get(timeout: timeout, url: url)`

#### Scenario: Member expression uses explicit value
- **WHEN** source attempts `client.get(config.url:)`
- **THEN** compilation diagnoses invalid shorthand and recommends
  `client.get(url: config.url)`

#### Scenario: Positional argument follows a named argument
- **WHEN** source calls `client.get(timeout:, url)`
- **THEN** compilation rejects the positional argument after the named argument

### Requirement: Comma-grouped declarations and assignments are explicit
The grammar SHALL accept a comma-separated list of simple binding names before
one shared type annotation in a `mut`, `inmut`, or `inmut::strict` declaration.
It SHALL accept an optional comma-separated initializer list and simultaneous
assignment to a comma-separated list of assignable places. These forms SHALL
remain distinct from tuple construction and destructuring patterns.

#### Scenario: Shared-type declaration
- **WHEN** source declares `mut first, second: String;`
- **THEN** the AST records two mutable bindings with the shared `String` type

#### Scenario: Simultaneous swap
- **WHEN** source assigns `left, right = right, left;`
- **THEN** the AST records one simultaneous assignment with two destinations
  and two source expressions

#### Scenario: Assignment arity mismatch
- **WHEN** source assigns `left, right = right, left, extra;`
- **THEN** parsing preserves both arities so semantic analysis can emit a
  targeted count-mismatch diagnostic

### Requirement: Safety and concurrency grammar
The grammar SHALL parse unsafe function modifiers and blocks, `commit` regions, `task` blocks and callable sugar, `task scope`, `cancellation shield`, await timeouts, and `select` branches with `after`, `default`, and cancellation cases without introducing `async fn`.

#### Scenario: Select statement is parsed
- **WHEN** source contains task, channel, timer, and default select branches
- **THEN** the parser produces distinct guarded branches and their result bindings

### Requirement: Contextual safety restrictions
The parser and semantic frontend SHALL preserve enough contextual information to diagnose `await`, task/thread creation, or irreversible effects in a reversible unsafe transaction and unsafe-only operations outside an unsafe boundary.

#### Scenario: Commit appears outside unsafe
- **WHEN** source places `commit {}` outside an unsafe block
- **THEN** compilation fails with a contextual syntax or semantic diagnostic

### Requirement: Decorator declarations have explicit forms
The grammar SHALL recognize `fn dec Name(parameters) { target-blocks }` and `repeatable fn dec Name(parameters) { target-blocks }`. Each target block SHALL be named only `class`, `attribute`, `function`, `method`, or `parameter` and SHALL bind its typed target explicitly.

#### Scenario: Repeatable method decorator parses
- **WHEN** `repeatable fn dec Middleware(name: String) { method(target) { ... } }` is parsed
- **THEN** the syntax tree records a repeatable decorator, its typed argument schema, and one method target block

#### Scenario: Unsupported decorator target parses for diagnosis
- **WHEN** a decorator contains `construct(target) { ... }`
- **THEN** the parser preserves a recoverable target block and emits a targeted unsupported-decorator-target diagnostic rather than a generic unexpected-token error

### Requirement: Decorator applications are ordered syntax
The grammar SHALL recognize `@Name` and `@Name(arguments)` applications before supported declarations and parameters, preserving exact top-to-bottom source order and application spans.

#### Scenario: Multiple applications decorate a method
- **WHEN** `@Authorized() @Cached(5.minutes) fn report(): Report { ... }` is parsed across one or multiple lines
- **THEN** both applications are attached to the method in source order with independent spans

### Requirement: Decorator dependency clauses are parsed explicitly
The grammar SHALL recognize optional `requires decorators [...]`, `before decorators [...]`, and `after decorators [...]` clauses after a decorator header and before its body. Each list SHALL contain decorator names and SHALL preserve its source order and spans for semantic validation.

#### Scenario: Ordering constraint parses
- **WHEN** `fn dec Authorized() before decorators [Cached] { ... }` is parsed
- **THEN** the decorator node contains a `before` constraint referencing `Cached`

### Requirement: Decorator phase patterns expose their payloads
The grammar SHALL parse `Inspect`, `Augment`, and `Wrap` variants inside `match target.transform`, including explicit repeatable payloads such as `Inspect(context, applications)`. It SHALL parse `Before`, `After`, `Catch`, and `Around` variants inside `match target.wrap` and SHALL apply normal exact-arity pattern parsing, including `_` payload omissions.

#### Scenario: Repeatable applications are explicitly bound
- **WHEN** `Inspect(context, applications) => { ... }` is parsed
- **THEN** the arm introduces both payload bindings and no implicit `applications` binding is added elsewhere

#### Scenario: Wrapper payload is ignored
- **WHEN** `After(result, _) => { ... }` is parsed
- **THEN** the arm binds `result` and records one wildcard payload position

### Requirement: Class syntax

The parser SHALL recognize `class`, its fields and methods, `construct`, `this`, the visibility modifiers, `abstract`, `extends`, and `implements`, per `ZIRK_LANGUAGE_SPEC.md` section 7.

#### Scenario: Complete class
- **WHEN** `class User implements Serializable { public inmut id: Int32; construct(id: Int32) { this.id = id; } }` is parsed
- **THEN** a declaration with one implemented contract, one field, and one constructor is produced

#### Scenario: Combined inheritance and contracts
- **WHEN** `class Admin extends User implements Auditable, Clone { }` is parsed
- **THEN** a declaration with one superclass and two contracts is produced

#### Scenario: `construct` outside a class
- **WHEN** `construct` appears at the top level of the file
- **THEN** a diagnostic indicating that a constructor belongs to a class is emitted

#### Scenario: Multiple constructors and named arguments
- **WHEN** a class declares several `construct` and is constructed with reordered named arguments
- **THEN** the tree retains all signatures and each argument's labels for semantic resolution

### Requirement: Contract syntax

The parser SHALL recognize `interface` and `trait` with their methods, and allow a body only in those of a `trait`.

#### Scenario: Interface
- **WHEN** `interface Serializable { fn serialize(): String; }` is parsed
- **THEN** a declaration with a signature without a body is produced

#### Scenario: Trait with implementation
- **WHEN** `trait Greet { fn hello(): String { return "hello"; } }` is parsed
- **THEN** a declaration whose method has a body is produced

### Requirement: Generics syntax

The parser SHALL recognize type parameters `<T>` in declarations, type arguments at use sites, and constraints with `from`.

#### Scenario: Generic function with a constraint
- **WHEN** `fn serialize<T from Serializable>(value: T): String { }` is parsed
- **THEN** a type parameter with a constraint is produced

#### Scenario: Multiple type parameters
- **WHEN** `class Map<K, V> { }` is parsed
- **THEN** two type parameters are produced

#### Scenario: Type argument at the use site
- **WHEN** `mut b: Box<Int32>;` is parsed
- **THEN** the type carries an argument

#### Scenario: `<` that does not open generics
- **WHEN** `a < b` is parsed
- **THEN** a comparison, not a type argument, is produced

### Requirement: Data type syntax

The parser SHALL recognize `record`, value classes, enum variants with associated data, unions `A | B`, and aliases with `type`.

#### Scenario: Enum with associated data
- **WHEN** `enum Shape { Circle(Int32), Rect(Int32, Int32) }` is parsed
- **THEN** two variants with one and two associated types are produced

#### Scenario: Traditional enum mapping
- **WHEN** `enum Direction { North -> "N", South }` is parsed
- **THEN** `North` retains its explicit mapping and `South` remains without an explicit mapping

#### Scenario: Pattern with destructuring
- **WHEN** `match s { Shape.Circle(r) => r, _ => 0 }` is parsed
- **THEN** the pattern binds a name to the associated value

#### Scenario: Union
- **WHEN** `mut x: Int32 | String;` is parsed
- **THEN** a type with two members is produced

#### Scenario: Alias
- **WHEN** `type Id = Int32;` is parsed
- **THEN** an alias declaration is produced

### Requirement: Phase 3 does not yet parse the final function type

Phase 3's parser SHALL NOT yet accept the final syntax
`Function(Int32, Int32) => Int32` nor its alias `Fn(Int32, Int32) => Int32`.

This is a temporary restriction of the Phase 3 compiler. The syntax, signature
compatibility, and final escape are already decided in the canonical
checkpoint.

#### Scenario: Function type in an annotation
- **WHEN** a type annotation with the shape of a function signature is parsed
- **THEN** a diagnostic indicating that function types arrive in a later phase is emitted

#### Scenario: The lambda as an expression does not change
- **WHEN** `(a: Int32): Int32 => a + 1` is parsed in value position
- **THEN** a lambda is produced, the same as in the previous phase

### Requirement: Cast syntax

The parser SHALL recognize the postfix form `expr as T` and the prefix form `<T>expr`, per `ZIRK_LANGUAGE_SPEC.md` section 11.

#### Scenario: Postfix cast
- **WHEN** `mut v = source as String;` is parsed
- **THEN** a conversion to the named type is produced

#### Scenario: Prefix cast
- **WHEN** `mut v = <String>source;` is parsed
- **THEN** the same conversion as the postfix form is produced

#### Scenario: Cast that reinterprets memory
- **WHEN** a cast that requires `unsafe` is parsed
- **THEN** a diagnostic indicating that the low-level tier arrives in a later phase is emitted

### Requirement: Integer-width and `Float` literals

The grammar SHALL recognize an integer literal as any of the signed or unsigned widths when the context determines it, and a fractional literal (with optional scientific notation) as `Float`, both with `_` as a visual separator.

#### Scenario: Visual separator in a wide literal
- **WHEN** `1_000_000` is written
- **THEN** it is lexed as the integer `1000000`

#### Scenario: Scientific notation
- **WHEN** `1e2` is written
- **THEN** it is lexed as a `Float` literal with value `100.0`

### Requirement: `Char` literal

The grammar SHALL recognize a `Char` literal delimited by single quotes, capable of containing an extended Unicode grapheme of more than one code point.

#### Scenario: Literal of an ASCII character
- **WHEN** `'a'` is written
- **THEN** it is lexed as a `Char` literal

#### Scenario: Unclosed delimiter
- **WHEN** a `Char` literal does not have its closing quote before the end of the line
- **THEN** a lexical diagnostic is emitted

### Requirement: Bitwise and shift operators

The grammar SHALL recognize `&`, `|`, `^`, `~`, `<<`, `>>` as binary operators (`~` unary), at the precedence levels `ZIRK_LANGUAGE_SPEC.md` fixes for them, distinct from the logical `&&`/`||`.

#### Scenario: Precedence distinct from the logical one
- **WHEN** an expression combining `&` with `&&` is written
- **THEN** it is parsed according to the precedence of each operator, not as if they were the same

### Requirement: Interpolation in `String` literals

The grammar SHALL recognize `{expr}` within a `String` literal as an interpolated expression, with `\{` as the escape for a literal brace.

#### Scenario: Simple interpolation
- **WHEN** `"Hello, {name}"` is written
- **THEN** it is parsed as literal text plus an interpolated expression `name`

#### Scenario: Escaped brace
- **WHEN** `"\{not interpolated\}"` is written
- **THEN** it is parsed as literal text with braces, with no interpolated expression

### Requirement: Index expression grammar

The parser SHALL recognize `expr '[' expr ']'` as a postfix index expression at the same precedence tier as method call and field access, left-associative and chainable, valid both as an ordinary read expression and, when the receiver type permits mutation, as a simultaneous-assignment or ordinary assignment target. The grammar itself SHALL NOT restrict which receiver types support indexing — that restriction belongs to the checker.

#### Scenario: Index expression parses as a place
- **WHEN** `view[0] = 0x7f;` is parsed
- **THEN** it produces an index expression usable as an assignment destination, the same classification `expr.field` already receives

#### Scenario: Chained index and field access
- **WHEN** `a.field[i][j]` is parsed
- **THEN** it produces a left-associative chain of field access followed by two index expressions

#### Scenario: Indexing an unsupported receiver type is a checker error, not a parse error
- **WHEN** an expression whose type does not support indexing is subscripted
- **THEN** parsing succeeds and the checker rejects the expression, naming the receiver type
