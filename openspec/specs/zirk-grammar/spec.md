# zirk-grammar

## Purpose

Defines the grammar of the language and the construction of the syntax tree, together with the errors it reports.

The parser decides whether a program is well *formed*, not whether it makes *sense*: that belongs to the type system.
## Requirements
### Requirement: Function declaration

The parser SHALL recognize function declarations with the form from `ZIRK_LANGUAGE_SPEC.md` section 6: `fn`, name, list of typed parameters, an optional return type after `:`, and a body between braces. The `: ReturnType` annotation is optional when a body is present; its absence marks the signature for return-type inference by the checker rather than producing a diagnostic.

#### Scenario: Function without parameters

- **WHEN** `fn main(): Void { }` is parsed
- **THEN** a function declaration named `main`, with no parameters and return type `Void`, is produced

#### Scenario: Function with parameters

- **WHEN** `fn add(a: Int32, b: Int32): Int32 { return a + b; }` is parsed
- **THEN** a function with two typed parameters and return type `Int32` is produced

#### Scenario: Omitted return type with a body

- **WHEN** `fn main() { }` is parsed
- **THEN** a function declaration with no declared return type is produced
- **AND** no missing-return-type diagnostic is emitted

#### Scenario: Missing return type

- **WHEN** a signature without a body omits the return type, as in a `contract` operation, an `abstract` method, or an `extern "C" fn`
- **THEN** a diagnostic pointing to where the type was expected is emitted
- **AND** the help indicates that a signature without a body must declare its return type

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

`**` SHALL be parsed as an infix operator in the implemented subset, not
rejected as a deferred construct. Its right operand MAY be a unary expression,
so `2 ** -1` parses with the negation as the right operand.

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

#### Scenario: Exponentiation is right-associative

- **WHEN** `2 ** 3 ** 2` is parsed
- **THEN** the tree represents `2 ** (3 ** 2)`

#### Scenario: Exponentiation binds tighter than unary minus

- **WHEN** `-2 ** 2` is parsed
- **THEN** the tree represents `-(2 ** 2)`

#### Scenario: Exponentiation binds tighter than multiplication

- **WHEN** `3 * 2 ** 2` is parsed
- **THEN** the tree represents `3 * (2 ** 2)`

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

The parser SHALL emit a specific diagnostic for constructs that exist in the language but are not yet implemented, distinguishing them from syntax errors, and a distinct removed-construct diagnostic for constructs that were removed.

`concurrent`, `spawn`, and `parallel` are implemented and parse as real grammar. The removed words `task`, `await`, `select`, `scope`, `shield` keep their removed-construct diagnostic. `thread` remains deferred until its own change.

#### Scenario: Implemented concurrency form parses
- **WHEN** `concurrent { }`, `spawn f()`, or `parallel { }` is parsed
- **THEN** the parser produces the corresponding node and emits no diagnostic

#### Scenario: Removed construct still reported as removed
- **WHEN** `task f()` or `await h` is parsed
- **THEN** the removed-construct diagnostic names the replacement

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

The parser SHALL recognize optional parameters, default values, and a final variadic parameter prefixed with `...`. It SHALL additionally recognize `...expression` as an explicit spread argument in calls and `...expression` as a collection-literal element, while keeping declaration and expression contexts distinct.

#### Scenario: Variadic declaration
- **WHEN** `fn print(...values: Int32): Void { }` is parsed
- **THEN** a variadic parameter is produced

#### Scenario: Spread call argument
- **WHEN** `print(...values)` is parsed
- **THEN** one spread argument node is produced

#### Scenario: Rest destructuring pattern
- **WHEN** `[first, ...remaining]` is parsed in a destructuring declaration
- **THEN** a final rest binding is produced

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

#### Scenario: `.zkinit` is not imported from code

- **WHEN** an `import` of project configuration is found
- **THEN** the `.zkinit` out-of-scope diagnostic is emitted

### Requirement: Complete range and slice forms

