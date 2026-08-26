## Why

`openspec/changes/archive/2026-08-19-fase-3-objects-and-type-system` closed with a documented gap: a user-declared `abstract class` parses and type-checks (structural conformance to it is verified via `require_abstract_conformance`, `checker.rs:1855`), but naming it as a type is unconditionally rejected at lowering with `NOT_LOWERED` (E0423, `checker.rs:2474-2483`) — "a value typed through it would need dynamic dispatch through its adopter... and that path does not exist for it yet." That claim is only true for *user-declared* abstract classes: the compiler's own built-in exception hierarchy (`Throwable`/`Error`/`RuntimeError`, registered natively in `checker.rs:1270-1330`+, each `ClassKind::Abstract`) already dispatches dynamically through exactly this mechanism today — `catch Throwable(e) { e.message() }` lowers `e.message()` through `InstKind::CallVirtual` (`lower.rs:5370-5389`), keyed on `method.overridden`, indexing into the concrete exception subclass's own method table. The vtable mechanism is proven in production; only the wiring that lets *user* code declare its own abstract class and have it participate is missing.

## What Changes

- Remove the checker's blanket rejection of a user-declared `ClassKind::Abstract` (`checker.rs:2474-2483`).
- Wire a user abstract class into the same method-table/virtual-index machinery the native exception hierarchy already uses: every method the abstract class declares gets a stable `index` in the flattened method list its adopters share (the same `method.overridden` / `CallVirtual` mechanism `lower.rs:5370-5389` already reads), so a value statically typed as the abstract class dispatches through its concrete adopter's own table, identically to how `Throwable`-typed dispatch already works.
- An abstract class itself still has no `construct`, no state, no layout, and is never directly instantiated — only the dispatch path through a *value* typed as it (held via a concrete adopter) is being delivered here, matching the existing spec text verbatim (`zirk-classes`'s "no constructor, body, state allocation, or layout contribution").

### Explicitly out of scope

- **Value-type contract dispatch** (a `record`/`value class` implementing a contract) — a different, harder representation problem (no descriptor to carry a vtable at all for an inline value); tracked as its own future change (`fase-3-value-type-contract-dispatch`, not yet proposed).
- **Multiple abstract-class inheritance** — `zirk-classes`'s existing rule (single `extends`, multiple `implements`) is unchanged; this only makes an already-legal single-inheritance-shaped abstract class actually dispatch.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `zirk-classes`: the existing "Abstract class contract" scenario is reaffirmed and extended with a dynamic-dispatch scenario — a value statically typed as the abstract class, holding a concrete adopter instance, dispatches to that adopter's override.

## Impact

- Affected code: `crates/zirk-sema/src/checker.rs` (remove the `not_lowered` gate at `check_class`'s `ClassKind::Abstract` arm, ~line 2474-2483; verify `require_abstract_conformance` and method-index assignment already produce a usable flattened method list for a user abstract class the same way they do for the native ones), `crates/zirk-ir/src/lower.rs` (verify `CallVirtual` lowering needs no change — the dispatch mechanism is receiver-type-driven, not abstract-class-declaration-driven, so this is expected to mostly "just work" once the gate is lifted, but this must be confirmed by real tests, not assumed).
- Public documentation: `docs/handbook/13-appendices/07-current-limitations.md`/`12-feature-status.md`'s "Several Phase 3 constructs..." bullet drops "abstract-class dynamic dispatch" once delivered. `../zirk-lang-site` sync required.
- No breaking changes: lifting a rejection only allows previously-rejected programs to compile.
