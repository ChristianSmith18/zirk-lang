# Next steps after `fase-4a`/`fase-4b`/`fase-4c`

- **Status:** planning document (not an ADR, fixes no decision)
- **Date:** August 20, 2026
- **Context:** pure research and planning — does not touch `crates/*/src/*.rs`

## 0. Starting point

`fase-4a-errores`, `fase-4b-excepciones`, and `fase-4c-recursos` are on `develop`,
with all three `tasks.md` fully checked (`[x]` on every item, including
their closeout sections — `cargo test --workspace`, `clippy`, `fmt`), and
`openspec validate <change> --strict` passes clean for all three. `openspec
list` reports them as `✓ Complete`. No implementation work remains within
what these three changes set out to do.

That does not mean roadmap Phase 4 (`docs/init/ZIRK_ROADMAP.md`,
"Errors and memory") is finished — they deliberately covered a subset of
it. What follows is an inventory of what was deliberately left out of
each of the three `proposal.md` files.

## 1. Inventory of what's pending within Phase 4

### 1.1 From `fase-4a-errores` (Result<T,E>)

- **Generic `Result` combinators** (`map`, `map_error`, `and_then`,
  `or_else`, `or_throw`): need a method's own type parameter
  (`map<U>(...)`), which `MethodInfo` doesn't support today — this is a
  new piece of type inference, not a minor extension. `or_throw` also
  needs exceptions (already existing since `fase-4b`, so that specific
  dependency has already been satisfied).
- **`get_or_else(factory: Fn() => T): T`**: blocked by decision D9
  (function types `Fn(...) => R` with no annotatable syntax in any
  position of the language). Still blocked — D9 has not been touched in
  any of the three later phases.
- **Automatic propagation (`?`)**: not debt, it's an explicit language
  decision ("Zirk 1.x has no `?` propagation operator"). Does not apply
  as a real pending item.

### 1.2 From `fase-4b-excepciones`

- **Structured stack traces** (`stack_trace()` returns a real but empty
  `StackTrace` today) and **`suppressed`** populated from a cleanup
  failure in `finally`. `suppressed` also depends on `List<T>` (Phase 7).
- **Converting native runtime failures** (division by zero, overflow,
  invalid cast, invalid shift, invalid repeat) **into a catchable
  `RuntimeError`**: investigated and explicitly deferred —
  `design.md` documents D5–D7 with the concrete reason (the check lives
  in `zirk-codegen-llvm/src/emit.rs`, a layer below where
  `zirk-ir/src/lower.rs` builds the `try`/`catch` mechanism; moving it
  requires inserting the checks at the `zirk-ir` level instead of codegen).
  D7 already splits this into a tractable portion (the four
  zero/negative/range checks) and one that needs its own spike (overflow,
  because of the multi-result-instruction question). Still blocked, with
  an attack plan already written.
- **Variant patterns in `catch`** (`catch NetworkError.Timeout(duration)`):
  needs a user exception to declare internal variants, a mechanism that
  doesn't exist.
- **`Fn(...) => T throws X`**: blocked by D9, same as above.
- **Real native stack unwinding** (landing pads/personality function):
  deliberately not built — the current mechanism (`thread_local`
  pending + `try_stack`) is correct for every program the checker
  accepts, but it's not what a production compiler would use. There's no
  indication this is urgent; it's a long-term architecture note, not a
  dated task.

### 1.3 From `fase-4c-recursos`

- **Grouped acquisition** (`match a with x, b with y { ... }`) with
  right-to-left closing when a later acquisition fails.
- **`ResourceFailure<BodyError,CloseError>`**: today `close()` is called
  and its `Result` is discarded; it's not combined with the body's result.
- **`suppressed` populated from a close failure during exception
  propagation**: depends on `List<T>` (Phase 7) and on `ResourceFailure`.
- **`TransferableResource`/`transfer()`** and escape/use-after-transfer
  analysis: a resource can today escape its scope without the compiler
  detecting it — it remains the programmer's responsibility, unverified.
  This is the biggest safety gap that `fase-4c` leaves open.
- **Dependent resources that don't outlive their parent**, **`take` on a
  non-cloneable resource inside a container**: need collections (Phase 7).
- **Cancellation** (`STRUCTURED_CONCURRENCY_SEMANTICS.md`): depends on
  Phase 5.

### 1.4 The rest of roadmap Phase 4 (not covered by any of the three changes)

- **The full memory strategy** (`unsafe {}`, `Pointer<T>`, safe/weak/
  dependent references, deep clone graph semantics, automatic bounded
  native pinning): still tracked in `ADR-003-memoria.md`, status "accepted
  / open (implementation)". See section 2.