The parser SHALL accept `start..end`, `start..=end`, descending bounds, colon
steps (`start..end:step` and `start..=end:step`), and braced interpolated
bounds/steps such as `0..{number}:2`. It SHALL reject the legacy
`start..end..step`, `.step(...)`, and `.reverse()` range-builder forms. Slice
syntax `[start:end:step]` SHALL remain unchanged.

#### Scenario: Descending stepped range

- **WHEN** source contains `10..=0:-2`
- **THEN** it represents an inclusive descending range with distance two

#### Scenario: Braced expression bound

- **WHEN** source contains `0..{limit + 1}:2`
- **THEN** the expression is retained as the range end operand

### Requirement: Collection literals with range expansion

The parser SHALL accept `[...]` as a collection literal and SHALL permit range
expressions among its elements. Range elements SHALL be distinguishable from
indexing and slicing by primary-expression context. Constructor argument lists
for `Array(...)` and `List(...)` SHALL also accept range expressions for
expansion.

#### Scenario: Range-only literal

- **WHEN** source contains `[0..100]`
- **THEN** it represents one collection literal whose expanded elements are `0` through `99`

#### Scenario: Mixed literal

- **WHEN** source contains `[1, 5..8, 9]`
- **THEN** it represents the expanded sequence `1, 5, 6, 7, 9`

#### Scenario: Reverse slice

- **WHEN** source contains `values[::-1]`
- **THEN** it represents a slice with omitted bounds and negative step

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

### Requirement: Scalar conversion expressions

The grammar SHALL retain the complete contained operator tree of
`Decimal(expression)`, `Float(expression)`, and `String(expression)` so
semantic analysis can evaluate that expression normally and convert only its
completed result.

#### Scenario: Nested arithmetic remains one conversion argument

- **WHEN** `Float64((a + 1) / (b * 2))` is parsed
- **THEN** the conversion contains the entire nested arithmetic expression as its single argument

#### Scenario: Conversion does not encode an operand domain

- **WHEN** `Float(3 / 4)` is parsed
- **THEN** the AST records an ordinary division argument and a Float conversion, without a contextual-domain marker

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

### Requirement: Capability manifests use one requires block

The manifest grammar SHALL accept one `requires` block in either an application or package manifest. The block SHALL contain unique capability identifiers from the finite Zirk capability vocabulary; it SHALL NOT require resource-specific scopes or build/runtime/test phase fields. The grammar SHALL reject `permissions`, `compile_permissions`, and `during` with a diagnostic that names the unified `requires` form. The parser SHALL preserve each capability name and source span for compiler diagnostics and editor actions.

#### Scenario: Application declares capability needs

- **WHEN** an application `.zkinit` contains a `requires` block with `filesystem.read` and `network.listen`
- **THEN** the manifest AST records those two capabilities and their source spans

#### Scenario: Legacy permission sections are rejected

- **WHEN** `.zkinit` contains `permissions`, `compile_permissions`, or `during`
- **THEN** the parser emits a targeted removed-syntax diagnostic pointing to `requires`

### Requirement: Project manifest grammar covers the canonical project model

The `.zkinit` grammar SHALL parse `project`, repeated `source` declarations, `build_targets`, `dependencies`, `dev_dependencies`, `globals`, and the single `requires` block. The project object SHALL support `name`, `version`, `type`, optional `authors`, `description`, `license`, and the type-specific `entry` or `root`. Dependency declarations SHALL support a logical package key with a version string or an expanded source object. Global declarations SHALL support the type shorthand and an expanded object with `type`, optional `default`, `from`, and `readonly`. Duplicate keys, malformed values, and unknown structural forms SHALL retain spans for diagnostics.

#### Scenario: Full application manifest parses

- **WHEN** a manifest declares application metadata, source roots, build targets, dependencies, globals, and requires using the canonical forms
- **THEN** the parser produces one project model with spans for each field and block

#### Scenario: Target list and source roots parse

