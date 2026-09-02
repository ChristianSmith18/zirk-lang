## Why

Phase 1 closed out a thin but complete pipeline: a minimal subset compiles down to a native binary. That subset has no loops, no closures, no `match`, no nullability, and no way to split a program across more than one file. No real program of even modest size can be written yet.

This phase completes the **language surface that does not depend on objects**. `ZIRK_ROADMAP.md` defines it as:

> Complete control flow, complete functions, `match` with basic exhaustiveness, nullability, and modules within a single crate.

By the end, a program with several functions, real control flow, and closures can be written and run -- still without classes or concurrency, which are Phase 3 and Phase 5.

## What Changes

### Control flow

- `for`, `for ... in` over iterable subset types, `while`, `loop`, `break`, `continue`.
- `if` stops being just a statement: it is an expression when both branches are type-compatible, per `ZIRK_LANGUAGE_SPEC.md` section 5.

### Complete functions

- Optional parameters (`name?`), default values, named parameters, and variadic parameters (`...values`).
- Closures/lambdas as function values, with safe immutable capture (`ZIRK_LANGUAGE_SPEC.md` section 6). Shared mutable capture is out of scope: it requires the concurrency analysis of Phase 5.

### `match`

- `match` as an expression, exhaustive, over a simple set of named constructors that this change introduces with the minimum necessary to have something real to check for exhaustiveness (see design.md, D1, on why this is not the full algebraic enums of Phase 3).
- `match` as a statement, controlling flow without producing a value.
- Patterns over literals, binding variables, and the `_` wildcard. Record destructuring, `match with` over `Resource<E>`, and patterns over unions are out of scope: they depend on records/unions/`Resource`, which are Phase 3 and Phase 4.

### Nullability

- `T?` as sugar for `T | Null`, safe access `?.`, fallback `??`, per `ZIRK_LANGUAGE_SPEC.md` section 4.

### Modules within a crate

- `share` publishes a declaration, `import` brings it into another file of the same crate, `use` enables globals without bringing in a qualified name.
- Everything within a single crate: no `init.zrk`, no external dependencies, no packages (`ZIRK_ROADMAP.md` Phase 6 and Phase 8).
- Phase 1's diagnostic for `import` -- "modules arrive in a later phase" -- is removed: now they do arrive.

### Explicitly out of scope

Classes, `construct`, inheritance, interfaces, traits, generics, records, value classes, algebraic enums with associated data, unions, `Result`, error handling, real memory (still a placeholder per ADR-003), concurrency, decorators, `init.zrk`, external packages, destructuring, `match with` / `Resource<E>`, and the stdlib beyond `println` and what is strictly necessary to iterate in `for ... in`.

## Capabilities

### New Capabilities

- `zirk-modules`: `share`/`import`/`use` within a crate -- name resolution across files, visibility, no `init.zrk`.

### Modified Capabilities

- `zirk-grammar`: loop grammar, `if` as an expression, optional/named/variadic parameters, closures, `match`, `T?`/`?.`/`??`, `share`/`import`/`use`.
- `zirk-type-system`: types and checks for everything above -- `match` exhaustiveness, closure typing, optional types, name resolution across files.
- `zirk-ir-lowering`: lowering of loops with `break`/`continue`, `if` as an expression, closures, `match`, safe access, and null coalescing.
- `zirk-native-codegen`: codegen for the constructs above -- loop jumps, closure environments, `match` dispatch, null checking.

## Impact

**Modified crates:** `zirk-ast`, `zirk-parser`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`. None new: unlike Phase 1, this phase extends a pipeline that already works end to end.

**No new external dependencies.**

**Risks:**

- **This phase's minimal enums must not become debt that Phase 3 has to undo.** They are scoped to constructors without associated data (see design.md D1) precisely so that Phase 3 extends them instead of rewriting them.

- **Closures are the first time the IR needs to capture state outside the function's stack frame.** This is a representation decision with the same long-term stakes the IR's shape had in Phase 1 (ADR-007).

- **Modules within a crate tempt one to already resolve Phase 3's `public`/`private`/`protected` visibility.** This phase only needs `share` (visible outside the file) versus private by default (visible only within the file); the full three levels are Phase 3, on classes.
