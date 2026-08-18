## ADDED Requirements

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