- **WHEN** `.zkinit` contains `source "src"` and a `build_targets` list of quoted supported target identifiers
- **THEN** the manifest AST retains each path/target and its source span in declaration order

### Requirement: Source files accept executable top-level statements

The parser SHALL accept ordinary executable statements at source-file scope in addition to declarations. Imports SHALL remain static declarations and SHALL precede executable top-level statements. The parser SHALL retain the source order of top-level items so the checker and runtime can distinguish declarations from the entry/module body.

#### Scenario: Entry has sequential top-level statements

- **WHEN** a source file contains two executable statements at file scope
- **THEN** the parser preserves their source order as the file body

#### Scenario: Import follows executable code

- **WHEN** a static import appears after an executable top-level statement
- **THEN** the parser emits a targeted diagnostic requiring imports before executable statements

### Requirement: Named argument shorthand is explicit

The call grammar SHALL accept `name: expression` as an explicit named argument
and `name:` as shorthand for `name: name`. The shorthand SHALL accept only a
simple identifier, SHALL remain reorderable with other named arguments, and
SHALL NOT reinterpret a reordered bare positional identifier by matching its
spelling to a parameter. After the first explicit or shorthand named argument,
the call SHALL NOT accept a positional argument.

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

The grammar SHALL parse unsafe function modifiers and blocks and `commit` regions without introducing `async fn`. It SHALL NOT provide `task` blocks, `task scope`, `cancellation shield`, `await` timeouts, or `select` branches; the concurrency surface is `concurrent { }` / `parallel { }` / `spawn`, defined by their own changes.

#### Scenario: No task or select grammar

- **WHEN** source contains `task { }` or `select { }`
- **THEN** the parser emits the removed-construct diagnostic, not a grammar production

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

The parser SHALL recognize `class`, its fields and methods, `construct`, `this`, the visibility modifiers, `abstract`, `final`, `extends`, `implements`, and nested class members, per `ZIRK_LANGUAGE_SPEC.md` section 7. A method inside a type body SHALL be written `name(params): Return { ... }` without `fn`; a field remains `name: Type;`. The parser SHALL disambiguate a member after its modifiers and identifier by lookahead: `(` or `<` opens a method, `:` opens a field, `class` opens a nested class.

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

#### Scenario: Method without `fn`

- **WHEN** `class C { greet(): String { return "hi"; } }` is parsed
- **THEN** a method member is produced

#### Scenario: `fn` inside a type body

- **WHEN** `class C { fn greet(): String { return "hi"; } }` is parsed
- **THEN** a diagnostic is emitted indicating methods no longer use `fn`

#### Scenario: Function-typed field is not a method

- **WHEN** `class C { handler: fn(Int32): Void; }` is parsed
- **THEN** a field whose type is a function type is produced, not a method

#### Scenario: Nested class member

- **WHEN** `class Outer { class Inner { } inner class Bound { } }` is parsed
- **THEN** two nested class members are produced, the second marked `inner`

#### Scenario: Local class statement

- **WHEN** `fn f(): Void { class Local { } }` is parsed
- **THEN** a class declaration statement inside the body is produced

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

### Requirement: Type names are valid identifiers

The grammar SHALL accept `Decimal` and `Dec` as type names for the exact base-ten decimal scalar. It SHALL accept `Float16`, `Float32`, `Float64`, `Float128`, and `Float` as type names for the IEEE 754 binary family. The spellings `BinaryFloat`, `BinaryFloat16`, `BinaryFloat32`, `BinaryFloat64`, and `BinaryFloat128` SHALL NOT be accepted as type names and SHALL be rejected with a diagnostic.

#### Scenario: New exact decimal type name

- **WHEN** a type annotation or type expression uses `Decimal`
- **THEN** it is parsed as the exact base-ten decimal type

#### Scenario: New exact decimal alias

- **WHEN** a type annotation or type expression uses `Dec`
- **THEN** it is parsed as an alias for the exact base-ten decimal type

#### Scenario: New binary float type name

