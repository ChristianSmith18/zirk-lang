## Context

`fase-4a-errores` closed the *expected*-failure quarter of `ZIRK_LANGUAGE_SPEC.md` section 9 (`Result<T,E>`). This change closes the *extraordinary*-failure quarter: `throw`/`try`/`catch`/`finally`, the `Error`/`Throwable`/`RuntimeError` hierarchy, and `throws` in function signatures, checked for "catch or declare" (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` section 3).

The full mechanism described there — stack unwinding through arbitrary intervening frames, lazily-materialized structured stack traces, `suppressed` populated from a cleanup failure during propagation, `Resource<E>`/`match with` — is large. This change scopes a first slice that both type-checks completely and actually runs, deferring the parts that need either a real unwinding runtime (stack traces with real frames) or exceptions already running (`Resource<E>`).

## Goals / Non-Goals

**Goals:**

- `throw expr;` / `throw;` (rethrow, only inside a `catch`).
- `try { } catch Type(name) { } ... finally { }`, at least one `catch` or `finally`.
- `throws Type (| Type)*` on a `fn`/method signature.
- The compiler-known `Error`/`Throwable`/`RuntimeError` abstract class hierarchy, and a minimal concrete `StackTrace` with an empty `to_string()`.
- "Catch or declare": an exception thrown or propagated from a `throws` call must be covered by a local `catch` or by the enclosing function's own `throws`. A `catch` that cannot be reached because an earlier one already covers its type is an error.
- Real execution through a compiler-inserted, invisible propagation channel (D1) — not true stack unwinding.

**Non-Goals:**

- Real stack traces (frames, source locations) — `stack_trace()` returns an empty `StackTrace`.
- `suppressed` populated from a `finally` cleanup failure during propagation.
- Converting the existing native `RuntimeError` failures (division by zero, overflow, invalid cast, out-of-bounds) to catchable exceptions — they still abort via `fatalError`'s mechanism, unchanged.
- Catch patterns with associated data (`catch NetworkError.Timeout(duration)`) — only `catch Type(name)` by class.
- `Fn(...) => T throws X` — function types have no syntax yet (D9, `fase-4a-errores`).
- `Resource<E>`/`match with`.

## Decisions

**D1 — exceptions propagate through one thread-local pending-exception slot, checked after every call that can throw, not through real stack unwinding.** The checker's own "catch or declare" analysis already proves, at compile time, that every exception a function's body can produce is either caught locally or named in that function's own `throws` — transitively, all the way up to wherever it is finally caught. That proof is what makes an `errno`-shaped mechanism sound (the same shape Swift's own `swifterror` convention uses, chosen independently and confirmed similar in spirit — `zirk-runtime/src/exceptions.rs`'s own doc comment): `zirk_rt_throw`/`zirk_rt_has_pending_exception`/`zirk_rt_take_pending_exception` are three new `extern "C"` runtime functions around one `thread_local!` `Cell<*const c_void>`. `throw expr;` calls `zirk_rt_throw` and then immediately dispatches locally (see D2); every call site whose target's `throws` is non-empty calls `zirk_rt_has_pending_exception` right after and, on a hit, dispatches the same way. Dispatch either jumps into the nearest enclosing `catch` that covers the *actual*, runtime-tested exception type (a new `InstKind::IsInstance`, `CheckedCast`'s own ancestor-list search answered as a boolean instead of asserted) or — having run every `finally` it passes on the way out (D3) — puts the exception back as pending and returns early from the current function with a placeholder value (a new `InstKind::Undefined`, LLVM's own `undef`, for whichever `IrType`s have no cheap zero the pre-existing `default_value` already covers). No landing pads, no personality function, no DWARF tables, no crossing an `extern "C"` boundary safely — but a real, verifiable propagation for every program the checker accepts, which is what "ejecuta, no solo chequea" needs. The original plan for this decision described a per-function "invisible `Result`-shaped return value" instead; that would have needed a second return channel `zirk-ir`'s `Function`/`Terminator::Return` do not have (a genuinely bigger change), and the thread-local slot needs none of it.

**D2 — a `catch` clause's handler block and binding slot are built *before* the `try`'s body is lowered, and a throwing call deep inside jumps straight into one.** `FunctionLowering` gained `try_stack: Vec<TryFrame>`, pushed with each `catch`'s pre-built `(class, handler block, binding slot)` right before the body is lowered and popped right after — so dispatch (D1) always has, at any point inside the body however deeply nested, the complete list of catches actually in scope, innermost frame last. Testing each catch's class needs the exception's *actual* runtime class, so dispatch takes the pending value (clearing the slot) before testing, and puts it back (re-throws it, unchanged) if it escalates past every active frame with no match — the same `zirk_rt_throw` a `throw` statement itself calls.

**D3 — `finally` is lowered by duplicating its block's IR at each exit path out of the `try`, not by a runtime unwind-cleanup table.** A `try`'s body can leave through: falling off the end normally, an uncaught throw propagating past every local `catch` (both handled: `Self::lower_finally_block` runs before the `Jump` to the shared continue block, and before dispatch escalates to the next enclosing frame). An explicit `return`/`break`/`continue` written *directly inside the try body* is a real, disclosed gap this pass does not close: `lower_return`/`lower_break`/`lower_continue` do not consult `try_stack` at all, so that exit skips the `finally` entirely rather than running it. Closing it needs those three lowering functions to walk `try_stack` the same way dispatch does — a contained, mechanical follow-up, not a design change, deferred here for time rather than difficulty. The checker still rejects a `return`/`break`/`continue`/`throw` written directly *inside* `finally` itself (unconditionally, a blunter rule than the spec's own "only when it would replace an active outcome" — sound, since forbidding more than the spec strictly requires never admits an unsound program).

**D4 — `Error`/`Throwable`/`RuntimeError` are registered the same "inject the tables directly" way `Result`/`Iteration<T>` are, as real (abstract) classes, not enums or contracts.** Unlike `Result`, these are ordinary classes a user's own exception type `implements`/extends. Two follow-on fixes this needed, both general (not exception-specific) gaps the existing class machinery had never been exercised against before an abstract-class-typed *value* existed to call a method on:

- `declare_class` seeds a class's own `methods` table from its `base`'s (`extends`), never from an `implements`-adopted `abstract class`'s — sound for the class's *own* dispatch, but wrong the moment a value is called through *that abstract class's own static type* (`catch Throwable(e); e.message();`): the abstract class's `message` sits at whatever index the checker happened to assign it, and a concrete implementer starting its own table from empty could put its override at a *different* index, so virtual dispatch through the abstract-typed reference would call whatever the wrong index means for that particular class. Fixed narrowly: `declare_class` also seeds from `self.native_exceptions` specifically (not any user-declared abstract class) before processing a class's own methods, so `message`/`code`/`cause`/`stack_trace` always land at the same index as `Error`/`Throwable`/`RuntimeError`'s own template.
- Codegen's `CallVirtual` built the indirect call's LLVM function-pointer type from `self.functions[&layout.methods[index]]` — the *static* receiver type's own table entry — which for an abstract class named the dummy body every never-instantiated `ObjectLayout` points its table at (`UNREACHABLE_ABSTRACT_METHOD`, a shared zero-argument `Void` function `lower()` emits once, needed only so codegen's own unconditional descriptor-building step has *something* real to reference). Fixed by building the indirect call's signature from the `CallVirtual` instruction's own already-correct return type and each argument's own LLVM type instead — the dynamically-loaded function pointer was always correct; only the *type* codegen called it through, used nowhere but for this instruction, could disagree.

Both are pre-existing gaps in ordinary Phase 3 class machinery that nothing before this phase had a reason to exercise (no program ever called a method through a value statically typed as an `abstract class` until `catch Throwable(e)` existed) — fixed here because this phase needed them, not scope creep.

## Risks / Trade-offs

- **The thread-local pending-exception slot (D1) is not itself part of any function's calling convention** — an ordinary function's real ABI is completely untouched, which is smaller a risk than the original per-function-return-channel plan would have carried. What every `throws`-target call site *does* pay is one extra `zirk_rt_has_pending_exception` call plus a branch; scoped to exactly those call sites, not every call in the program.
- **`return`/`break`/`continue` written directly inside a `try` body skip that `try`'s own `finally` (D3)** — a real, disclosed narrowing of the spec's "every exit path runs `finally`", not a soundness bug (nothing runs *twice*, nothing is skipped *silently* — it is written down here and the fix is mechanical, not a redesign).
- **`Checker::implements_abstract_class`'s transitive walk (used for `throw`/`catch`/`throws` type validation and catch coverage) is separate from `is_subclass_of`'s existing, non-transitive `abstract_bases.contains` check** used for ordinary `expect_assignable`. Deliberately not unified: `is_subclass_of` is exercised everywhere assignability is checked, and changing its behavior for abstract classes generally was more risk, on a tight verification budget, than adding one narrow, additive helper used only by this phase's own checks.
- **A deeply nested `try`/`catch`/`finally` duplicates the `finally` block once per exit path (D3)** — code size grows with the number of exit paths inside one `try`, not with nesting depth otherwise. Accepted for this pass; a shared cleanup landing block reachable by `Jump` from each exit would remove the duplication later without changing the semantics.

## Migration Plan

Additive over a pipeline that already compiles and runs end to end. Nothing existing changes behavior. Rollback: revert the merge, nothing later depends on this yet.

## Open Questions

- None outstanding — scope confirmed with the user before implementation started (`fase-4a-errores`'s own `AskUserQuestion` precedent).
