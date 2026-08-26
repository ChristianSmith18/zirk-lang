## Context

`fase-3-objects-and-type-system` (merged) built the generic-instantiation machinery (`specialize_enum`, `lower.rs:752`) generically enough to serve any enum, but only ever exercised it against the two compiler-native generic enums (`Iteration<T>`, `Result<T,E>`), and gated every other instantiation at the checker (`checker.rs:4295-4305`). This change removes that gate and verifies the existing mechanism against real user shapes it was never tested against.

## Goals / Non-Goals

**Goals:**
- A user-declared `enum Bar<T>` instantiates and lowers correctly for arbitrary type arguments, the same way `Result<T,E>` already does.
- Multiple type parameters, nested generic instantiations in a variant payload, and a recursive generic enum (a variant referencing the enum's own generic type) all work correctly.
- No behavior change for the two native cases already delivered.

**Non-Goals:** generic contract dispatch (see proposal's "Explicitly out of scope").

## Decisions

### D1: Remove the `is_native` gate outright rather than allow-listing more cases

`checker.rs:4295-4305`'s `is_native` check exists purely as a scope fence, not because `Iteration<T>`/`Result<T,E>` receive any different treatment from `specialize_enum` itself — the archived change's own comment says the fence is there because monomorphization "was scoped and verified for classes," not because the mechanism needs per-case allowance. Removing the check entirely (rather than adding a third/fourth allowed enum id) is the correct fix: the two native enums were never special-cased in the specialization pass itself, only gated at this one checker call site.

### D2: Verify, don't assume, three specific shapes before considering this done

1. **Multiple type parameters** (`enum Either<L, R> { Left(L), Right(R) }`) — confirms substitution handles more than one type variable per instantiation.
2. **Nested generic instantiation in a payload** (`enum Wrapper<T> { Some(Option2<T>) }` where `Option2<T>` is itself another user generic enum, or `Bar<Baz<T>>`) — confirms `specialize_enum`'s substitution recurses correctly rather than only replacing a bare `T`.
3. **A recursive generic enum** (`enum Tree<T> { Leaf, Node(T, Tree<T>, Tree<T>)}`) — the type most likely to expose an infinite-specialization risk (each instantiation of `Tree<T>` needs `Tree<T>` again in its own payload, not a fresh distinct instantiation) or a layout-computation cycle if `specialize_enum` naively tries to fully materialize a variant's payload type before noticing it already has a layout for that exact instantiation. This is the one shape design flags as a real risk, not a formality — `specialize_enum` must memoize by instantiation identity (enum id + concrete type arguments) the same way generic class specialization already must, or this case does not terminate.

### D3: No new diagnostic needed for a case that should now succeed

Since this change is lifting a rejection, not adding a new one, no new error code is introduced. If verification (D2) surfaces a genuine unsupported shape (e.g., a truly unbounded case `specialize_enum` cannot handle), the existing `not_lowered` call stays for that specific residual case with corrected wording — not a blanket revert of D1.

## Risks / Trade-offs

- **[Risk] A recursive generic enum (D2's third case) could cause non-terminating specialization if instantiation memoization is missing or keyed incorrectly.** → Mitigation: verify memoization explicitly with a dedicated test before considering this change done; if `specialize_enum` does not already memoize by instantiation identity, this is a blocking finding to report, not something to work around ad hoc.
- **[Risk] Pattern matching/exhaustiveness over a generic enum's variants might have latent assumptions from only ever having seen two enums (`Iteration`/`Result`) exercise this path** (e.g., an off-by-one in a payload-count check, or an assumption both native enums happen to share). → Mitigation: exhaustiveness/pattern-binding tests against the new user shapes (D2), not just construction/specialization tests.