- **WHEN** a type annotation or type expression uses `Float64`
- **THEN** it is parsed as the 64-bit IEEE 754 binary floating-point type

#### Scenario: Old binary name is rejected

- **WHEN** a type annotation uses `BinaryFloat64`
- **THEN** the parser emits a diagnostic naming `Float64` as the replacement

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

### Requirement: Integer-width and fractional literals

The grammar SHALL recognize an integer literal as any of the signed or unsigned
widths when the context determines it. The grammar SHALL recognize a fractional
literal (with optional scientific notation) and no suffix as an exact-decimal
`Decimal` literal, and the same form with an `f`, `f16`, `f32`, `f64`, or `f128`
suffix as a `FloatN` literal. `Decimal` is the default fractional type and
SHALL have no literal suffix — a trailing `d` keeps its existing meaning as
the `Duration` days unit, never a decimal marker. The old `b` and `bN`
suffixes SHALL NOT resolve and SHALL be rejected with a diagnostic pointing to
`f`/`fN` for binary floats. `_` SHALL be accepted as a visual separator in all
numeric literals.

#### Scenario: Visual separator in a wide literal

- **WHEN** `1_000_000` is written
- **THEN** it is lexed as the integer `1000000`

#### Scenario: Scientific notation is exact

- **WHEN** `1e2` is written
- **THEN** it is lexed as an exact-decimal `Decimal` literal with value `100`

#### Scenario: Binary-float literal

- **WHEN** `1.5f32` is written
- **THEN** it is lexed as a `Float32` literal

#### Scenario: `d` is a duration unit, not a decimal suffix

- **WHEN** `1.5d` is written
- **THEN** it is lexed as a `Duration` literal of one and a half days, not a decimal literal

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

### Requirement: Member marker syntax

The parser SHALL recognize `#name` member markers on their own prefix position before a class member's modifiers. `#override` SHALL be the only defined marker in this change; an unrecognized `#name` SHALL produce a diagnostic. Markers SHALL apply only to instance methods.

#### Scenario: Override marker

- **WHEN** a class member begins with `#override` followed by a method header
- **THEN** the method is marked as an override

#### Scenario: Unknown marker

- **WHEN** a class member begins with `#unknown`
- **THEN** a diagnostic is emitted indicating `#override` is the only defined member marker

#### Scenario: Marker on a field

- **WHEN** `#override` precedes a field declaration
- **THEN** a diagnostic is emitted indicating markers apply only to methods

### Requirement: `final` modifier positions

The parser SHALL accept `final` before `class` and in the member-modifier sequence before a method. `final` in any other position (fields, constructors, `trait`, parameters, variables, and any expression position) SHALL produce a targeted diagnostic. Removed `record` and `interface` declarations SHALL produce their migration diagnostic.

#### Scenario: Final class and method

- **WHEN** `final class A { }` and `class B { final m(): Void { } }` are parsed
- **THEN** the final flags are recorded on the class and the method

#### Scenario: Final field rejected

- **WHEN** a class body contains `final x: Int32;`
- **THEN** a diagnostic is emitted indicating attributes use `inmut`, not `final`

### Requirement: The concurrent block

The parser SHALL recognize `concurrent { ... }` as a statement. Inside it, `inmut`
and `mut` declarations are branch bindings, plain statements are the block-body
branch, and `spawn` expressions are dynamic branches. The parser SHALL expose the
block's top-level binding list so the checker can build the dependency DAG.

#### Scenario: Concurrent block with named branches
- **WHEN** source contains `concurrent { inmut a = f(); inmut b = g(); }`
- **THEN** the parser produces a concurrent-block node with `a` and `b` as branch bindings

#### Scenario: Concurrent block with a spawn loop
- **WHEN** a `concurrent` block body contains `for x in xs { spawn h(x); }`
- **THEN** the parser records the `spawn` calls as dynamic branches of the block

### Requirement: The spawn expression

The parser SHALL recognize `spawn <call>` and `spawn { block }` as a prefix
expression, bindable (`inmut h = spawn f()`) or used as a statement (`spawn f()`).

