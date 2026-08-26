## Why

`fase-3-generic-enums` (merged) found and reported, rather than fixed, a real gap: a self-referencing enum declaration — recursive, generic **or plain** — cannot be declared at all today. `Checker::declare_enum` (`crates/zirk-sema/src/checker.rs:3261`) resolves every variant's associated field types (`self.resolve_type(&field.ty)`, `checker.rs:3338`) in the same single pass that registers the enum's own name into `self.enums` (the push happens only once the whole function returns) — so a variant payload naming the enum's own type (`enum IntList { Nil, Cons(head: Int32, tail: IntList) }`, no generics involved at all) fails to resolve with `E0402: unknown type: IntList`. This is not a `specialize_enum` memoization risk as `fase-3-generic-enums`'s own design anticipated — nothing self-referencing reaches that pass today, because the enum can't even be declared.

A `class` already avoids exactly this problem via a two-phase split: `Checker::register_class` (`checker.rs:2127`) mints the class's own type-parameter ids and pushes a placeholder `ClassType` (empty fields) into `self.classes` *immediately*, ahead of every other pass — "a field elsewhere in the file may write `Box<Int32>` before `Box` itself is declared" (`checker.rs:2151-2155`'s own comment). A later pass, `Checker::declare_class_members` (`checker.rs:2504`), resolves each class's actual field types, by which point every class (including a self-referencing one) is already a resolvable name. `declare_enum` has no equivalent split.

## What Changes

**Delivered:**
- Split `declare_enum` into two phases, mirroring `register_class`/`declare_class_members`: `register_enum` mints the enum's own type-parameter ids and pushes a placeholder `EnumType` (name, arity, type params — empty variants) into `self.enums` immediately, ahead of any variant field resolution; `declare_enum_variants` (run in the same later pass classes' own member resolution runs in) resolves each variant's associated field types, by which point every enum's and class's name — including the one currently being resolved — is already registered. This makes declaration order between an enum and a class (or another enum) that reference each other not matter, the same way it already doesn't for two classes.

**Found blocked during this change's own verification, safety-netted rather than fixed for real:**
- Implementation discovered design's own D2 prediction was wrong: a *directly* self-referential enum field (`Cons(tail: IntList)`, or `Tree<T>`'s own recursive fields) is not a `specialize_enum` memoization question at all — every enum lowers to an **inline-flattened struct** with no indirection anywhere in the pipeline (`IrType::is_managed_reference`, `EnumLayout`, and every codegen field-type computation all recurse with no cycle guard, on the documented, now-invalidated assumption that "a layout can never nest itself... already rejected upstream"). Lifting the old declaration-time rejection without anything else would have let this reach IR lowering and crash the compiler with a stack overflow (confirmed against a real build). Fixing this for real needs automatic heap indirection ("boxing") for a self-referential field — a new `IrType` case, a runtime allocation kind, GC root/tracing rules, and codegen for it — a substantially larger, cross-cutting IR/codegen/GC design that is **not** built in this change.
- Instead, this change adds a narrower, immediately-shippable safety net: `Checker::reject_unindirected_enum_cycles` rejects the specific unindirected shape (an enum whose own variant field embeds its own type, or another enum's, with nothing — no `Pointer<T>`, no class reference — in between) with a clean `NOT_LOWERED` diagnostic, naming the cycle, before it ever reaches IR lowering. This is what makes the declaration-order fix above safe to ship on its own: a program that never writes a directly self-referential enum field sees no behavior change at all; one that does gets a clear compile-time diagnostic instead of a crash.
- **Not delivered, and not safety-netted away — a real remaining gap**: actually *constructing and using* a self-referential enum (`IntList`, `Tree<T>`) still does not work; it is rejected, not lowered. That needs the boxing design above, as its own future change.

### Explicitly out of scope

- **Boxing a self-referential enum field** — the design and implementation needed to actually deliver `IntList`/`Tree<T>` construction/pattern-matching, per the finding above. Tracked as its own future change once someone designs the representation (new `IrType` case, runtime allocation, GC integration).
- **Nested generic instantiation in a non-recursive variant payload** (`Bar<Baz<T>>`) — `fase-3-generic-substitution-recursion`'s own, separate concern.
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
