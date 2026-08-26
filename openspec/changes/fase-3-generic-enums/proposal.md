## Why

`openspec/changes/archive/2026-08-19-fase-3-objects-and-type-system` closed with a documented gap: a user-declared generic enum (`enum Bar<T> { ... }`) parses and type-checks (type arguments validated against constraints), but every instantiation except the two compiler-native ones (`Iteration<T>`, `Result<T,E>`) is unconditionally rejected at lowering with `NOT_LOWERED` (E0423, `crates/zirk-sema/src/checker.rs:4299-4305`). The rejection is not a missing feature at the mechanism level: `specialize_enum` (`crates/zirk-ir/src/lower.rs:752`) already builds one concrete `EnumLayout` per instantiation generically, substituting `T` in variant payloads, and already runs unconditionally over every entry of `checked.enum_instances` (`lower.rs:55-59`) — including instances a user enum would produce, if the checker let one through. The gate exists only because generic-class monomorphization (roadmap task 11.1/13.5) was scoped and verified for classes; nothing has yet verified the identical mechanism is sound for a user's own generic enum.

## What Changes

**Delivered:**
- Removed the checker's blanket rejection of a generic enum instantiation for anything other than `Iteration<T>`/`Result<T,E>` (`checker.rs:4295-4305`'s `is_native` condition) — a user's `enum Bar<T>` instantiated as `Bar<Int32>`, `Bar<String>`, etc. is accepted the same way the two native cases already are, for a flat single- or multi-type-parameter shape.
- A second, previously undocumented gate: `declare_enum` minted a generic enum's own type parameters through `enter_type_params`, which unconditionally reports `NOT_LOWERED` on first use (correct for a truly-unimplemented generic function/method, wrong for an enum) — fixed by minting ids the same way `register_class` does for classes. Without this, no user generic enum could even be declared, making the checker-gate removal alone a no-op.

**Found blocked, reported rather than fixed (judged out of "verify, don't build" scope):**
- A variant payload naming another generic instantiation (`Bar<Baz<T>>`) fails in shared generic-inference machinery (`infer_type_params`/`substitute`, which only handle a directly-`Base::Param` type, unlike `substitute_type` which already recurses correctly) — a checker-wide gap, not enum-specific.
- A self-referencing enum declaration (recursive, generic **or plain**) cannot be declared at all today — `declare_enum` resolves variant field types before registering the enum's own name, so even a non-generic `enum IntList { Nil, Cons(head: Int32, tail: IntList) }` fails. This is a different, earlier root cause than this change's own design anticipated (a `specialize_enum` memoization risk that turned out not to be reachable at all) — needs the same two-phase declare/resolve split classes already have (`register_class`/`declare_class_members`), applied to enums.

### Explicitly out of scope

- **Generic contract dispatch** (`contract Foo<T>`) — a materially different, larger problem (`fase-3-abstract-dispatch`'s own investigation found no dispatch-table mechanism exists for it at all, unlike enums where the mechanism already works generically); tracked as its own future change.
- **A generic enum with a method body** — Zirk enums are closed data without user methods regardless of genericity (`zirk-type-system`'s existing rule); nothing about this change touches that.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `zirk-type-system` (or wherever the closest existing generic-enum/monomorphization requirement lives — confirmed during design against actual current spec text): the existing generic-instantiation requirement is reaffirmed and extended to state explicitly that a user-declared generic enum lowers the same way the two compiler-native ones do, closing what the archived `fase-3-objects-and-type-system` change left as documented debt.

## Impact

- Affected code: `crates/zirk-sema/src/checker.rs` (`resolve_enum_reference`, ~line 4290-4305 — remove the `is_native` gate), `crates/zirk-ir/src/lower.rs` (`specialize_enum`, verify against new shapes — likely no functional change needed, but confirm via new tests, not assumption).
- Public documentation: `docs/handbook/13-appendices/07-current-limitations.md`/`12-feature-status.md`'s "Several Phase 3 constructs..." bullet drops "user generic... enums" once delivered. `../zirk-lang-site` sync required.
- No breaking changes: lifting a rejection only allows previously-rejected programs to compile; nothing that compiles today changes behavior.
