## Why

`fase-3-generic-enums` (merged) found and reported, rather than fixed, a real gap: a self-referencing enum declaration — recursive, generic **or plain** — cannot be declared at all today. `Checker::declare_enum` (`crates/zirk-sema/src/checker.rs:3261`) resolves every variant's associated field types (`self.resolve_type(&field.ty)`, `checker.rs:3338`) in the same single pass that registers the enum's own name into `self.enums` (the push happens only once the whole function returns) — so a variant payload naming the enum's own type (`enum IntList { Nil, Cons(head: Int32, tail: IntList) }`, no generics involved at all) fails to resolve with `E0402: unknown type: IntList`. This is not a `specialize_enum` memoization risk as `fase-3-generic-enums`'s own design anticipated — nothing self-referencing reaches that pass today, because the enum can't even be declared.

A `class` already avoids exactly this problem via a two-phase split: `Checker::register_class` (`checker.rs:2127`) mints the class's own type-parameter ids and pushes a placeholder `ClassType` (empty fields) into `self.classes` *immediately*, ahead of every other pass — "a field elsewhere in the file may write `Box<Int32>` before `Box` itself is declared" (`checker.rs:2151-2155`'s own comment). A later pass, `Checker::declare_class_members` (`checker.rs:2504`), resolves each class's actual field types, by which point every class (including a self-referencing one) is already a resolvable name. `declare_enum` has no equivalent split.

## What Changes

- Split `declare_enum` into two phases, mirroring `register_class`/`declare_class_members`: a `register_enum` phase that mints the enum's own type-parameter ids and pushes a placeholder `EnumType` (name, arity, type params — empty variants) into `self.enums` immediately, ahead of any variant field resolution; a `declare_enum_variants` phase (run in the same later pass classes' own member resolution runs in, so an enum and a class can reference each other regardless of declaration order) that resolves each variant's associated field types, by which point the enum's own name — and every other enum's and class's — is already registered.
- This enables a self-referencing enum declaration, both non-generic (`enum IntList { Nil, Cons(head: Int32, tail: IntList) }`) and generic (`enum Tree<T> { Leaf, Node(value: T, left: Tree<T>, right: Tree<T>) }`), to be declared at all.
- Combined with `fase-3-generic-enums`'s own delivered generic-enum lowering, and once `fase-3-generic-substitution-recursion` lands (only needed if a recursive enum's own recursive field also nests a *different* generic instantiation — the base recursive case `Tree<T>` itself does not need it, since `Tree<T>`'s own recursive field is exactly `Tree<T>`, a direct match, not a nested distinct instantiation), a recursive generic enum should specialize and lower correctly through the existing `specialize_enum` mechanism — this change's own scope is the *declaration*-time fix; verifying specialization and lowering for a real recursive (generic and non-generic) enum end-to-end is this change's own closing task, not deferred to a third change.

### Explicitly out of scope

- **Nested generic instantiation in a non-recursive variant payload** (`Bar<Baz<T>>`) — that is `fase-3-generic-substitution-recursion`'s own, separate concern (a substitution-machinery gap, not a declaration-order one). This change's own recursive-enum verification only needs the direct self-reference case to work, not the general nested-instantiation case.
- **Cross-referencing between an enum and a class that also needs a two-phase split on the class side for some new reason** — classes already have their own two-phase split; this change only extends the same shape to enums, it does not change how classes work.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `zirk-type-system` (or the closest existing enum-declaration requirement — confirmed during design against current spec text): no prior requirement explicitly forbade or permitted a self-referencing enum; a new scenario is added confirming a recursive enum (generic and non-generic) declares and lowers correctly.

## Impact

- Affected code: `crates/zirk-sema/src/checker.rs` (`declare_enum` split into `register_enum`/`declare_enum_variants`, and whatever top-level declaration-pass driver currently calls `declare_enum` once needs to call the two phases at the right points relative to class declaration's own two phases — confirmed against the actual current pass ordering during implementation, not assumed).
- Public documentation: `docs/handbook/13-appendices/07-current-limitations.md`/`12-feature-status.md`'s note (added by `fase-3-generic-enums`) about "self-referencing enum declaration... blocked" is removed once delivered.
- No breaking changes: this only makes a previously-rejected declaration shape succeed; nothing that compiles today changes behavior.
