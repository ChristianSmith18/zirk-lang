## Context

`fase-5-executor-core` delivered the runtime: `zirk_rt_run_main` runs `main` as
the cooperative executor's root task, and `zirk_rt_task_spawn(body: extern "C"
fn(*mut c_void) -> usize, arg) -> u64` / `zirk_rt_task_await(id: u64) -> usize` /
`zirk_rt_task_is_done` are defined and Rust-tested, with a **provisional** ABI
(packed-`u64` `TaskId`, `usize` result). Nothing in the compiler calls them.

`task` and `await` lex today (`Keyword::Task` / `Keyword::Await`) and
`Keyword::phase()` maps them to `Phase::FIVE`, so `Keyword::in_subset()` is false
and the parser emits the phase diagnostic. `parallel` / `thread` / `sync` are
gated the same way and must stay gated.

Phase 4d already lowers an escaping closure to a two-word `{fn ptr, capture-block
ptr}` value: `InstKind::MakeCallable { target, captures }` allocates a
GC-tracked capture block (`zirk_rt_alloc_callable`) and `InstKind::CallCallable`
invokes it. `.clone()` deep-copies the block. That is exactly the machinery a
`task` body needs.

The normative behavior is already in the main specs (`zirk-grammar` "Task
creation and scope syntax" / "Await and timeout syntax", `zirk-type-system`
"Async core types are known" / "`task` produces a typed handle..." /
"A task result is consumed exactly once", `zirk-ir-lowering` "Lowering of task
creation and awaiting", `zirk-native-codegen` "Codegen for cooperative task
suspension"). This change implements the **bare** forms only.

## Goals / Non-Goals

**Goals**

- `task expr`, `task { block }`, `await expr` parse, type-check, lower, and run.
- `Task<T>` is a known type; `await` on `Task<T>` produces exactly `T`.
- A second `await` of the same statically tracked handle is a compile-time
  error; an ignored `Task<T>` is a must-use diagnostic dischargeable with
  `_ = handle`.
- A `.zrk` `main` can spawn a task and the executor keeps the process alive
  until it finishes (already true via `fase-5-executor-core`).
- CLI fixtures proving each of the above.

**Non-Goals**

- `task scope`, sibling-failure propagation, cancellation, `cancellation
  shield`, `await ... timeout`, `select`, channels, aggregation,
  `TaskSettlement<T>` — each its own later change. The parser gives them a
  targeted "arrives in <slice>" diagnostic, not a syntax error.
- Transfer / Share analysis. A task body captures by the ordinary Phase 4d
  closure rules; fixtures stay within value / projection / whole-reference
  captures. The data-race analysis is a separate change.
- `parallel` / `thread` — stay gated.
- Changing the `zirk_rt_task_*` ABI. Generated code commits to the provisional
  shape; a real `Task<T>` representation is a coordinated follow-up.

## Decisions

### D1: `Task<T>` — a typed name over a one-word handle

In the checker, `Task<T>` is `Base::Task(Box<Type>)` (a nominal single-parameter
generic, resolved by `Type::from_name` + the generic-argument parser, not a
user-declared generic). It carries `T` purely so `await` knows the result type.

In the IR it is `IrType::Task` — a one-word opaque handle, ABI `i64` (the packed
`TaskId` `zirk_rt_task_spawn` returns). It carries **no** `T`: codegen never
needs it, because the `Await` instruction records the result `IrType` directly.
`IrType::Task` does not need allocation and is not a garbage-collection
reference — the task control block it names is owned by the executor, not the
collector, and stays alive until `await` (or `_ =` at scope end via the
executor's own reclamation).

_Alternative:_ a real boxed `Task<T>` GC object. Rejected — the handle is a
number, the TCB is runtime-private, and boxing would add a GC root that must be
kept consistent with executor-side liveness for no benefit.

### D2: `task` body is a captured closure — but a dedicated node, not a synthesized `LambdaExpr`

The intent is unchanged: a `task` body is a zero-argument closure over a
GC-tracked capture block, lowered through the Phase 4d `MakeCallable` path.

**But it cannot reuse the AST `LambdaExpr` node.** `parse_lambda` requires an
explicit `: ReturnType` (`(a): Int32 => ...`) — the grammar has no
inferred-return lambda, and `TypeRef` has no "infer" form — while a `task` body's
result type `T` is only known after checking. So:

- AST: `Expr::Task(TaskExpr { body: TaskBody, span })`,
  `TaskBody = Block(Block) | Expr(Box<Expr>)`. `Expr::Await(AwaitExpr { operand:
  Box<Expr>, span })`. (`TaskBody::Expr` covers `task f(a,b)` and any expression;
  `task { }` is `TaskBody::Block`.)
- Checker: a new `check_task` that mirrors `check_lambda`'s scope/capture
  handling (`scopes.push_function`, `capture_stack.push`, the outer-capture
  propagation loop, `fn_types` / `lambdas` registration) **minus** the
  explicit-return requirement, **plus** inferring `T` from the body — the block's
  result type or the expression's type. It produces `Base::Task(Box<T>)` and
  registers a capturing-closure `fn_type` + `LambdaInfo` so lowering has a
  `target` function and a `captures: Vec<Capture>` list, exactly like a lambda.
- Lowering: `lower_task` reuses `lower_lambda`'s body-function emission and
  `MakeCallable { target, captures }`, then wraps the resulting boxed callable in
  `TaskStart`.
- Codegen: the capture-block emission is the existing `MakeCallable` path
  unchanged; only the `TaskStart` (thunk + `zirk_rt_task_spawn`) and `Await`
  (`zirk_rt_task_await` + narrow) instructions are new.

Refactoring `check_lambda` / `lower_lambda` to share their core with
`check_task` / `lower_task` (rather than copy) is the first real task of group 3.

### D3: `TaskStart` lowering and the spawn thunk

`TaskStart` carries both the boxed callable operand and the lifted body target.
The explicit target avoids reconstructing an SSA producer in codegen and lets
the backend emit a deterministic per-site thunk.

New `InstKind::TaskStart { target, body }` where `body` is the two-word boxed
callable from D2 and `target` is the lifted body function. It lowers to:

1. a per-site **thunk** `extern "C" fn __zirk_task_thunk_N(capture: *mut c_void)
   -> usize`, emitted by codegen, that: loads the body function pointer from the
   boxed callable's descriptor (or is monomorphized against the known body
   function), calls it with `capture`, and **widens** the `T` result to `usize`
   — a reference becomes its pointer bits, a scalar is zero-extended, `Void`
   yields `0`.
2. `call zirk_rt_task_spawn(__zirk_task_thunk_N, capture_block_ptr)` → the `u64`
   handle, kept as the `IrType::Task` value.

The boxed callable is split at lowering: its capture-block pointer is `arg`, and
the thunk closes over its function pointer. Because the executor is
single-threaded and the thunk runs on the child task's own stack, the capture
block is reachable from that task's shadow-stack frame the moment the body
function pushes its frame.

### D4: `Await` lowering and result narrowing

New `InstKind::Await { handle: Operand, result: IrType }` — a suspension point.
It lowers to `call zirk_rt_task_await(handle)` returning `usize`, then **narrows**
to `result`: a reference `T` is the pointer bits reinterpreted, a scalar is
truncated to its width, `Void` discards. Codegen emits the call plus a resume
label exactly as `zirk-native-codegen` "Codegen for cooperative task
suspension" requires; the ordinary calling convention is unchanged.

`await` on anything that is not a `Task<T>` is a type error. `await` outside a
function body is impossible (there is always a function; `main` is task 0).

### D5: Single-consume — linear tracking of a `Task<T>` binding

The checker already tracks must-use `Result` values and post-`transfer(r)`
use-after-move. `Task<T>` reuses that: a `Task<T>` local is a **linear** binding.
`await x` consumes it. A second `await x`, or any other use of `x` after the
consuming `await`, is `SECOND_AWAIT` (a new diagnostic code) pointing at the
first `await`. An unconsumed `Task<T>` at scope end is the existing must-use
diagnostic; `_ = x` discharges it (the executor still awaits the task's
completion — `_ =` is not detach).

A `Task<T>` passed to a function, stored in a field, or returned escapes the
static tracking; the runtime `zirk_rt_task_await` still enforces single-consume
via `result_consumed` (`fatalError` on the second call) as the backstop.

### D6: Keyword ungating and the deferred-form diagnostics

- `Keyword::phase()` drops `Task | Await` from the `Phase::FIVE` arm (keeping
  `Parallel | Thread | Sync`). `Keyword::in_subset()` is then true for them.
- The parser's expression grammar gains `task` and `await` productions.
- `task` followed by `scope`, and `await <expr>` followed by `timeout`, are
  recognized and rejected with a targeted diagnostic naming the slice that
  delivers them (`fase-5-task-scope` / `fase-5-select-and-channels`), not a
  generic syntax error — this satisfies the `zirk-feature-phasing` "structural
  async forms stay deferred" scenario.

### D7: `main` spawning a task

No new work: `zirk_rt_run_main` runs `main` as task 0 and the executor loop only
finishes once every live task is terminal (`fase-5-executor-core`). A `.zrk`
`main` that writes `task background()` and returns is kept alive until
`background` completes. A fixture proves it.

## Risks / Trade-offs

- **[The provisional `zirk_rt_task_*` ABI gets baked into generated binaries.]**
  → Accepted and documented; a `Task<T>` representation change becomes a
  coordinated runtime + codegen update, and no shipped Zirk binary exists yet.
- **[Result widening/narrowing (`usize` <-> `T`) is lossy for a `T` wider than a
  pointer.]** → In this slice `T` is a reference, a `≤64-bit` scalar, or `Void`.
  A `record` / large value `T` returned from a task is rejected by the checker
  with "task results wider than a machine word arrive with `Task.settled`" until
  a boxed-result path lands. Fixtures stay within the supported `T`.
- **[A `Task<T>` that escapes static tracking can be awaited twice at runtime.]**
  → `zirk_rt_task_await`'s `result_consumed` `fatalError` is the backstop; the
  checker covers the common (local, awaited-in-place) case.
- **[Capture-block liveness across the suspend.]** → Handled by
  `fase-5-executor-core`'s per-task shadow-stack chains: the body function
  pushes its frame, the capture block is a root of the child task, and a
  collection triggered from any task walks every task's chain. Covered by a
  GC-under-task fixture.
- **[`task` desugaring to a closure changes error spans.]** → The synthesized
  closure carries the `task` keyword's span so diagnostics point at the user's
  code, not a synthetic node.

## Migration Plan

- No source migration — `task` / `await` do not compile today.
- Per-group milestones in `tasks.md`, each leaving `cargo test --workspace`
  green. Order: keyword + parser + AST → checker → IR + lowering → codegen →
  fixtures → docs.
- After merge: `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD` for
  the `ZIRK_FEATURE_STATUS.md` Phase 5 change.

## Open Questions

- **Does `task expr` accept a non-call expression (`task 1 + 2`)?** Leaning yes
  — `task { return expr; }` — it is harmless and consistent, and the checker
  rejects a `Void` non-block `task` body only if the expression is itself
  `Void`-typed with no effect. Confirm against the grammar scenario wording.
- **Where does the per-site thunk live — one per `task` site, or one generic
  thunk that dispatches through the boxed callable's descriptor?** Leaning: one
  generic `extern "C" fn(*mut c_void) -> usize` that treats `*mut c_void` as the
  boxed-callable value and goes through the same load-fn-ptr path `CallCallable`
  uses, so no per-site codegen. Decided during implementation of D3.
- **`SECOND_AWAIT` vs. reusing the `transfer` use-after-move code.** Leaning: a
  distinct code with an `await`-specific message and a "first consumed here"
  note, but the same underlying liveness pass.
