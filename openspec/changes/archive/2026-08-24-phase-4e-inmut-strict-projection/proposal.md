## Why

`openspec/specs/zirk-type-system/spec.md` ("Multiple bindings and simultaneous assignment are atomic at the language level" and "Reference mutability and strict aliases") already states, normatively, that "mutating a projection through an `inmut::strict` referent SHALL be rejected" and carries its own scenario for it ("Strict reference projection is a destination"). It is not implemented: `p.x = 5;` with `p` declared `inmut::strict` compiles today without a diagnostic. This is not a new discovery — `Checker::check_writable_field`'s own doc comment says so explicitly ("Where the object came from does not enter into it here... [that] lands with the rest of reference mutability"), and the change that introduced `inmut::strict` (`fase-3-objects-and-type-system`, task 5.15) documented in writing that it covered only the narrowest case — a `let`/`mut` initialized directly from another variable's bare name — and left "mutating a projection... through an `inmut::strict` referent" for a later phase. `fase-4d-declaraciones-multiples` (archived) hit this gap while reusing the single-assignment writability rule for simultaneous assignment and documented it rather than silently reproducing it further.

`docs/decisions/ADR-003-investigacion-fase-4.md`'s exploration of Phase 4e (24 de agosto de 2026) found that this is the one piece of that phase's scope that is fully self-contained: unlike `Weak<T>`, deep `clone()`, or a real collector, it does not depend on ADR-003's still-open memory-strategy decision — it is static alias analysis in the checker, exactly like the rest of D11's `mut`/`inmut`/`inmut::strict` matrix.

## What Changes

- The checker rejects writing to a field reached through a projection whose root binding (a local, or a function parameter) is `inmut::strict`, at any projection depth (`p.a.b.c = x;` is rejected the same as `p.x = x;`) — not just a direct rebinding of the root name itself.
- The existing `codes::STRICT_ALIAS_VIOLATION` (E0430) diagnostic is reused for this case (same violation category — a strict reference producing a mutable write — different site), with wording specific to "writing through a projection" rather than "producing an alias".
- `check_multi_assign` (from `fase-4d-declaraciones-multiples`) needs no change: it already delegates to the shared `check_assign_target`/`check_writable_field` path this change fixes, so the new rejection applies to simultaneous assignment automatically once the underlying rule is fixed.

### Explicitly out of scope

- **Field-declared strictness independent of the root binding's own mutability** — a class field itself declared `inmut::strict` (`FieldDecl.mutability`), read through an otherwise-`mut` chain (e.g. `holder.strictField.count = 5;` where `holder` is `mut` but `strictField` is declared `inmut::strict` on `Holder`). This change only propagates the *root binding's* mutability down a projection chain; a field's own declared strictness overriding what its container's mutability would otherwise allow is a different, deeper rule (whether/how strictness composes across a field boundary is not settled by the current spec text) and is left for a follow-up.
- **Index/subscript writes** (`arr[0] = x;`) — `AssignTarget` has no `Index` variant today; collections are Phase 7. Nothing to fix yet.
- **Calling a mutating method through a strict reference** (`strictObj.mutatingMethod();`) — a different escape path (through `this` inside the callee) than an assignment target, not covered by `AssignTarget`/`check_assign_target` at all; left for its own investigation.
- **Passing a strict reference into a `mut`-parameterized function and mutating through the parameter inside the callee** — already covered by this change's own mechanism (the parameter is itself a binding with its own declared mutability, resolved through the same scope lookup as a local), but not specifically tested beyond confirming the general root-binding walk handles it; noted here so it is not mistaken for new mechanism.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `zirk-type-system`: reaffirms the existing "Multiple bindings and simultaneous assignment are atomic at the language level" and "Reference mutability and strict aliases" requirements verbatim — no requirement text changes; this delta exists only so the archive step records the requirement as delivered for this specific scenario, not just specified.

## Impact

- Affected code: `crates/zirk-sema/src/checker.rs` (`check_writable_field`, and a new helper to walk a projection's root binding).
- No breaking changes: every program that compiles today keeps compiling, except a program that today silently mutates state through a projection off an `inmut::strict` binding — that program was already violating the language's own normative guarantee and starts being rejected, which is the point.
- Public documentation: none of `docs/handbook`'s existing `inmut::strict` material claims this case is implemented today (checked before writing this proposal), so no "not yet implemented" caveat needs removing. `docs/init/ZIRK_ROADMAP.md`'s Phase 4e bullet "Implement `inmut::strict` with reachable-alias analysis" is broader than this change (it also covers field-declared strictness across a projection boundary and strict-reference method dispatch, both explicitly out of scope above) — this change's own tasks note progress on that bullet without claiming to close it, and `docs/decisions/ADR-003-investigacion-fase-4.md` gets a short note that this specific gap, which its own exploration surfaced, is closed.
