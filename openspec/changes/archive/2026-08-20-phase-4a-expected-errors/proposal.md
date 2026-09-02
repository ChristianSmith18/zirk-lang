## Why

`ZIRK_ROADMAP.md` sets Phase 4 as "Errors and memory" — but it is, in reality, two decisions of very different size and independence pushed into the same phase number. The memory strategy (traced GC vs RC vs regions, `unsafe`/`Pointer<T>`, safe/weak/dependent references) is, in the roadmap's own words, "the project's highest-leverage decision", and [ADR-003](../../../docs/decisions/ADR-003-memoria.md) explicitly states that closing it needs "a language with closures and real objects to measure against" — evaluating it against toy Zirk programs would be exactly the mistake ADR-003 avoided making in Phase 0.

Error handling has no such dependency. `Result<T,E>` is a generic algebraic enum with two parameters — Phase 3 already built generic algebraic enums with one (`Iteration<T>`) — and does not need the memory strategy to be decided in order to exist. Separating it into its own phase (`fase-4a`, leaving `fase-4b-excepciones` and memory for later) avoids blocking real value behind the project's biggest and slowest decision.

This phase, moreover, covers only the *expected*-failure half (`Result<T,E>`) of `ZIRK_LANGUAGE_SPEC.md` section 9, not the *extraordinary*-failure half (`try`/`catch`/`throw`/`Throwable`). That second half needs an exception-propagation machine (stack unwinding or its equivalent, `finally` running on every exit, an implicit catchable `RuntimeError`) that is, again, a piece of its own — building it hastily alongside `Result` would risk both.

### Correction of a supposed Phase 3 debt

A prior pass by this same agent documented "an enum cannot implement `to_string()`, nor any method" as outstanding Phase 3 debt. That is a mistake: `ZIRK_LANGUAGE_SPEC.md` section 7 is explicit — "enums are data-only and declare no user methods" — the same rule that `ZIRK_STDLIB_SPEC.md` and the handbook's enum pages restate independently. `EnumType` having no `methods` field (unlike `ClassType`) is correct on purpose, not an omission; an enum's domain behavior is an external function that uses `match`, by design. `docs/init/ZIRK_AGENT_PROMPT.md` has already been corrected to reflect this.

This simplifies this phase's `Result<T,E>`: its API (`is_ok`, `map`, `unwrap`, …) cannot be a table of enum methods — no such thing exists, nor should it — so it is recognized structurally in the checker, the exact same pattern that explicit `to_string()` on a native scalar already uses (`Checker::is_native_to_string_call`/`lower_native_to_string_call`, Phase 3b): by name and receiver, not through a generic method table.

## What Changes

### `Base::Never`, the bottom type

- An expression of type `Never` is assignable to any type, and unifies with any type at a union point (`cond ? 5 : fatalError("...")`).
- Precondition of `fatalError`, which does not return normally.

### `fatalError(message: String): Never`

- A function recognized by the compiler, not a method — matches `ZIRK_LANGUAGE_SPEC.md` section 9 and the `zirk_rt_overflow`/`zirk_rt_division_by_zero`/etc. that the runtime has had since Phase 1, generalized to an arbitrary program message instead of a fixed compiler-determined cause.

### `Result<T,E>`

- Compiler-known generic algebraic enum: `Ok(T) | Error(E)`, with the same mechanism that registers `Iterable<T>`/`Iterator<T>`/`Iteration<T>` today (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 2, the most recent authorial source on errors — it takes precedence over older descriptions where they conflict).
- API with no method-level type parameter of its own — only the `T`/`E` that the `Result` instantiation already resolves: `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `unwrap`, `unwrap_error`. `unwrap`/`unwrap_error` on the wrong variant invokes `fatalError`, not an exception — it is a programmer assertion, not a recoverable failure (section 9).
- Mandatory consumption: a `Result` used as a discarded expression statement is a compile error; `_ = expr;` is the explicit allowed discard (`docs/handbook/02-handbook/15-errors/02-handling-result.md`).

### Explicitly out of scope

- **`map`, `map_error`, `and_then`, `or_else`, `or_throw`** — each introduces its own type parameter (`map<U>(transform: Fn(T) => U): Result<U,E>`, etc.), inferred from the callback's return at the call site. `MethodInfo` (`zirk-sema/src/types.rs`) has no `type_params` field of its own today — only the containing class/contract/enum can be generic, not an individual method — so this is a genuinely new type-inference piece, not a minor extension of the rest. `or_throw` also needs exceptions, which do not exist yet either. Separate change, after this one.
- **`get_or_else(factory: Fn() => T): T`** — unlike the other seven in-scope methods, its parameter is a function type. `docs/init/ZIRK_AGENT_PROMPT.md` is explicit (decision D9): "function types have no syntax yet" — a closure infers its type locally but cannot be annotated as a parameter, return, or field type, anywhere in the language, not just for `Result`. Fabricating a structural signature around that restriction would be exactly the shortcut D9 deliberately left pending for a future phase. Moved here, alongside the generic combinators.
- **`try`/`catch`/`finally`/`throw`, the `Throwable`/`RuntimeError`/`StackTrace` hierarchy, `throws` in function signatures** — `fase-4b-excepciones`, a separate change.
- **`Resource<E>`/`match with`** — depends on exceptions (section 9/10 of `ZIRK_STDLIB_SPEC.md`).
- **The memory strategy, `unsafe`/`Pointer<T>`, safe/weak/dependent references, `inmut::strict`** — the rest of roadmap Phase 4; needs its own decision process (ADR-003), does not fit in the same change as this.
- **Automatic propagation (`?`)** — the spec is explicit: "Zirk 1.x has no `?` propagation operator." It is not debt, it is a language decision.

## Impact

- Specs affected: `zirk-errors` (implements the `Result` half of the four requirements it already documents; the other three — exceptions, `Throwable`, `catch` patterns — remain as they were, for `fase-4b`), `zirk-type-system` (`Base::Never`, compiler-known `Result<T,E>`), `zirk-grammar` (`_ = expr;`), `zirk-ir-lowering`, `zirk-native-codegen`.
- No breaking changes: nothing that compiles today stops compiling.