#### Scenario: Spawn with a handle
- **WHEN** source contains `inmut h = spawn compute();`
- **THEN** the parser produces a binding whose initializer is a spawn node over `compute()`

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
`class` declaration. Writing a promoted parameter on an `enum`, `trait`, or
`abstract class` member — or on any other declaration — SHALL produce a
targeted diagnostic naming where promotion is allowed. Removed `record` and
`interface` declarations SHALL produce their migration diagnostic.

#### Scenario: Removed record declaration is rejected

- **WHEN** source attempts to declare a record with a promoted member
- **THEN** the removed-declaration diagnostic explains the migration to a `contract`, `Object`, `class`, tuple, or enum

### Requirement: Package and module declarations have explicit grammar

The grammar SHALL recognize `package qualified.name;`, `mod name { ... }`, and
`share mod name from "relative-path";` at their permitted source scopes. The
parser SHALL retain declaration and path spans for diagnostics and tooling.

#### Scenario: Package index declaration
- **WHEN** source contains `package std.io; share mod stdout from "./stdout";`
- **THEN** the syntax tree records the package identity and one module re-export

#### Scenario: Inline module declaration
- **WHEN** source contains `mod stdout { share fn println(): Void { } }`
- **THEN** the syntax tree records a named module with its published function

### Requirement: Package paths and relative paths are lexically distinct

An import source beginning with `./` or `../` and enclosed in quotes SHALL be
parsed as a relative source path. An unquoted dotted identifier path SHALL be
parsed as a logical package/module path. The parser SHALL reject ambiguous or
mixed spellings with a targeted diagnostic.

#### Scenario: Relative import
- **WHEN** source contains `import { User } from "./models/user";`
- **THEN** the source is recorded as a relative path without `.zrk`

#### Scenario: Logical package import
- **WHEN** source contains `import { stdout } from std.io;`
- **THEN** the source is recorded as an unquoted logical path

### Requirement: Package import and module member paths are parsed

The grammar SHALL recognize `::` only in an unquoted package import path, such
as `std.io::stdout`. Member access in values, calls, and type positions SHALL
use `module.member`.

#### Scenario: Qualified call
- **WHEN** source contains `stdout.println("hello");`
- **THEN** the parser records a two-segment qualified member access

#### Scenario: Qualified import
- **WHEN** source contains `import { println } from std.io::stdout;`
- **THEN** the parser records the package path `std.io` and module segment `stdout`

### Requirement: Contract declaration syntax
The parser SHALL recognize `contract` declarations containing required properties and operation signatures. Contract operations SHALL omit `fn` consistently with existing type-body method syntax. Contract members SHALL NOT contain implementation bodies; trait methods may contain bodies.

#### Scenario: Contract with a property and operation
- **WHEN** `contract Named { name: String; label(): String; }` is parsed
- **THEN** a contract declaration contains one property requirement and one operation signature

#### Scenario: Contract body is rejected
- **WHEN** a contract operation includes a method body
- **THEN** a diagnostic directs reusable implementation to a trait or class

### Requirement: Dynamic object literal keys and access
The parser SHALL accept identifier keys, quoted string keys, and quoted keys containing ordinary string interpolation in object literals. It SHALL parse `object[key]` as computed object-property access when the receiver is `Object`, while preserving collection and string indexing rules for other receiver types.

#### Scenario: Interpolated key
- **WHEN** `{ "{name}": value }` is parsed
- **THEN** the key is represented as a runtime string expression

#### Scenario: Computed property access
- **WHEN** `object[key]` is parsed with an `Object` receiver and a string key
- **THEN** it is represented as dynamic property access rather than a numeric collection index

### Requirement: Dynamic object spread and pattern syntax
The parser SHALL accept object spread and object rest syntax over dynamic objects. It SHALL accept object property patterns as partial tests and SHALL NOT treat them as closed-shape tests. Existing collection spread and rest grammar SHALL retain its separate iterable meaning.

