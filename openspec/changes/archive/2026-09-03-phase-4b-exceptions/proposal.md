## Why

Phase 4b of the Zirk roadmap — exception completion — has one remaining concrete gap: the two compiler-known native safety checks that still abort (`arithmetic overflow` and `invalid cast`) are not yet catchable `RuntimeError` subclasses. The other pieces of the `Throwable` metadata contract (`suppressed`, lazy stack traces, deep immutability) depend on runtime/standard-library infrastructure that does not exist yet. This change therefore closes only the catchable overflow/cast gap, which is the last observable exception feature needed before Phase 5 structured concurrency.

## What Changes

- **Catchable arithmetic overflow**: signed and unsigned integer overflow throws `ArithmeticOverflowError` instead of aborting.
- **Catchable invalid cast**: runtime `as` and `<T>` cast failures throw `InvalidCastError`.
- Register the two new concrete `RuntimeError` subclasses and synthesize their IR method bodies, following the exact same pattern the four earlier catchable native errors used.
- Move the overflow and cast failure checks from the LLVM `trap_if`/`fatalError` path into the IR-level `throw_native_failure` path, so they dispatch through `try`/`catch`/`finally` like any other exception.

## Capabilities

### New Capabilities

- *None.* Every item in this change is a delivery of an existing partial capability.

### Modified Capabilities

- `zirk-errors`: extend the catchable implicit-failure set to `arithmetic overflow` and `invalid cast`.

## Impact

- `crates/zirk-runtime/src/failure.rs` — `zirk_rt_overflow` and `zirk_rt_invalid_cast` are replaced by catchable alternatives (or removed if no longer called).
- `crates/zirk-codegen-llvm/src/emit.rs` — overflow and cast failure branches emit exception-throwing code instead of `fatalError`.
- `crates/zirk-sema/src/checker.rs` — new concrete `ArithmeticOverflowError` and `InvalidCastError` class registrations.
- `crates/zirk-ir/src/lower.rs` — new lowering paths for checked arithmetic and checked casts that throw the new classes.
- `crates/zirk-cli/tests/corpus/` and per-crate tests — new fixtures for catchable overflow and invalid cast.
