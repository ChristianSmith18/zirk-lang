## Context

Every managed object already carries a real header — `[type descriptor | next (mark bit) | size | fields...]` (`crates/zirk-codegen-llvm/src/emit.rs:556-559`) — and `CallContract`'s existing dispatch resolves a contract method by loading the receiver's descriptor and looking up the contract's table within it (`emit.rs:1125-1142`, `1204-1210`). This mechanism has zero awareness of *how* a receiver came to exist; it only ever reads a descriptor off a pointer. `fase-4e-weak`'s WeakCell already proved a compiler-synthesized allocation with its own descriptor composes cleanly with the collector without needing a user-visible class declaration behind it. `record`/`value class`'s own inline representation (roadmap task 11.5) is completely unaffected by anything in this change — this only concerns what happens at the moment a value is converted into a contract-typed reference.

## Goals / Non-Goals

**Goals:**
- A `record`/`value class` implementing a contract can be held through a contract-typed reference and dispatch to its own methods correctly.
- The value type's existing inline/compact representation, and every existing direct-call path on it (`method_of` already accepts `IrType::Value` the same way it does `IrType::Object`), is completely unchanged.
- No mutation is possible through the contract-typed reference — matches the language's existing value-semantics guarantee.
- Reuse the collector and `CallContract`'s existing dispatch unmodified — no new runtime mechanism, no special-casing in codegen for "is this receiver an object or a boxed value."

**Non-Goals:** generic contract dispatch, mutation through the contract-typed view, changing the value type's own unboxed representation (see proposal's "Explicitly out of scope").

## Decisions

### D1: A value-to-contract conversion boxes the value into an ordinary, collector-tracked heap allocation with a real descriptor — built the same way a class's own descriptor is, not a new kind of object

When a `record`/`value class` value is converted to a reference of a contract type it implements (an implicit conversion at a `return`, assignment, argument-passing, or collection-insertion site whose target type is the contract, mirroring exactly the conversions a class instance already undergoes for the same contract-typed positions), the value's fields are copied into a fresh heap allocation carrying: the same three-word header every object has, and a static descriptor built at compile time for this `(value type, set of implemented contracts)` pair — a method table in the same index order a class's own descriptor already uses, and `contract_instances` entries registered exactly the way `check_conformance` already registers them for a class (`checker.rs`, same data the existing dispatch already reads). The boxed value's own field layout inside the allocation is identical to its existing inline layout — nothing about *how the fields are stored* changes, only that they now sit behind a header.

Alternative considered: give every `record`/`value class` a permanent inline descriptor word, the same way a class always has one. Rejected — this is exactly the option `fase-3-objects-and-type-system`'s own closure (14.3) already ruled out: it would grow every value type's size and contradict the entire reason a value type gets to be inline and compact in the first place, imposing that cost on every value regardless of whether it is ever used through a contract.

Alternative considered: static per-call-site monomorphization (generate a specialized version of a function for each concrete value type it's ever called with through a contract parameter, avoiding any vtable). Rejected — contract-typed storage exists precisely for the *heterogeneous* case (e.g., a collection holding several different concrete adopters, `List<Shape>` mixing value-class shapes), which monomorphization cannot serve: there is no one concrete type to specialize against once storage genuinely needs to hold different adopters interchangeably. Boxing handles this uniformly, exactly the way a `class`-typed contract reference already does.

### D2: `CallContract`'s existing dispatch needs no changes

Since the box's descriptor is built with the same shape and the same `contract_instances` registration a class's own descriptor already has, `CallContract`'s existing lookup (load descriptor → find the contract's table within it → call) works on a boxed value with no special-casing at all — it cannot distinguish "this descriptor belongs to a boxed value type" from "this descriptor belongs to an ordinary class," and does not need to.

### D3: The box is read-only; no write path is ever generated for it

`record` is already fully immutable, and `value class` has no observable identity or any existing mutation surface beyond its own methods (which already dispatch directly on the *unboxed* value via `method_of`, untouched by this change). The box's own fields therefore need no store/write instruction at all — only the copy-in at construction time and reads through `CallContract`'s own field/method access. This is what keeps "no mutation through the contract-typed reference" true by construction rather than by a separate enforcement rule.

### D4: One box per conversion, no interning or deduplication

Converting the same value twice (e.g., passing it as two separate contract-typed arguments) produces two independent boxes — matching `fase-4e-weak`'s own WeakCell precedent ("each call allocates its own independent handle... simpler, and nothing in the spec requires identity between" two occurrences). Since a value type has no identity to begin with, there is nothing for two boxes of "the same" value to need to share.

## Risks / Trade-offs

- **[Risk] Every value-to-contract conversion allocates and copies** — a real cost, proportional to the value type's own field size, paid at every such conversion site. → Accepted: this matches the unavoidable cost of boxing a value type behind a dynamic-dispatch view in any comparable language (e.g., `Box<dyn Trait>` in Rust, autoboxing in Java) for the same scenario; a program that never converts a value type to a contract type pays nothing extra, and its ordinary inline usage is completely unaffected.
- **[Risk] Building a per-`(value type, contracts)` static descriptor at compile time duplicates a small amount of the same table-building logic classes already have** (`emit.rs:126-150`'s descriptor/method-table construction). → Mitigation: reuse that exact code path parameterized over a value type's own method/contract data rather than writing a second, divergent descriptor builder — confirmed as a concrete implementation task, not left as a vague aspiration.
- **[Trade-off] A boxed value's identity (`is`) is meaningless/unspecified** since two boxes of equal values are never deduplicated (D4) — accepted, since `record`/`value class` already have no observable identity by the language's own existing rule; nothing about boxing should be allowed to leak a new, accidental identity concept through the back door of `is` on a contract-typed reference. This must be confirmed explicitly during implementation (does `is` on two contract-typed references to separately-boxed equal values need an explicit rejection, or does it just correctly return `false` by virtue of comparing distinct addresses?) rather than left as an assumption.
