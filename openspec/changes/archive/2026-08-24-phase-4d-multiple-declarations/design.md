## Context

The pipeline is `zirk-lexer` → `zirk-parser` (AST) → `zirk-sema` (checker) → `zirk-ir` (lowering) → `zirk-codegen-llvm`. Single-target declarations (`mut x: T = v;`) and single-target assignments (`x = v;`) already exist end-to-end. This change generalizes both statement forms to a comma-separated list on the binding/destination side, and (for assignment) a comma-separated list on the source side, without touching tuple types, tuple construction, or destructuring patterns — those are a separate, already-shipped grammar production and must stay separate productions.

The normative behavior is already fixed by `openspec/specs/zirk-grammar/spec.md` ("Comma-grouped declarations and assignments are explicit") and `openspec/specs/zirk-type-system/spec.md` ("Multiple bindings and simultaneous assignment are atomic at the language level"). This design covers only how the compiler delivers that behavior.

## Goals / Non-Goals

**Goals:**
- Parse `mut`/`inmut`/`inmut::strict` with N comma-separated simple names, one shared type annotation, and an optional comma-separated initializer list of arity N (or 0).
- Parse assignment with N comma-separated assignable places on the left and M comma-separated source expressions on the right as one AST node, preserving both N and M for semantic analysis even when they differ.
- Check and lower simultaneous assignment so every source is evaluated exactly once, left to right, before any destination is written, and writes commit left to right after — making `left, right = right, left;` a correct swap.
- Emit a targeted, distinct diagnostic for declaration-initializer arity mismatch and for assignment arity mismatch (not the generic type-mismatch diagnostic).

**Non-Goals:**
- Tuple types, tuple literals, or destructuring patterns (`(a, b) = pair;`, `let (a, b) = f();`) — unrelated existing/absent features, not touched or reused here.
- Comma-grouped declarations/assignments with anything other than a simple binding name on the declaration side, or anything other than an assignable place (identifier, field projection, index projection) on the assignment destination side.
- Parallel/concurrent evaluation of sources — "evaluate every source exactly once... before any destination write" is a sequencing guarantee, not a concurrency one.

## Decisions

### D1: Grammar production is a genuinely new statement shape, not a desugared tuple

Comma-grouped declarations and simultaneous assignment get their own AST nodes (e.g. `Stmt::MultiDecl { names, ty, permission, inits }` and `Stmt::MultiAssign { targets, sources }`), distinct from any tuple/record node. The spec explicitly requires this: "These forms SHALL remain distinct from tuple construction and destructuring patterns." Reusing a tuple-typed intermediate would risk requiring N and M to materialize an actual runtime tuple value, which is not what the spec asks for (no tuple type is created or observed here) and would make the arity-mismatch diagnostic route through generic tuple-arity checking instead of the targeted one the spec calls for.

Alternative considered: desugar `left, right = right, left;` at parse time into `let __tmp0 = right; let __tmp1 = left; left = __tmp0; right = __tmp1;`. Rejected — it would work for the swap case but loses the single simultaneous-assignment AST node the grammar spec's own scenario asserts ("the AST records one simultaneous assignment with two destinations and two source expressions"), and pushes the arity-mismatch check earlier than semantic analysis, where the spec says it belongs ("parsing preserves both arities so semantic analysis can emit a targeted count-mismatch diagnostic").

### D2: Single-target statements keep their existing AST nodes; multi-target is parsed only when a top-level comma is actually present

The existing single-name `mut x: T = v;` and single-target `x = v;` productions are left untouched (no arity-1 special case of the new nodes) to avoid changing existing diagnostics, spans, or IR lowering for the overwhelmingly common case. The parser only builds the new multi-target node when it sees a comma at the relevant position before the type annotation (declarations) or before `=` (assignment).

### D3: Simultaneous assignment lowers as evaluate-all-then-store-all, using existing single-place store lowering per destination

`zirk-ir`'s lowering evaluates every source expression into an IR temporary in left-to-right source order, then emits a store for each destination in left-to-right destination order using the corresponding temporary — reusing whatever store-emission path single-target assignment already uses per destination (place resolution, `inmut::strict` mutation checks already happen in the checker per D4 below, not re-derived in IR). No new runtime representation: temporaries are ordinary IR locals, the same mechanism already used for multi-argument call evaluation order.

### D4: Checker enforces per-position rules by reusing the single-assignment rule set, applied once per position

Duplicate-destination detection, `inmut`/`inmut::strict` rebinding rejection, and `inmut::strict` projection-mutation rejection are exactly the rules the checker already applies to a single assignment target — this change applies that same rule function to each of the N destinations plus a new cross-destination duplicate check (compare resolved place identity pairwise). No new mutability/permission rule is introduced; only the fan-out and the duplicate check are new.

### D5: Arity-mismatch diagnostics are new, dedicated diagnostic codes

Both the declaration-initializer case and the assignment case get their own diagnostic (new `E0` code each, following the project's existing diagnostic numbering — see `crates/zirk-sema` for the next free code), rather than reusing a generic arity/type-mismatch diagnostic, per the grammar spec's explicit call for "a targeted count-mismatch diagnostic."

## Risks / Trade-offs

- **[Risk] A naive lowering could store into an already-mutated destination before reading a later source that aliases it (e.g. `a, b = b, a;` where `a`/`b` overlap through a projection alias).** → Mitigation: D3's evaluate-all-sources-into-temporaries-first ordering means no source read ever happens after any destination write, by construction — this is the same fix the grammar spec's swap scenario requires.
- **[Risk] Parser ambiguity between a comma-grouped declaration/assignment and a future tuple-destructuring pattern reusing similar surface syntax (`a, b = ...` vs. `(a, b) = ...`).** → Mitigation: the parenthesized form is reserved for tuples; comma-grouped multi-declaration/assignment is deliberately bare (no enclosing parens), matching the spec's own examples (`mut first, second: String;`, `left, right = right, left;`), so the two surfaces don't collide today. If tuple destructuring is added later, its own change must keep the parenthesized form as the disambiguator.
- **[Trade-off] Reusing per-target checker rules instead of writing bespoke multi-target logic keeps the diagnostic behavior consistent with single-target assignment, but means any future change to single-target rules must remember it now has a second caller (the multi-target fan-out).** → Accepted: the alternative (duplicating rule logic) is worse for long-term consistency.
