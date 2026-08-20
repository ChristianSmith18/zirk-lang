## Context

Decision D9 (`fase-4a-errores`'s design doc) rejected `Fn(P...) => R`/`Function(P...) => R` in every type position on purpose, deferring it whole. Every phase since has hit its shadow: `fase-4a` couldn't type `Result`'s `get_or_else(factory: Fn() => T)`; `fase-4b` couldn't type `Fn(...) => T throws X`; this session's own `proximos-pasos-fase-4.md` names closure escape as the most important open question blocking ADR-003 (the memory-strategy decision), because it is exactly the pattern that would exercise a real choice between escape analysis and any alternative — and today nothing can type a closure that outlives its creating frame, so the pattern cannot even be written.

Investigating this pass split D9 into two independent questions that the roadmap's single bullet ("permit closures to escape through compiler-managed capture environments") had bundled together:

1. **Is a closure's *representation* safe to escape its creating frame?** Yes, already — see D12 below.
2. **Can two *different* closures (different captures, therefore different LLVM layouts) share one annotatable type?** No, not without a new boxed representation — see D13 below, explicitly out of scope this pass.

## Goals / Non-Goals

**Goals:**
- `Fn(P...) => R`/`Function(P...) => R` real, parseable, checked, in every position the `zirk-callables` spec names.
- A named function or capture-less lambda freely satisfies/interchanges with a matching `Fn(...) => R` position.
- A single capturing closure literal, written directly at a `Fn(...) => R`-typed position, escapes correctly (returned, stored in a field, passed as an argument, bound to an explicitly-typed local — including the recursive-lambda case).
- `is` works between two callable values; `==`/`!=` stay rejected (already correct).
- A position that would need two *different* capturing closures is rejected with a clear, dedicated diagnostic — never silently miscompiled.

**Non-Goals:**
- General callable-type polymorphism (boxed/type-erased captures) — D13, deferred.
- Mutable captured bindings lifted into a shared cell — independent mechanism, deferred (see `proposal.md`).
- `.clone()` on a closure, captured projections as deep snapshots, permission/mutability compatibility, `throws` in callable types — see `proposal.md`'s "Explicitly out of scope" for why each is a non-issue or a separate concern.

## Decisions

**D12 — a closure's representation is already safe to escape; nothing in codegen or IR needs to change for the single-literal case.** `zirk-ir/src/lower.rs`'s `lower_lambda` (D10, `fase-2`) reads every captured value out of its slot *at closure-creation time* (`InstKind::Load` on each capture's slot, immediately packed into the `MakeClosure` value) — there is no pointer back into the creating frame anywhere in a `ClosureLayout`/`IrType::Closure` value. A whole-object capture is already a shared pointer (captured "by value" just copies the pointer, so the referent is naturally shared — the second capture rule in `CORE_LANGUAGE_SEMANTICS.md`). A primitive capture is already an independent copy (the first rule, "immutable captured values are snapshots"). Verified with real `.zrk` programs in this pass's own investigation, not assumed from reading the code. This is why this proposal is primarily a type-system change: the value a closure *is* was already correct; only naming its type, and letting the checker accept that type outside an inferred local, was missing.

**D13 (deferred, not built this pass) — general callable-type polymorphism needs captures heap-boxed behind a uniform calling convention.** Today every lambda literal gets its own non-interned `Base::Function`/`IrType::Closure` id (`Checker`'s lambda-checking code, ~line 6698: "each lambda gets a type of its own... its captures are part of its representation... two lambdas of the same shape are not interchangeable"), because `MakeClosure`/`ClosureLayout` bakes each closure's captures inline into its own LLVM struct (`{function pointer, capture_0, capture_1, ...}`, `crates/zirk-codegen-llvm/src/emit.rs`'s `MakeClosure`/`CallClosure`) — two closures with different captures are two different struct types at the LLVM level, full stop. Making `Fn(...) => R` accept *any* matching closure (not just one specific literal) needs a uniform representation every shape can produce: heap-allocate the capture block (one `zirk_rt_alloc`-style call per closure creation instead of "nothing is allocated," per D10's own comment, no longer accurate once this lands), and give every closure the same two-word shape — `{function pointer, capture-block pointer}` — with the target function taking one opaque capture-block pointer instead of individual capture parameters, unpacking it itself. This is a real new mechanism (new `InstKind`s or a runtime allocation call, a new codegen calling-convention path, and a decision about who frees the capture block — presumably nobody, matching this codebase's existing "objects are never freed in this phase" convention for everything else). Recommended as the next slice once this one is verified; not attempted here.

**D14 — a `Fn(...) => R`-typed position accepts a capturing closure only when exactly one distinct closure literal can reach it, checked structurally, not by full dataflow analysis.** The tractable, soundly-checkable rule: for a given `Fn(...) => R`-typed position (a local's declared type, a parameter's declared type at one call site, a function's declared return type, a field's declared type),
- a named function reference or a capture-less lambda always qualifies (uniform representation, D12);
- a capturing closure literal qualifies when it is written directly at that position (`ast::Expr::Lambda` as the initializer/argument/return/field-value expression itself — not a variable reference to a previously-bound closure);
- if a *function's declared return type* is a capturing `Fn(...) => R`, every `return` statement that yields a `Base::Function` value must yield the *same* lambda literal's span (the ordinary case: one `return <lambda>;`, or the same literal referenced from a variable that was itself only ever initialized from that one literal) — different spans means two different `IrType::Closure` layouts would have to share the function's one return slot, which is exactly D13's unbuilt mechanism;
- reassigning a `Fn(...) => R`-typed local from one capturing literal to a *different* capturing literal is rejected the same way — the local's own storage slot is sized for the first literal's `ClosureLayout` and cannot hold a second, differently-shaped one.

This is checked structurally (comparing lambda spans / `Base::Function` ids the checker already assigns, no new dataflow machinery) — implementers should treat "does this rule actually catch every case a straightforward structural check can catch, and cleanly reject what it cannot" as the acceptance bar, and report back (per this session's own standing practice) rather than force a case that turns out to need real dataflow analysis to decide soundly.

**D15 — `is` on two callable values compares the same underlying `IrType::Closure` id and function-pointer-plus-captures value; `==`/`!=` stay rejected.** `CORE_LANGUAGE_SEMANTICS.md`: "callable identity uses `is`; callables do not implement `==`." Today `is` on two closures fails with "has no identity to compare... a value is not a reference" (`checker.rs`) — closures are treated as pure values with no comparison at all, which undershoots the spec's own rule. Since a closure's representation is a flat value (D12, no heap box), "identity" here means bit-equality of the whole `{function pointer, captures...}` struct (an `IsInstance`/`Identical`-shaped codegen comparison, not a pointer comparison) — implemented the same way `is` already compares two `T?` nullable structs (`fase-4c`'s bugs-6/7 fix in `emit.rs`), generalized to a closure's own struct shape. `==`/`!=` remain rejected exactly as today (`E0403`, "there is no structural equality for code") — unchanged, already correct.

## Risks / Trade-offs

- **D14's "exactly one literal" rule is stricter than the language's final target** (which is full polymorphism, D13) — a real program that returns two differently-shaped closures from two branches will be rejected this pass, with a clear diagnostic pointing at why, not silently miscompiled. Accepted: matches this session's own established pattern (fase-4b's D5-D7, fase-4d's overflow/cast) of shipping the sound, tractable slice and naming the harder follow-up explicitly rather than attempting both at once.
- **D14 depends on the checker already tracking a lambda literal's exact span/id through ordinary type-checking** — if a case surfaces where the structural check cannot soundly tell "same literal" from "different literal" (e.g., a lambda literal produced through a generic function that itself doesn't preserve span identity), the correct response is to reject with a clear message, not to guess.

## Migration Plan

Additive over a pipeline that already compiles and runs. Nothing existing changes behavior — this only accepts programs that previously failed to parse (`NOT_IMPLEMENTED`) or type-check (`TYPE_MISMATCH`/E0403), never rejects anything that compiles today. Rollback: revert the merge; nothing later depends on this yet.

## Open Questions

- None outstanding for the scope actually implemented — confirmed with the user before implementation started (the general-polymorphism-vs-scoped-slice fork was resolved via `AskUserQuestion` before this design was written).
- Open for a future pass: D13 (general callable-type polymorphism via boxed captures), the lifted-shared-cell mutable-capture mechanism, and `.clone()` on a closure — see `proposal.md`.
