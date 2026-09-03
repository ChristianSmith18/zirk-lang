## Why

Phase 4 of the Zirk roadmap must be complete before the concurrency work in Phase 5 can begin. This change narrows the broader `fase-4-cierre-completo` effort to the remaining 4d Callable work: general callable-type polymorphism, boxed captures, and callable cloning. Closing Phase 4d callables is the final piece of the single-threaded value substrate that later phases rely on.

## What Changes

- **General callable-type polymorphism (design D13)**: a `Fn(P...) => R` binding can hold closures with different capture sets.
- **Boxed capture representation**: escaping or stored captured closures lower to a two-word `{function pointer, capture-block pointer}` value, with a heap-allocated, GC-tracked capture block.
- **`.clone()` on callables**: a callable can be cloned when every captured value is `Clone`, producing an independent deep copy of its capture block.

## Capabilities

### New Capabilities

*None.*

### Modified Capabilities

- `zirk-callables`: add general callable polymorphism across two or more differently-captured closures sharing one position; box escaping captures; support `.clone()` on callable values.

## Impact

- `crates/zirk-sema/src/checker.rs` — new subtyping rules for `Fn(...)` callable values and escape/alias analysis for boxed captures.
- `crates/zirk-ir/src/ir.rs` and `crates/zirk-ir/src/lower.rs` — `MakeCallable`/`CallCallable` instructions and lowering of captured closures to the boxed representation.
- `crates/zirk-codegen-llvm/src/emit.rs` and `crates/zirk-codegen-llvm/src/runtime.rs` — LLVM emission and runtime symbols for boxed callables and clone.
- `crates/zirk-runtime/src/` — runtime helpers for callable allocation and clone.
- `crates/zirk-cli/tests/corpus/` and `crates/zirk-ir/tests/` — new fixtures and assertions for polymorphic assignment, returned/stored closures, and `.clone()`.