#### Scenario: Object spread expression
- **WHEN** `{ ...source, name: "Ada" }` is parsed in expression position
- **THEN** it produces an object expression with a spread source and explicit override

#### Scenario: Object rest pattern
- **WHEN** `{ name, ...remaining }` is parsed as a pattern
- **THEN** it produces an object property pattern with one final rest binding

### Requirement: Remaining data type declarations parse
The parser SHALL continue to recognize enum variants with associated data, unions A | B, and aliases with type. It SHALL recognize recursive union/intersection type expressions, singleton value types, generic type expressions, and structural object type expressions in every type position. & SHALL bind more tightly than |; parentheses SHALL override precedence. T? SHALL remain equivalent to T | Null. The parser SHALL NOT recognize legacy record or interface declarations as active declaration syntax.

#### Scenario: Enum with associated data
- **WHEN** enum Shape { Circle(Int32), Rect(Int32, Int32) } is parsed
- **THEN** the variants retain their declared payload types

#### Scenario: Union type
- **WHEN** mut value: Int32 | String; is parsed
- **THEN** the declaration contains a two-member union type

#### Scenario: N-ary union and intersection
- **WHEN** type View = A & B & C | D | E; is parsed
- **THEN** the syntax tree preserves three intersection members in the first union branch and two additional union alternatives

#### Scenario: Parentheses control type precedence
- **WHEN** (A | B) & (C | D) is parsed
- **THEN** both parenthesized unions remain distinct operands of the outer intersection

#### Scenario: Nested type expressions in generic arguments
- **WHEN** type Values = List<A | B> & Iterable<C | D>; is parsed
- **THEN** each generic argument contains its complete nested union expression

#### Scenario: Boolean, numeric, and string singleton syntax
- **WHEN** type Status = 200 | 404; type Mode = "read" | "write"; type Flag = true | false; is parsed
- **THEN** each literal remains a singleton type expression rather than a value expression

#### Scenario: General compile-time value singleton syntax
- **WHEN** type Holiday = const(Date(2026, 12, 25)) | const(Date(2027, 1, 1)); is parsed
- **THEN** each const(...) expression is preserved for bounded compile-time evaluation

#### Scenario: Temporal value patterns
- **WHEN** a match uses a Duration literal or deterministic Date, Time, or DateTime constructor call as a pattern
- **THEN** the parser preserves the exact literal or constructor expression and its source span for constant checking

#### Scenario: Structural object type syntax
- **WHEN** a type alias declares required, optional, literal-valued, and read-only properties in an object type expression
- **THEN** each property modifier and property type is preserved in the syntax tree

#### Scenario: Legacy record declaration is rejected
- **WHEN** source declares record Point { x: Int32; }
- **THEN** a targeted migration diagnostic recommends contract and Object

### Requirement: Match syntax supports partial object patterns
The parser SHALL retain compatible-value, type, enum, union, regex, wildcard, and comma-grouped patterns while accepting nested object property patterns. Object patterns SHALL remain partial patterns and SHALL NOT imply a closed object shape.

#### Scenario: Nested enum payload and object pattern
- **WHEN** an enum payload pattern contains `{ id, name }`
- **THEN** the parser produces an object property pattern nested within the enum pattern

#### Scenario: Regex pattern remains supported
- **WHEN** a regex literal appears in a match pattern
- **THEN** it is parsed as a full-match regex pattern

### Requirement: Spread and rest syntax remains distinct by domain
The parser SHALL distinguish positional call and collection spread based on `Iterable<T>` from named object spread and rest. Object spread and rest SHALL accept open objects; collection rest remains an ordered suffix binding.

#### Scenario: Collection spread remains iterable syntax
- **WHEN** `[0, ...values]` or `sum(...values)` is parsed
- **THEN** the spread operand is represented as an iterable expansion

#### Scenario: Dynamic object spread parses
- **WHEN** `{ ...source, field: value }` is parsed in expression position
- **THEN** it produces a dynamic object spread expression
