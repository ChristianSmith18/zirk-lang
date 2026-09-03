## Why

Phase 4 of the Zirk roadmap — failure handling, callable completion, and managed memory — remains the active gate before structured concurrency (Phase 5). Several concrete sub-features across 4b, 4c, 4d, and 4e are still listed as `partial` or `not started` in `ZIRK_FEATURE_STATUS.md` and `ZIRK_ROADMAP.md`. Closing them in one coordinated pass finishes the substrate that Phase 5's `Task`, `Channel`, `Mutex`, and `Atomic` will rely on: a complete exception and resource model, uniform callable values, and the remaining unsafe/memory primitives.

## What Changes

- **4b — Exceptions**: make the two remaining implicit native safety checks (`arithmetic overflow` and `invalid cast`) catchable `RuntimeError` subclasses; finalize suppressed-failure lists, lazy materialized stack traces, and deep thrown-object immutability.
- **4c — Resources**: implement grouped `match with` acquisition, surfaced/merged close failures, `Resource` transfer under `TransferableResource`, cancellation-aware cleanup, and dependent-resource lifetime analysis.
- **4d — Callables**: deliver general callable-type polymorphism (design D13) by boxing differently-captured closures behind a uniform `{function pointer, capture-block pointer}` representation.
- **4e — Managed memory and unsafe**: implement `Dependent<T>` references, automatic bounded native pinning (`Pin<T>`), and the resource/dependency/throwable cleanup interactions that require the complete memory model; complete `inmut::strict` reachable-alias analysis for field declarations and mutating method calls.
- **Status updates**: update `ZIRK_ROADMAP.md`, `ZIRK_FEATURE_STATUS.md`, and `docs/handbook/13-appendices/07-current-limitations.md` to mark Phase 4 complete; sync `../zirk-lang-site` status catalog where public claims change.

## Capabilities

### New Capabilities

- *None.* Every item in this change is a delivery or refinement of an existing normative capability.

### Modified Capabilities

- `zirk-errors`: extend the catchable implicit-failure set to `arithmetic overflow` and `invalid cast`; finalize suppressed-failure propagation, lazy stack-trace materialization, and deep `Throwable` immutability.
- `zirk-resources`: complete grouped acquisition, surfaced/merged close failures, `TransferableResource` transfer, cancellation-aware cleanup, and dependent-resource lifetime enforcement.
- `zirk-callables`: add general callable polymorphism across two or more differently-captured closures sharing one position.
- `zirk-object-memory`: add `Dependent<T>` references, automatic bounded native pinning, and the runtime/heap interactions they need.
- `zirk-memory-safety`: complete `inmut::strict` reachable-alias analysis for field declarations and mutating method calls; tighten native view provenance where needed.
- `zirk-ir-lowering`: add lowering rules for catchable overflow/cast, grouped resource cleanup, boxed callables, dependent references, and pinning.
- `zirk-native-codegen`: add runtime helpers, shadow-stack root paths, and emission rules for the new IR instructions.

## Impact

- `crates/zirk-sema/src/checker.rs` — new type rules for `Dependent<T>`, `Pin<T>`, `TransferableResource`, and general callable subtyping.
- `crates/zirk-ir/src/lower.rs` and `crates/zirk-ir/src/ir.rs` — new instructions for boxed callables, dependent refs, pinning, grouped resource cleanup, and catchable overflow/cast.
- `crates/zirk-codegen-llvm/src/emit.rs` and `crates/zirk-codegen-llvm/src/runtime.rs` — new runtime symbols and LLVM emission for the above.
- `crates/zirk-runtime/src/` — new runtime helpers for grapheme/pin/dependent-ref/throwable support.
- `crates/zirk-cli/tests/corpus/` and per-crate tests — new fixtures and assertions for every delivered behavior.
- `docs/init/ZIRK_ROADMAP.md`, `docs/init/ZIRK_FEATURE_STATUS.md`, `docs/handbook/13-appendices/07-current-limitations.md` — status closeout.
- `../zirk-lang-site` — status catalog update if public implementation claims change.
