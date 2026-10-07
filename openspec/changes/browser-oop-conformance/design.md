## Context

The interpreter executes verified IR only (ADR-020). The object model is already in the IR (`Alloc`, `LoadField`, `StoreField`, `CallVirtual`, `CallContract`, `IsInstance`, `CheckedCast`, `Retype`, `MakeClosure`, `BuildValue`, `BuildEnum`, `Discriminant`), and `fix-interpreter-parity-gaps` closed the `Void`-call, `Retype` and intrinsic gaps that blocked typed `catch`, downcasts and subclass-to-interface coercion. What is missing is systematic evidence and an honest public claim. `InstKind::Clone` is excluded today and is handled by `browser-deep-clone`.

The native `zirk run` pipeline stays the semantic authority. A construct is "supported in the browser" only when a shared fixture produces identical normalized output and completion category in both engines.

## Goals / Non-Goals

**Goals:**
- Establish, per OOP construct, the real browser status with fixture evidence.
- Fix divergences found by the audit without widening host authority.
- Make the support matrix and Feature Status accurate and fixture-backed.

**Non-Goals:**
- New IR instructions, new host effects, or a new `apiVersion`.
- Deep `clone()` (see `browser-deep-clone`) and execution observation (see `browser-execution-trace`).
- Multi-file or package execution in the browser, `Weak<T>`, pointers, native slices, concurrency, and anything else still excluded by ADR-020.
- Language-level semantic changes. A divergence rooted in ambiguous language semantics is escalated through an OpenSpec change on the owning language capability.

## Decisions

1. **Audit first, then fix.** The first task group produces a checked-in construct-by-construct matrix (construct, handbook source, native result, browser result, verdict) before any production code changes. Rationale: the interpreter's real state is already ahead of the documented one; fixing blind risks churn in `lib.rs` (about 8,000 lines) for constructs that already work. Alternative considered: write fixes speculatively per handbook chapter — rejected because it hides which gaps are real.
2. **Fixtures are the claim.** Each fixture is a single-file JSON unit reusing the existing format (`name`, `source`, `virtualName`, `expectedOutput`, `completion`, `exclusions`). Expected output is derived from native execution, never hand-written to match the interpreter. A construct without a passing fixture stays unclaimed.
3. **Group fixtures by teaching concept, not by crate.** Names follow `oop-<concept>-<case>` (for example `oop-polymorphism-list-of-supertype`). This keeps the matrix readable for the learning-surface consumer and bounds each fixture to one idea.
4. **Unsupported constructs fail at preflight with a capability code.** If the audit finds an OOP construct that lowers to an excluded instruction (for example `Clone`), the user sees a `ZB001`-family capability diagnostic naming the construct, not an internal `ZB198` fault.
5. **Discovery data derives from fixtures.** The compatibility information exposed through the existing `contract` operation is generated from the fixture catalog so it cannot drift from evidence.

## Risks / Trade-offs

- [Audit finds many real divergences and the change balloons] → Triage into must-fix (blocks a handbook-documented construct), defer (rare edge), and escalate (language ambiguity); only must-fix lands here, the rest becomes follow-up changes.
- [Native fixture runner needs LLVM and is slow in CI] → Reuse the existing native fixture script and run OOP fixtures in the same job; keep each fixture small.
- [Public claim outruns evidence] → The site-owned status catalog is reviewed by a human before any wording change, per repository policy; this change only supplies evidence.
- [Generic classes depend on monomorphization details] → Cover them with fixtures at two instantiations each so a lowering change shows up as a diff.