- **Transactional write journals and rollback** for managed/validated
  ranges, with irreversible `commit`: has not been touched in any recent
  session. Depends on `unsafe`/`Pointer<T>` existing first.
- **`inmut::strict`**: deep immutability with alias analysis. The roadmap
  is explicit that it needs the same analysis as the memory strategy, so
  it doesn't make sense before that strategy is closed.

Confirmed with `grep` in this session: `unsafe` produces `E0302` ("not
implemented yet, arrives in Phase 4"), `Pointer<T>` produces `E0402` for
the same reason, and the `extern` keyword doesn't exist in the lexer. This
has not changed since the investigation in `ADR-003-investigacion-fase-4.md`.

## 2. Can the memory strategy (ADR-003) be closed already?

My reading, after reviewing the "What questions remain open" section of
the full research document:

**Not yet, but not for lack of evidence overall — for a concrete and
nameable gap.** The research document already gathered real execution
evidence on three of the four decision criteria that ADR-003 itself sets:

- **Criterion 3 (interaction with `Resource<E>`/the C ABI boundary):** half
  of `Resource<E>` is answered with strong evidence (probe 4 — a
  resource's graph remains correct during exception unwinding). Half of
  the C ABI boundary is confirmed as *nonexistent*, not "unmeasured":
  `unsafe`/`Pointer<T>`/`extern` have no language surface yet at all.
  This part of criterion 3 is genuinely blocking — there's nothing to
  measure against until `unsafe {}` and `Pointer<T>` have at least a
  minimal parser/checker.
- **Criterion 4 (pauses against budget):** does not apply — no collector
  exists yet. Blocking by definition: you can't measure the pause of
  something that doesn't exist. But I note this is circular with the very
  decision ADR-003 must make — it's not an external precondition, it's the
  implementation itself. It shouldn't be treated as a blocker prior to
  deciding, but as what comes *after* deciding.
- **Criterion 1 (closures that capture and escape):** blocked by D9, with
  a partial approach (probes 8–10, field capture instead of closures) that
  the document itself is careful not to oversell. This is, in my judgment,
  the most important of the three real gaps, because it is exactly the
  pattern that most pressures the choice between "escape-analysis tracing"
  and any alternative — it's where escape analysis has something genuine
  to decide (promote to stack or not?), and today it cannot be exercised.
- **Criterion 2 (barrier cost in `parallel for`):** `parallel`/`thread`
  doesn't exist in the lexer. Blocking in the same sense as criterion 4 —
  it's post-Phase-5, not a reasonable precondition for deciding the memory
  strategy now (the roadmap itself puts memory in Phase 4 and concurrency
  in Phase 5, in that order, precisely because memory must be closed
  first).

**My own reading, not just the document's:** of the four criteria, two
(2 and 4) are not real blockers for *deciding* — they are criteria for
*post-implementation validation*, not for *prior choice*. Demanding
evidence for them today inverts the order: you cannot measure the pause of
a GC that hasn't been written yet, nor the cost of a barrier in a
`parallel for` that arrives in the next phase. Treating those two as
blockers would, in practice, mean never deciding.

The two that do matter — criterion 1 (escaping closures) and the C-ABI
half of criterion 3 — share the same root cause: both depend on language
surface that doesn't exist yet (D9 for the first, `unsafe`/`Pointer<T>`
for the second), not on missing research work. No additional volume of
probes will unblock them; implementation is needed first.

**Conclusion:** ADR-003 remains open, and rightly so — not out of generic
caution but because two of its four criteria have a named, concrete gap
(D9 and minimal `unsafe`/`Pointer<T>`) that no additional research
document can close without the compiler advancing first. The "informed
lean" that `ADR-003-investigacion-fase-4.md` already lays out (tracing
with escape analysis + inline value types, pure RC discarded) seems
reasonable to me, and I wouldn't contradict it with anything reviewed
here — but I agree with the document that treating it as already
validated against the case that would most put it to the test would be
premature.

## 3. Does it make sense to prepare ground for Phase 5 (concurrency) now?

No, and I think the answer is clear, not ambiguous. The reason isn't just
that the roadmap orders it later — it's a real content dependency:

- The roadmap itself describes Phase 5 step 3 as "compiler-derived
  transfer/share and capture analysis sufficient to enforce the safe-code
  data-race guarantee" — that is, literally, an analysis that needs to
  know what a shared reference is, what exclusive ownership is, and how
  it moves between tasks. None of those notions exist yet in the compiler
  because they are exactly what Phase 4's memory strategy has to define
  first.
- `Mutex<T>`/`RwLock<T>`/`Atomic<T>` (Phase 5 step 4) need a shared-memory
  model with visibility guarantees — it can't be specified without
  knowing whether the runtime uses tracing, RC, or regions.
