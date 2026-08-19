## Context

`ZIRK_ROADMAP.md` names Phase 4 "Errors and memory" as one phase, but the two halves have almost nothing in common: the memory strategy needs a real evaluation against Zirk programs with closures and objects before it can close ([ADR-003](../../../docs/decisions/ADR-003-memoria.md)), while `Result<T,E>` is an ordinary generic algebraic enum that Phase 3's own generics work already made possible. This change is the error half only, and only the *expected*-failure quarter of that half — `Result<T,E>` — not the *extraordinary*-failure quarter (`try`/`catch`/`throw`/`Throwable`), which needs exception-propagation machinery (unwinding or an equivalent, `finally` running on every exit path) this change does not build.

The authoritative source for `Result`'s exact shape is `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 2 — marked "authorial source of truth", accepted 2026-08-16, explicitly superseding older conflicting descriptions elsewhere. It gives `Result`'s full method signatures, including four (`map`, `map_error`, `and_then`, `or_else`) that introduce a type parameter of their own beyond `Result`'s own `T`/`E`. `MethodInfo` (`zirk-sema/src/types.rs`) has no `type_params` field — only a class/contract/enum as a whole can be generic today, not one of its individual methods — so those four, plus `or_throw` (which additionally needs exceptions), are real, separate future work, not a mechanical extension of this change.

A prior pass of this same agent misdiagnosed "an enum cannot implement `to_string()`" as Phase 3 debt. It is not: `ZIRK_LANGUAGE_SPEC.md` section 7 states plainly that "enums are data-only and declare no user methods", restated independently by `ZIRK_STDLIB_SPEC.md` and the handbook's own enum pages. `docs/init/ZIRK_AGENT_PROMPT.md` is corrected. This closes off what would have been the wrong foundation for `Result`'s own API: it cannot be a table of enum methods, because that mechanism must not exist at all. It is recognized structurally instead, the same way explicit `.to_string()` on a native scalar already is (`Checker::is_native_to_string_call`, Phase 3b) — by receiver type and method name, not through a method table.

## Goals / Non-Goals

**Goals:**

- `Result<T,E>` as a compiler-known generic algebraic enum, matching `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 2's `enum Result<T, E> { Ok(T); Error(E); }`.
- The non-generic-method quarter of its API: `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `get_or_else`, `unwrap`, `unwrap_error` — called with ordinary dot syntax on a `Result` value.
- Mandatory consumption: a `Result`-typed bare statement is a compile error; `_ = expr;` is the explicit discard.
- `Base::Never`, the bottom type, and `fatalError(message: String): Never` as a compiler-recognized function.
- `unwrap`/`unwrap_error` on the wrong variant call `fatalError`, reusing the controlled-abort pattern every checked arithmetic/cast operation already has since Phase 1.

**Non-Goals:**

- `map`/`map_error`/`and_then`/`or_else`/`or_throw` — need per-method generic type parameters, a real new inference feature (see Context). Also out: any other method-level generic (this is not scoped to `Result` specifically, it just happens to be where the gap first matters).
- `try`/`catch`/`finally`/`throw`, `Throwable`/`RuntimeError`/`Error`/`StackTrace`, `throws` in function signatures, compiler-known `RuntimeError` subclasses for existing safety checks (division by zero, overflow, invalid cast, …) — `fase-4b-excepciones`.
- `Resource<E>`, `match with` — depends on exceptions.
- The memory strategy, `unsafe`/`Pointer<T>`, safe/weak/dependent references, `inmut::strict` — the rest of roadmap Phase 4; needs ADR-003's own evaluation process, not this change.
- General enum methods — the language does not have these and must not gain them; `Result`'s API is a structural special case, not a precedent to generalize.

## Decisions

(Decisions D1+ recorded here as the implementation proceeds, once each is actually made — see `tasks.md` for the sequencing.)

## Risks / Trade-offs

- **`Result<T,E>` is two type parameters where `Iteration<T>` (the only existing compiler-known generic enum) has one.** If the existing generic-enum-instantiation machinery assumes single-arity anywhere non-obvious, this is where it would surface. Audited as the first implementation task, before committing to the rest of the shape — mirrors how the Phase 3b integer-widths migration started with an audit before writing any new width.
- **Structural method dispatch by name (`is_ok`, `unwrap`, …) risks colliding with a real method a user's own class happens to declare with the same name.** Scoped narrowly: the dispatch only fires when the receiver's type is specifically the compiler-known `Result` enum instantiation, never for a class/contract/value-class receiver — exactly the same scoping `is_native_to_string_call` already uses for `to_string()`.
- **Mandatory consumption is a new category of check** — nothing today rejects a bare discarded expression statement of any type. Scoped to `Result` specifically (not a general `#[must_use]`-style annotation mechanism), since that is all the spec asks for; a general mechanism is not built speculatively ahead of a second type that would need it.

## Migration Plan

Additive over a pipeline that already compiles and runs end to end. Nothing existing changes behavior; `Result`, `Never`, `fatalError` and mandatory discard are all new surface. Rollback: revert the merge, nothing later depends on this yet.

## Open Questions

- **Exact wording of the mandatory-discard diagnostic and which statement shapes count as "used".** `docs/handbook/02-handbook/15-errors/02-handling-result.md` shows a bare call statement (`load(path);`) as the rejected shape and `_ = load(path);` as accepted — an assignment, a `return`, or an argument position all clearly count as "used"; resolved during implementation against those examples, not a genuine open design fork.
