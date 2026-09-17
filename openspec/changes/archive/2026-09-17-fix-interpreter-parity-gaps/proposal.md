## Why

The browser interpreter diverges from the native pipeline on everyday programs: a
plain `helper();` call, `xs.add(v)`, `m.set(k, v)`, typed `catch`, `as` downcasts,
`Decimal` arithmetic, integer `/`, `**`, `x.abs()`, `s.split()`, and all temporal
parsing currently fail in the playground — some as internal `ZB198` faults that
look like compiler crashes, some as `ZB001` capability rejections for externs that
need no host authority at all. On the language side, `xs.length` is `UInt64`,
which makes the idiomatic `for mut i = 0; i < xs.length` loop a compile error, and
the checker rejects `<` on `Decimal` even though lowering already implements it.

These were found by auditing the pinned browser package against the native driver;
each has a confirmed reproduction.

## What Changes

- **Void-call contract**: a `Call`/`CallVirtual`/`CallContract`/`CallClosure` whose
  callee returns `Void` must not require a return value at frame pop. Fix the
  `result: Some` + `Return(None)` mismatch — either in lowering (`emit` vs
  `emit_effect` for `IrType::Void` calls) or in the interpreter's
  `ReturnDestination` handling — and cover both directions in tests.
- **Effect-instruction results**: instructions the interpreter executes as
  `Ok(None)` (`ListAdd`, `ListInsert`, `MapSet`, `SetAdd`, `Store`, `StoreField`,
  `ArrayListStore`, `Throw`, `SetSuppressed`, …) must not carry `result: Some`.
  Same fix locus as above; the verifier may gain a rule that a `Void`-typed
  instruction declaring a result is invalid IR.
- **Implement `InstKind::Retype`**: it is declared `BrowserSupport::Direct` but
  has no execution arm, so it falls into the `not implemented yet` internal error.
  This unblocks typed `catch`, `as`/`as?` downcasts, and passing a subclass where
  a supertype/interface is expected.
- **Browser intrinsics for pure externs**: implement the `zirk_rt_decimal_*`,
  `zirk_int_*`, `zirk_float_*`, `zirk_str_*`, `zirk_regex_*`, `zirk_char_*`,
  `zirk_duration_*`, and pure `zirk_rt_{date,time,datetime}_*` helpers inside the
  interpreter (via `zirk-runtime-core`, which is already a dependency) instead of
  rejecting them in preflight. Host-authority externs (`*_today`, `*_now_*`,
  journal/process/filesystem) stay excluded.
- **Uncaught-throwable message**: render `e.message()` instead of the internal
  `Heap(HeapId(N))` debug repr in `uncaught_exception`.
- **`length` comparability** (language-level decision): make `xs.length` usable in
  `i < xs.length` with a default `Int32` counter. Options to evaluate in design:
  (a) change `Array/List.length` to a signed type (`Int32`/`Int64`), noting the
  spec already documents `Map`/`Set` `length` as `Int32`; (b) allow mixed
  signed/unsigned comparisons with defined semantics; (c) add `count(): Int32` or
  an `indices` view as the idiomatic loop bound.
- **`Decimal` ordering**: the checker rejects `<`/`>`/`<=`/`>=` on `Decimal`
  (`expect_comparable` omits `Base::Decimal`) while lowering already emits
  `zirk_rt_decimal_cmp` for those operators. Align sema with the implemented IR —
  a shared int/decimal comparison fix also covers the `length` question if option
  (b) is chosen.

## Capabilities

### New Capabilities

### Modified Capabilities

- `zirk-ir-interpreter`: void-call return contract, `Retype` execution, browser
  intrinsic coverage for pure externs, uncaught-throwable rendering, and
  conformance fixtures for each fixed gap.
- `zirk-scalars`: `Decimal` ordering operators accepted by the checker;
  signed/unsigned comparison policy if option (b) is chosen for `length`.
- `zirk-collections`: `Array`/`List` `length` type if option (a) is chosen, or a
  documented `count`/`indices` member if option (c) is chosen.

## Impact

- `crates/zirk-ir/src/lower.rs`: `emit` vs `emit_effect` sites for `Void` calls
  and effect instructions.
- `crates/zirk-ir/src/verify.rs`: possibly a new rule rejecting `result` on
  `Void`-typed instructions.
- `crates/zirk-interpreter/src/lib.rs`: `Retype` arm, `caller_value` handling for
  `Void`, `is_browser_intrinsic` + `execute_browser_intrinsic` coverage,
  `uncaught_exception` rendering.
- `crates/zirk-sema/src/checker.rs`: `expect_comparable` for `Decimal`; numeric
  comparison rules if mixed signed/unsigned comparison is allowed.
- `crates/zirk-browser/fixtures/`: new conformance fixtures covering every fixed
  case (`void-call`, `list-add`, `map-set`, `typed-catch`, `decimal-ops`,
  `int-division`, `downcast`, …).
- Website: none required — the contract stays `apiVersion: 1`; the pinned package
  is rebuilt and the checksums regenerate.
