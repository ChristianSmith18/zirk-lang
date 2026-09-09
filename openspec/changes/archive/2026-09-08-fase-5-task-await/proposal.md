## Why

`fase-5-executor-core` delivered the whole async runtime — the cooperative
single-threaded executor, task control blocks, stackful-coroutine suspension,
per-task garbage-collection roots, and the `zirk_rt_task_spawn` / `zirk_rt_task_await`
C-ABI — but no `.zrk` program can reach any of it. `task` and `await` still lex as
keywords and then emit the "not implemented yet" phase diagnostic.

This change makes the two simplest forms real end to end: `task expr`,
`task { block }`, and `await expr`. That is the smallest slice that turns
concurrency from a runtime demo into a language feature, and it is a coherent unit
— a program can now spawn work and consume its result. Everything structural on
top (`task scope`, sibling-failure propagation, cancellation, `cancellation
shield`, `await ... timeout`, `select`, channels, aggregation, Transfer/Share) is
a follow-up change against the same specs.

## What Changes

- **Grammar / AST / parser**: `task expr`, `task { block }`, and `await expr` are
  parsed as expressions (`TaskExpr` over a call or a block, `AwaitExpr` over a
  handle). `task` and `await` in these forms are removed from the "constructs
  outside the subset" phase diagnostic; `task scope`, `await ... timeout`,
  `select`, `cancellation shield`, `parallel`, and `thread` stay deferred.
- **Type system**: `Task<T>` is a known single-parameter generic type. `task expr`
  is typed `Task<T>` where `T` is the expression's type; `task { block }` is
  `Task<T>` where `T` is the block's result type. `await handle` on a `Task<T>` is
  typed exactly `T` — no implicit `Result`, nullability, or exception wrapping.
- **Single-consume enforcement**: the checker tracks a `Task<T>` binding linearly.
  A second `await` of the same statically tracked handle is a compile-time
  use-after-consume error that points at the first consuming `await`. An ignored
  `Task<T>` is diagnosed under the existing must-use policy and dischargeable with
  `_ = handle`.
- **Task-body capture**: the code a `task` runs is compiled as a boxed closure
  over a heap-allocated, garbage-collected capture block, reusing the Phase 4d
  `MakeCallable` / `CallCallable` machinery. A capture that would be an
  ambiguous shared mutable alias is **not** analyzed in this change (that is the
  Transfer/Share change); for now a task body captures by the ordinary closure
  rules and a `.zrk` fixture stays within value / projection / whole-reference
  captures.
- **IR + lowering**: a new `TaskStart` instruction (a lifted body target plus a
  boxed callable operand) yielding a `Task<T>` value, and an `Await` instruction (a suspension
  point) yielding the element value. `Task<T>` lowers to a one-word handle
  (`IrType::Task`, an ABI `i64`). `await` lowers to a call to `zirk_rt_task_await`;
  `task` lowers to building the body thunk plus a call to `zirk_rt_task_spawn`.
- **Native codegen**: emit the body thunk (`extern "C" fn(*mut c_void) -> usize`
  that unpacks the capture block, runs the lowered body, and returns the boxed /
  widened result), the `zirk_rt_task_spawn` call, and the `zirk_rt_task_await`
  call plus a resume label. Function calling convention unchanged (`ADR-017`).
- **Feature status / phasing**: `task expr` / `task { }` / `await expr` move to
  implemented across lexer → CLI; `zirk-feature-phasing` clarified so the
  still-deferred forms keep their diagnostic.
- **Documentation + `.zrk` fixtures**: `docs/init/ZIRK_FEATURE_STATUS.md`, the
  handbook `18-concurrency` chapter's `task`/`await` section, a `11-reference`
  `Task<T>` page, and CLI fixtures — spawn + await a value, spawn a block,
  two independent tasks awaited in turn, the second-await rejection, `_ = handle`.

## Capabilities

### New Capabilities

_None._ Implements requirements already in the `zirk-grammar`,
`zirk-type-system`, `zirk-ir-lowering`, `zirk-native-codegen`, and
`async-runtime-core` main specs (synced from the archived `fase-5-async-core`).

### Modified Capabilities

- `zirk-feature-phasing`: the bare `task` / `await` expression forms and
  `Task<T>` are delivered by this change; `task scope`, `await ... timeout`,
  `select`, `cancellation shield`, and the `Channel<T>` / `TaskSettlement<T>`
  types remain deferred to their own changes and keep the phase diagnostic.

## Impact

- **Affected crates**: `zirk-lexer` (already lexes `task` / `await`; contextual
  handling unchanged), `zirk-ast` (`TaskExpr`, `AwaitExpr` nodes), `zirk-parser`,
  `zirk-sema` (`Task<T>` type, `await` typing, linear single-consume check,
  task-body capture typing), `zirk-ir` + `lower.rs` (`TaskStart` / `Await`
  instructions, `IrType::Task`, verify), `zirk-codegen-llvm` (`emit.rs` body
  thunk + spawn/await calls; `runtime.rs` declares `zirk_rt_task_spawn` /
  `zirk_rt_task_await`), `zirk-diagnostics` (`SECOND_AWAIT` and any new codes).
- **Runtime**: no new symbols — `zirk_rt_task_spawn` / `zirk_rt_task_await` /
  `zirk_rt_run_main` already exist (`fase-5-executor-core`). Their provisional
  ABI (packed-`u64` id, `usize` result) is now committed to by generated code;
  a later `Task<T>` representation change would be a coordinated update.
- **Behavior of existing programs**: none — `task` / `await` do not compile
  today, so nothing depends on their absence beyond the phase diagnostic. Every
  existing `.zrk` fixture is unaffected.
- **Affected public documentation**: `docs/init/ZIRK_FEATURE_STATUS.md`,
  `docs/init/ZIRK_ROADMAP.md` (Phase 5 step 1, first slice), the handbook
  `02-handbook/18-concurrency/` chapter, `11-reference` `Task<T>`. Normative
  behavior is already specified (synced); this change moves implementation
  status, not spec text.
- **Companion repository `../zirk-lang-site`**: impacted through
  `ZIRK_FEATURE_STATUS.md`. After the branch merges, run
  `./scripts/sync-website-content.sh` with `--audit-date YYYY-MM-DD`.
