# ADR-007 — Shape of the intermediate representation

- **Status:** accepted
- **Date:** August 13, 2026
- **Phase:** 1

## Context

`ZIRK_COMPILER_SPEC.md` section 4 requires the IR to be typed, target-independent and versioned, and to retain enough information for generic specialization, devirtualization, escape analysis, safety checks, vectorization and debug info.

It is also what gets distributed inside a `.zpkg` as `portable.ir` (section 4), so its shape is a contract with the future, not an internal detail of the compiler.

This decision is made in Phase 1, when the language subset is trivial. That is exactly the risk: what suffices for `if`/`else` and arithmetic may not suffice for classes, generics and concurrency.

## Decision

**Typed three-address code, over basic blocks, with local variables as slots and no own SSA.**

```
   Function
     ├── slots        (parameters and locals; read and written via Load/Store)
     └── blocks        (each ends in exactly one terminator)
           ├── instructions
           └── Return | Jump | Branch
```

Invariants enforced by the verifier:

- every block has exactly one terminator;
- values **do not cross blocks**: whatever needs to survive a jump travels through a slot;
- every instruction retains the source location that originated it;
- allocation operations do not name a memory strategy.

## Rationale

**Basic blocks and not a tree.** For the Phase 1 subset, a tree would have sufficed. It is discarded because:

- the analyses the spec requires to be preserved — escape analysis, vectorization — are flow analyses, awkward over a tree;
- Phase 2 introduces loops, `break` and `continue`, which over a tree would force a rework of the representation;
- the mapping to LLVM is direct, because LLVM is already exactly that.

**No own SSA.** Locals are slots with load and store, and promotion to registers is delegated to the backend. Own SSA — with phi functions and their upkeep — is real work that doesn't pay off until there are own optimizations. When those exist, it is introduced as a pass over this shape, not in its place.

That values do not cross blocks is the counterpart of that decision, and that's why the verifier checks it: it's the invariant that makes phi functions unnecessary.

**Typed independently of the frontend.** `zirk-ir` defines its own types instead of reusing those of `zirk-sema`. The IR is the boundary that gets distributed in a `.zpkg`; it must not move every time the frontend's internal type representation changes.

## Relationship with the memory strategy

The IR expresses **that** a value needs storage, never **how** it is obtained or released. No instruction names malloc, reference counting or garbage collection.

This is a direct requirement of [ADR-003](./ADR-003-memoria.md): the strategy is decided in Phase 4, and an IR that anticipates it would make that decision much more expensive. There is a test that pins down the restriction, because it's the kind of thing that erodes unintentionally.

## Consequences

- **Adding a language construct means adding instructions, not changing the shape.** Loops, `match` and closures fit into basic blocks without a redesign.
- **The verifier is part of the contract**, not a debugging tool. A lowering bug shows up as a precise message instead of an unreadable LLVM failure or, worse, a binary that silently miscompiles.
- **The IR is not yet versioned.** The spec requires it for `.zpkg`; that belongs to Phase 8, when there is something to distribute. Noted so it isn't discovered late.
- If in Phase 3 generics require information this shape doesn't retain, this ADR is revisited before deforming the IR with patches.
