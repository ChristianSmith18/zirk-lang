## Context

`Checker::check_strict_alias` (D11, `fase-3-objects-and-type-system`) already implements the `mut`/`inmut`/`inmut::strict` matrix for the narrowest case: a new binding whose initializer is a bare `Expr::Path` naming another binding directly. `Checker::check_writable_field` (used by both `check_assign_target` — single assignment — and, since `fase-4d-declaraciones-multiples`, `check_multi_assign`'s per-position fan-out) checks only a field's own declared `inmut`, never the mutability of the reference used to reach it. This change closes that one specific gap: writing to `field.object.name` when `field.object`'s root binding is `inmut::strict`.

## Goals / Non-Goals

**Goals:**
- Reject `place = value;` (and the same position inside a simultaneous assignment) whenever `place` is a field projection and the root binding reached by walking through nested field accesses is declared `inmut::strict` — at any depth.
- Keep the diagnostic in the same family as the existing strict-alias violation (`E0430`), since it is the same underlying guarantee (a strict reference cannot produce a mutable write), with wording that correctly describes a projection write rather than an alias.

**Non-Goals:**
- Propagating a class field's own declared `inmut::strict` mutability across a projection boundary independent of the root binding (see proposal's "Explicitly out of scope"). This change only asks "what is the root binding's declared mutability", never "did any intermediate field along the way declare its own stricter mutability."
- Index/subscript writes — no `AssignTarget::Index` exists; nothing to change.
- Rejecting a mutating method call reached through a strict reference (`strictObj.mutate();`) — a different mechanism (method dispatch, not `AssignTarget`), left for its own change.

## Decisions

### D1: Walk the projection's root binding through nested field accesses only

A new helper, `fn root_binding_mutability(&self, expr: &Expr) -> Option<Mutability>`, recurses: `Expr::Path(ident)` resolves the binding and returns its declared mutability; `Expr::Field(inner)` recurses into `inner.object`; anything else (`Expr::This`, a call result, an index expression, …) returns `None` — deliberately conservative, matching D11's own precedent of only handling the shapes the checker can prove something about rather than guessing.

`Expr::This` returns `None` rather than `Some(Mutability::Mutable)`: `this` inside a method body is not a binding with its own declared mutability the way a local or parameter is, and asserting one here would be inventing a rule (whether a method invoked through a strict receiver should see a "strict-like" `this`) this change deliberately does not take on — see the proposal's explicitly-out-of-scope "calling a mutating method through a strict reference".

Alternative considered: fold this into the existing `check_strict_alias` (which currently only fires at declaration time, comparing a new binding's declared mutability against its initializer's). Rejected — `check_strict_alias` answers "is this new binding's mutability consistent with its initializer's", a different question from "is this projection write allowed given where its root came from"; conflating them would make `check_strict_alias` responsible for call sites it was never designed to see (a field write is not a new binding declaration).

### D2: `check_writable_field` gains the check directly, not a wrapper

`check_writable_field` already computes `field.object`'s type and resolves the field. Adding one lookup (`self.root_binding_mutability(&field.object)`) and one conditional diagnostic there keeps the existing single call site for "is this field write allowed" as the one place both `check_assign_target` (single assignment) and, transitively, `check_multi_assign` (simultaneous assignment, `fase-4d-declaraciones-multiples`'s own D4: "reuse exactly the single-assignment rule set") already go through — no new call sites needed in either assignment path.

### D3: Diagnostic reuses `codes::STRICT_ALIAS_VIOLATION` (E0430)

Same code, since it is the same normative guarantee ("a strict reference cannot produce a mutable write") observed at a different site (a projection write, not a new binding's declared mutability). A new, third message variant is added alongside the two `check_strict_alias` already produces, naming the root binding and the fact that it is `inmut::strict`.

## Risks / Trade-offs

- **[Risk] A false negative through an intermediate call or index expression** (`get_obj().field = x;`, once such syntax exists) **is a known, accepted gap**, not a bug this change introduces — `root_binding_mutability` returning `None` for anything but a Path/Field chain is deliberate conservatism, consistent with `check_strict_alias`'s own existing scope. → Mitigation: none needed now; flagged explicitly as future work if/when index writes or a stronger place-tracking mechanism is added.
- **[Risk] Confusing this with full "reachable-alias analysis"** (the roadmap's own phrase) **could make future readers think Phase 4e's `inmut::strict` line item is done.** → Mitigation: proposal and this design both spell out, twice, what is still missing (field-declared strictness across a boundary, method calls through a strict receiver) so the roadmap bullet is not marked complete by this change alone.
