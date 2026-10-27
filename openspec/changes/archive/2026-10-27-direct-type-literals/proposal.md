## Why

Zirk's singleton type syntax currently exposes `const(...)`, a call-shaped wrapper needed by an earlier parser design. It makes literal types noisy and contradicts the intended surface: types and literal values should be sufficient, including date and time values without constructor calls.

## What Changes

- **BREAKING**: Remove `const(expression)` as source syntax for singleton types; singleton type alternatives use a type name or a literal directly.
- Accept primitive, numeric, string, character, duration, null, and enum-case literals directly in type position. Temporal singleton values must use the owning constructors `Date(...)`, `Time(...)`, or `DateTime(Date(...), Time(...))`, with direct integer components only. Reject bare ISO temporal forms, `const(...)`, arbitrary functions, and other expressions. Tuple types and structural object types remain ordinary type expressions.
- Update grammar, singleton evaluation, type-system requirements, language documentation, tests, and demos. Keep ordinary `Date(...)`, `Time(...)`, and `DateTime(...)` expression constructors available for runtime values.

## Capabilities

### New Capabilities

### Modified Capabilities
- `zirk-grammar`: Define direct literal type atoms, require owning temporal constructors for temporal singleton values, and reject call-shaped singleton wrappers and bare ISO temporal spellings.
- `type-constant-evaluation`: Define canonicalization of literal values used directly as singleton types without evaluating arbitrary expressions or calls.
- `zirk-data-types`: Define singleton alternatives over supported literal values, including temporal literals, using direct syntax.
- `zirk-type-system`: Preserve singleton identity and widening rules for direct literal types without relying on a wrapper form.

## Impact

Affected components include the lexer and parser, AST representation, semantic constant canonicalization, type relation checking, formatter, LSP parsing/diagnostics, compiler and IDE tests, demos, handbook/reference documentation, and the companion `../zirk-lang-site` content synchronized from the handbook. This changes public syntax and normative semantics. Historical archived change artifacts remain historical records and are not rewritten.
