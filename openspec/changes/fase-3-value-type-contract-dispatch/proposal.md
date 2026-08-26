## Why

`fase-3-objects-and-type-system`'s own design.md (closed 2026-08-19) recorded an explicit open question — "¿Una value class puede tener métodos virtuales?" — answered "no" at that change's own closure (14.3): a virtual method needs a runtime type descriptor, and storing one contradicts a value type's inline/compact representation. Consequently, `Checker::check_conformance` (`crates/zirk-sema/src/checker.rs:1634-1658`) fully checks a `record`/`value class`'s structural conformance to a contract it declares `implements`, but unconditionally rejects at lowering (`NOT_LOWERED`) the one thing that conformance would actually be *for*: reaching the value through the contract-typed reference, since "a value carries no vtable." The "no virtual methods" answer correctly rules out the naive approach (giving every value type its own inline descriptor), but does not by itself provide a design for what *does* let a contract-typed slot hold a value type — that question was never actually designed, only the wrong answer was ruled out.

Unlike `fase-3-abstract-dispatch` (a existing, proven mechanism generalized to a new declaration path) or `fase-3-generic-enums`/`fase-3-recursive-enums` (existing patterns extended), this is a genuine open design question requiring real exploration before implementation, the same way ADR-003 (memory strategy) needed exploration before the collector could be built.

## What Changes

- Design (in `design.md`) a representation that lets a `record`/`value class` implementing a contract be held through a contract-typed reference and dispatch correctly, without giving the value type itself a per-instance descriptor (preserving its inline/compact representation everywhere it is *not* held through a contract-typed slot).
- Deliver the chosen design: construction, conversion (value → contract-typed reference), and dispatch through the contract-typed reference for a `record`/`value class` implementing one or more contracts.
- Remove the checker's blanket rejection (`checker.rs:1650-1658`) once the chosen mechanism is real.

### Explicitly out of scope

- **Generic contract dispatch** (`contract Foo<T>`) — a separate, larger problem (no dispatch-table-per-instantiation mechanism exists at all, per earlier investigation); this change is scoped to a *non-generic* contract held by a *value* type. Combining both is future work once each is independently real.
- **Mutation through a contract-typed reference to a value type** — `record` is already immutable and `value class` has no observable identity; whatever this change's representation turns out to be, it must not introduce a way to mutate the original value's storage through the contract-typed view (matching the language's existing value-semantics guarantee — assignment/passing of a value type is already always a copy).
- **Changing anything about how a `record`/`value class` is represented when NOT held through a contract-typed reference** — its existing inline/compact representation (task 11.5) is unaffected; this change only adds a new representation for the specific case of a contract-typed view.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `zirk-contracts`: the existing "adopted through `implements`" requirement gets a new scenario confirming a `record`/`value class` adopter dispatches correctly through the contract type.
- `zirk-type-system`: `record`/`value class`'s existing "no observable identity"/inline-storage requirement is reaffirmed, extended with an explicit statement that holding one through a contract-typed reference does not grant it observable identity or mutability through that reference.

## Impact

- Affected code: `crates/zirk-sema/src/checker.rs` (remove the `not_lowered` gate; type the value→contract-typed conversion), `crates/zirk-ir` (a new conversion/dispatch IR shape — exact shape depends on `design.md`'s decision), `crates/zirk-codegen-llvm` (lowering that shape), `crates/zirk-runtime` (if the chosen design needs a runtime allocation/box, following the same collector-tracked-allocation pattern established by `fase-4e-weak`'s WeakCell and `fase-4e-clone`'s memoization table).
- Public documentation: `docs/handbook/13-appendices/07-current-limitations.md`/`12-feature-status.md`'s "value-type contract dispatch" line updated once delivered. `../zirk-lang-site` sync required.
- No breaking changes: this only makes a previously-rejected usage (naming a contract as a value type's type) succeed; nothing that compiles today changes behavior.