- Unlike ADR-003, where real partial evidence already exists and is worth
  documenting (closures, objects, `Resource<E>`), Phase 5 today has no
  buildable language at all that exercises it — no `task`, no
  `Channel<T>`, no `parallel`. Writing a "Phase 5 preconditions" document
  today of the same kind as was done for ADR-003 would, at best, produce a
  reread of the roadmap with no new evidence to contribute — and at worst,
  it would fix expectations about a language surface that the memory
  decision itself can still change.

My honest recommendation: this is not the time to touch Phase 5 at all,
not even in read/annotation mode. Wait until the memory strategy is at
least decided (not necessarily fully implemented) before investing time
there — any notes taken now about Phase 5 risk becoming obsolete as soon
as ADR-003 closes, because the shape memory takes directly changes which
transfer analysis is even possible.

## 4. Pending closeout work on what's already implemented

- **`tasks.md` of the three changes**: no unchecked items. All three are
  100% `[x]`, including their closeout sections (tests/clippy/fmt green,
  `design.md` with its `## Decisions` section completed).
- **`openspec validate --strict`**: passes clean for `fase-4a-errores`,
  `fase-4b-excepciones`, and `fase-4c-recursos`.
- **`openspec list`**: reports all three as `✓ Complete`.
- **Candidates for `openspec archive <change> --yes`** (not executed in
  this session, per explicit instruction — only flagged here): all three
  qualify. Each has a complete `tasks.md`, a closed `design.md`, and
  (where applicable) its spec delta already written —
  `fase-4c-recursos/tasks.md` item 7.3 explicitly documents the delta to
  `openspec/specs/zirk-resources/spec.md`. I suggest archiving all three,
  in the order they were completed (`fase-4a-errores` →
  `fase-4b-excepciones` → `fase-4c-recursos`), before opening any new
  change — it keeps `openspec list` clean and avoids a new change being
  accidentally confused with one of these three if it shares touched
  files.
- **Minor detail found during the research, not a closeout task for these
  three changes**: `crates/zirk-sema/src/types.rs` has an outdated
  comment next to `PHASE_4` (`pending_type`) that still lists `Resource`
  as pending, when `Resource<E>` has already been implemented since
  `fase-4c` and is resolved through another path (the name in that list
  is dead code in practice). It's a one-line comment fix, with no
  functional effect — worth having whoever next touches `types.rs` fix it
  in passing; it doesn't warrant its own openspec change.

## 5. Prioritization — what to tackle first and why

Following the pattern that has already proven to work in this run of
sessions (small, verifiable changes, one at a time, instead of whole
phases in one jump), my recommended order:

1. **Archive the three completed changes** (`fase-4a-errores`,
   `fase-4b-excepciones`, `fase-4c-recursos`) with `openspec archive --yes`.
   Minimal cost, zero risk, and cleans up the state before any new change
   is opened — it's process hygiene, not design work, but it trivially
   blocks traceability of what comes next if left undone.

2. **D5–D7 from `fase-4b-excepciones`: convert the four tractable native
   checks (division by zero, invalid shift, invalid repeat, `NaN`) into
   real catchable exceptions**, leaving overflow out (as D7 itself
   recommends) until the multi-result-instruction question gets its own
   spike. I'd prioritize this over any other item on the section 1 list
   because: (a) it already has design written and a concrete reason why
   it was deferred — it's not exploration from scratch; (b) it's coherent
   with the "catch or declare" pattern the language user already expects
   from any other exception, so closing this asymmetry has real language
   value, not just coverage value; and (c) it's a bounded change (four
   checks, not five, with the fifth explicitly postponed) of the size this
   run of sessions already handles well.

3. **Fix the compiler bugs that block continuing to measure ADR-003**
   (the five from the original document plus the `?.` on a `Void` method
   one documented in the extension) — with the caveat that this is
   already being worked on in parallel by another agent in this same
   session, per this task's context, so I wouldn't recommend it as my own
   "next step" but rather as something already in progress. Once that
   work lands, it would be worth an additional probe aimed specifically
   at larger object graphs and recursive traversals, to finish exercising
   what the original probe 3 left incomplete due to the bugs it found.

I don't include D9 (function types) or minimal `unsafe`/`Pointer<T>` in
this "first" list despite them being the two real gaps blocking ADR-003's
closure (section 2) — they are larger language-design decisions, with
their own surface (syntax, checking, inference), and it doesn't make sense
to size them until ADR-003 itself is addressed as its own decision
process. Advancing a partial `Pointer<T>` implementation just to "have
something to measure" without first deciding what memory guarantees it
must satisfy would mean building on a decision not yet made — exactly the
risk ADR-003 (phase 0) set out to avoid from the start.
