# ADR-005 — `String` opaque past the runtime boundary

- **Status:** accepted
- **Date:** August 12, 2026
- **Phase:** 0

## Context

`ZIRK_LANGUAGE_SPEC.md` section 3 defines `String` as a Unicode sequence *semantically indexed by graphemes and with an adaptive internal index/cache*. That is considerable work and belongs to Phase 7.

But Phase 1 already needs a `String`: the literal in `stdout.println("Hello from Zirk")`.

The trap: if Phase 1 represents `String` as "pointer to UTF-8 bytes" **inside the IR and codegen**, that assumption leaks into every site that touches strings, and undoing it in Phase 7 becomes a cross-cutting refactor.

## Decision

`String` is an **opaque type** to the compiler. Its layout is a private detail of `zirk-runtime` ([ADR-002](./ADR-002-runtime-staticlib.md)).

```
Zirk IR           ──▶  ZirkStr  (opaque handle, layout unknown to the IR)
LLVM codegen      ──▶  { ptr, len }   ← TODAY's representation, not a contract
zirk-runtime      ──▶  today:  plain UTF-8
                       Phase 7: + adaptive grapheme index
```

Every operation on strings goes through `extern "C"` runtime symbols. Neither the IR nor codegen inspect the content.

## Rationale

Adding grapheme indexing in Phase 7 must not require touching the lexer, the parser, the IR or codegen. With this boundary, the change stays contained within `zirk-runtime`.

The cost is an indirect call where direct access could otherwise exist. That is acceptable: `ZIRK_COMPILER_SPEC.md` section 5 assigns optimization to LLVM, and inlining trivial calls through a staticlib is exactly what LTO resolves in release builds.

## Consequences

- String literals materialize as LLVM global constants plus a runtime construction call, not as raw pointers handed to the user.
- The same discipline applies to future collections (`List<T>`, `Map<K,V>`, `Set<T>`): layout private to the runtime.
- If in Phase 11 it is measured that the indirection is a real cost in a specific case, it is optimized there with evidence — the boundary is not broken preemptively.

## Amendment — the handle is the observable identity

- **Date:** August 15, 2026
- **Reason:** the normative refinement defined `String` as a shared mutable reference with observable identity via `is`.

The later norm does not contradict this decision: it reinforces it.

The opaque handle **is** the identity that `is` compares. Two bindings that alias the same `String` share a handle and are identical; two distinct handles are not, even if their content matches. This adds nothing to the compiler — comparing two handles is comparing two pointers — and does not break opacity, because comparing identities is not inspecting content.

From this follows the division of responsibilities:

| Operation | Who resolves it | Why |
|---|---|---|
| `is` | handle comparison | identity **is** the handle |
| `==` | runtime | depends on content and canonical Unicode equivalence |
| hash | runtime | must be derived from the same canonical form as `==` |
| grapheme indexing | runtime | requires the adaptive index this boundary protects |
| normalization | runtime, with literals pre-canonicalized by the compiler | see [ADR-011](./ADR-011-identidad-e-igualdad-de-string.md) |

The compiler retains a single new responsibility: emitting literals already in canonical form. This is a transformation over the literal's text, not over the `String`'s representation, so the boundary remains intact.

**Opacity, far from getting in the way, is what makes all of this cheap:** the runtime can store, alongside the bytes, whatever flags and cached fields it needs —`is_ascii`, `normalization`, `grapheme_count`, `hash`— without any other layer knowing or having to change when they appear.
