## Why

Phase 4e of the Zirk roadmap — managed memory and unsafe — is the remaining single-threaded foundation before structured concurrency (Phase 5). `Dependent<T>`, `Pin<T>`, `inmut::strict` field completion, and native view provenance are still marked as `partial` in `ZIRK_FEATURE_STATUS.md` and `ZIRK_ROADMAP.md`. Closing Phase 4e finishes the unsafe/memory substrate that Phase 5's `Task`, `Channel`, `Mutex`, and `Atomic` will rely on: stable native addresses, dependent references that keep their base alive, and a completed strict/immutable memory model.

## What Changes

- **4e — Managed memory and unsafe**: implement `Dependent<T>` references, automatic bounded native pinning (`Pin<T>`), and the resource/dependency/throwable cleanup interactions that require the complete memory model; complete `inmut::strict` reachable-alias analysis for field declarations and mutating method calls; track native view provenance for `Pointer.from(...).as_slice(n)`.

## Capabilities

### New Capabilities

- *None.* Every item in this change is a delivery or refinement of an existing normative capability.

### Modified Capabilities

- `zirk-object-memory`: add `Dependent<T>` references, automatic bounded native pinning, and the runtime/heap interactions they need.
- `zirk-memory-safety`: complete `inmut::strict` reachable-alias analysis for field declarations and mutating method calls; tighten native view provenance for bounded slices.
- `zirk-ir-lowering`: add lowering rules for dependent references and pinning.
- `zirk-native-codegen`: add runtime helpers, shadow-stack root paths, and emission rules for the new IR instructions.

## Impact

- `crates/zirk-sema/src/checker.rs` — new type rules for `Dependent<T>`, `Pin<T>`, and `inmut::strict` field/method completion.
- `crates/zirk-sema/src/types.rs` — new type constructors and representations for `Dependent<T>` and `Pin<T>`.
- `crates/zirk-ir/src/ir.rs` and `crates/zirk-ir/src/lower.rs` — new instructions for `DependentFrom`, `PinObject`, and `UnpinObject`.
- `crates/zirk-codegen-llvm/src/emit.rs` and `crates/zirk-codegen-llvm/src/runtime.rs` — new runtime symbols and LLVM emission for pinning and dependent references.
- `crates/zirk-runtime/src/` — new runtime helpers for dependent ref and pin support, plus GC tracing of dependent bases and pinned roots.
- `crates/zirk-cli/tests/corpus/` and per-crate tests — new fixtures and assertions for every delivered behavior.
