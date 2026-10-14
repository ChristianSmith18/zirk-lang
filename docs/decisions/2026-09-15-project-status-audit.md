# Zirk Project Status Audit

**Audit date:** 15 September 2026
**Scope:** implementation status of every roadmap phase, cross-checked against
`docs/init/ZIRK_ROADMAP.md`, `docs/init/ZIRK_FEATURE_STATUS.md`, the OpenSpec
spec/change tree, the compiler source, and a full `cargo test --workspace` run.
**Mode:** read-only audit. This report changes no code, spec, or roadmap state.
Per ADR-006 it is written in English.

## Executive summary

Zirk is a real, working compiler — not a scaffold. The full pipeline
(lexer → parser → sema → IR → LLVM → native binary + linked runtime) is green:
**1337 tests pass across 32 test binaries with 0 failures** (verified locally on
macOS aarch64, `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20`).

The project is **at the end of Phase 5**, the roadmap's highest-risk phase:

- Phases 0–4 (incl. 3b, 4a–4e): **complete for their scoped delivery**.
- Phase 5 (concurrency): **4 of 5 sub-changes delivered**. Only
  `concurrency-completion` remains, and it is a fully specified, untouched
  OpenSpec change (49 tasks, none checked).
- Phase 6 (project system): **not started**, except `zirk check`, which exists
  already and works without LLVM.
- Phase 7 (stdlib): **partially delivered ahead of schedule** — the native
  collection and civil temporal families landed early because later phases
  needed them. The zone-aware temporal family and the wider `std.*` modules
  remain.
- Phases 7b–12: **not started**.

Two documentation drifts were found (detailed below): the `README.md` status
block is one phase behind reality, and `Channel` remains in the
`pending_type` table in violation of the `zirk-feature-phasing` spec.

## Verified metrics

| Metric | Value | Source |
|---|---|---|
| Workspace tests | 1337 passed / 0 failed (32 binaries) | `cargo test --workspace`, this audit |
| Rust source | ~76,200 lines across 9 crates | `wc -l crates/*/src` |
| Rust test code | ~16,900 lines | `wc -l crates/*/tests` |
| CLI corpus fixtures | ~209 valid + ~76 invalid `.zrk` programs | `crates/zirk-cli/tests/corpus/` |
| Main OpenSpec specs | 57 capability specs | `openspec/specs/` |
| Archived changes | ~80 | `openspec/changes/archive/` |
| Active changes | 1 (`concurrency-completion`, 0/49 tasks) | `openspec/changes/` |
| Handbook | 378 Markdown files, all 14 sections populated | `docs/handbook/` |
| ADRs | 19 accepted | `docs/decisions/` |
| VS Code extension | v0.4.2 | `editors/vscode/package.json` |
| CI matrix | fmt, clippy `-D warnings`, build, test on Linux x86_64/aarch64, macOS aarch64, Windows x86_64 | `.github/workflows/ci.yml` |
| Working tree | clean | `git status` |

## Phase-by-phase status

### Phase 0 — Decisions — ✅ complete

All four items closed and verified in CI. 19 ADRs now exist (the roadmap asked
for the Phase 0 subset; later decisions kept being recorded the same way —
ADR-017 suspension model, ADR-018 concurrency surface, ADR-019
parallel/multithreaded GC).

### Phase 1 — Minimal pipeline — ✅ complete

`zirk run hello.zrk` compiles and runs a real native binary end to end.

### Phase 2 — Core language surface — ✅ complete (scoped)

Complete control flow, `match` with exhaustiveness, nullability
(`T?`/`?.`/`??`), `share`/`import`/`use` within a crate, local closures,
optional/named/variadic parameters — plus later additions: compound
assignment, multiple declarations, spread/rest operators. Escaping closures
were explicitly deferred to 4d (and delivered there).

### Phase 3 — Objects and type system — ✅ complete (scoped)

Classes, visibility, single inheritance, interfaces/traits, generics with
`from` constraints, records, algebraic and generic enums, tuples, casts
(`as`, `as?`), abstract-class dynamic dispatch, derived structural equality
on records, `record` contract dispatch, static members, field defaults,
`type` aliases. `value class` was removed from the language.

**Known residuals (documented, owned by future work):**

- Generic contract lowering is `partial`: trait default bodies under
  substitution and user-defined generic function bodies remain pending.
- Self-referential enums (e.g. `IntList.Cons(tail: IntList)`) are cleanly
  rejected; real support needs heap indirection ("boxing") — a tracked future
  change.

### Phase 3b — Scalars and text — ✅ complete (scoped)

All ten integer widths, exact base-ten `Float` as the fractional default,
the IEEE `Float16`–`Float128` family, grapheme `Char`, bitwise/shift
operators, final-result conversion, interpolation, `**`/`**=`, the full
`String`/`Char` method surface (indexing, slicing, mutation, search,
normalization, views).

**Known residuals:** `Float128` excluded from the Windows corpus (MSVC
soft-float linker crash); `Float128` `to_string()`/`format()` truncates
through `f64` (lossy for non-representable values).

### Phase 4 — Failure, callables, memory — ✅ complete (scoped)

- **4a:** `Result<T,E>` with mandatory consumption, `_ =` discard.
- **4b:** `throws`/`try`/`catch`/`finally`, typed catch dispatch, all six
  concrete `RuntimeError` subclasses catchable, `suppressed()`,
  `stack_trace()`, deep immutability of caught bindings.
- **4c:** single and grouped `match ... with`, `transfer(r)`,
  `ResourceFailure` merging, cancellation-aware cleanup.
- **4d:** `Fn(...) => R` annotations everywhere, boxed callables
  (`MakeCallable`/`CallCallable`) end to end — storable, returnable,
  GC-tracked captures, `.clone()`, `is` identity.
- **4e:** non-moving mark-sweep GC (shadow-stack roots, reclaims cycles),
  `Weak<T>`, deep `clone()` with cycle-preserving memoization, `unsafe {}` /
  `commit {}` with transactional journal + rollback (including early
  `return`/`break`/`continue` exits), `Pointer<T>`, `extern "C" fn`,
  `NativeSlice<T>`/`NativeSliceMut<T>` (which also delivered the previously
  missing `expr[index]` grammar), `Dependent<T>` escape analysis, `Pin<T>`.

**⚠ Roadmap drift:** the last 4e checkbox in `ZIRK_ROADMAP.md` (line ~371) is
still unchecked and marked "Partial" for `Dependent<T>`/`Pin<T>`, but the
feature-status catalog and the archived changes record both as delivered.
The checkbox should be reconciled.

**Explicitly out of scope (tracked):** volatile pointer access, native
unions, general slice-provenance beyond the direct
`Pointer.from(place).as_slice(n)` shape, `record`/`enum` `Clone` derivation.

### Phase 5 — Concurrency — 🔄 4 of 5 sub-changes delivered

| Step | Change | Status |
|---|---|---|
| 0. Executor infrastructure | `fase-5-executor-core` | ✅ runtime delivered: cooperative executor, stackful coroutines (ADR-017), monotonic timer, per-task GC roots, `zirk_rt_run_main` |
| 1. `concurrent`/`spawn`/`Job<T>`/`Timer` | `concurrent-blocks-and-timers` | ✅ delivered: Rule-B dataflow branch ordering, cancellation unwinds natively, `main` in an implicit scope |
| 2. `parallel` CPU regions | `parallel-cpu-regions` | ✅ delivered: separate worker pool, `parallel {}`, work-split `for`, ordered `map`/`filter`/`for_each`/`Parallel.each`/`sum`/associative `reduce`, stop-the-world safepoint collector (ADR-019) |
| 3. `Channel<T>` | `typed-channels` | ✅ delivered: bounded/rendezvous/growable, cooperative send/receive, `try_*`, idempotent close, GC tracing |
| 4. Completion | `concurrency-completion` | ⬜ **0/49 tasks** — fully specified, not started |

The old `task`/`await`/`select`/shield surface was removed
(`remove-task-await-model`); legacy syntax reports `E0318` naming its
replacement.

**`concurrency-completion` remaining scope** (from its `tasks.md`):

1. **Combinators** — `Concurrent.of(...).first()/.settled()/.within(d)`,
   `each`/`each_settled`, `Outcome<T>` (Fulfilled/Rejected/Cancelled).
2. **Background work** — `App.background` scope, `Concurrent.detach`,
   bounded drain at shutdown, `App.on_background_error`,
   `application.spawn_service`.
3. **Raw threads** — `Thread.run` with cooperative join, safepoint polls,
   `THREAD_RUN_SUSPEND` restrictions.
4. **Synchronization** — `Atomic<T>` (SC default; weak ordering only in
   `unsafe`), `Mutex<T>`, `RwLock<T>`, `Semaphore`, `Barrier`, `Once<T>`,
   `Concurrent.protect` (shield replacement), guard-escape and
   guard-across-safepoint checks.
5. **Data-race freedom** — full `Transfer`/`Share` enforcement at branch
   captures, `concurrent` results, channel ops, `parallel` and `Thread.run`
   boundaries; safe-code data races become compile errors.
6. Docs, fixtures, ADR-020, and the Phase-5-complete closeout
   (spec deltas, catalog, README, website sync).

### Phase 6 — Project system and CLI — ⬜ not started (1 early delivery)

- `zirk build` / `zirk run` operate on a **single file**; artifacts land in
  `build/`.
- `zirk check` exists and is frontend-only (works without LLVM) — an early
  Phase 6 delivery.
- `.zkinit` manifest, `project`/`build_targets`/`globals`/`requires`/
  `permissions`, `new`/`init`/`test`/`lint`/`doc` subcommands (gated with
  Phase-6 diagnostics), multi-file namespacing, and cross-compilation all
  remain.

### Phase 7 — Standard library — 🔄 partially delivered ahead of schedule

Delivered early (native types needed by other phases):

- `Array<T>`, `List<T>` — full surface incl. eager `map`/`filter`/`for_each`/
  `reduce`/`sum`/`count`/`collect`, membership, reverse/sort, nullable
  first/last/pop (`collection-sequence-pipeline`).
- `Map<K,V>`, `Set<T>` — constructors and core ops; **runtime is a minimal
  i64 hash map that does not trace GC references yet** (documented gap).
- `Range<T>` — properties, steps, descending inference, slicing, expansion.
- `Duration` — literals `ns`–`w`, arithmetic, sign/abs queries.
- Civil temporal — `Date`/`Time`/`DateTime` with validated constructors,
  calendrical/sub-second properties, queries, `with_*`, `start_of`/`end_of`,
  strict ISO `parse`, `format`, arithmetic, `today`/`now_*` statics.
- `Regex` — `re'...'` literals, `matches`/`find`/`replace`/`split`/
  `find_all`, `Regex.parse`, `match`-pattern integration, capture groups.
- Universal `to_string`/`print`/`println` and triple-quoted strings
  (`universal-to-string-and-multiline-strings`).

**Remaining:** `Instant`/`ZonedDateTime`/`TimeZone`/`Period` (+IANA data,
`TimeShift`, locales) — still in `pending_type` at Phase 7; the wider
`std.collections` API; and every other `std.*` module (`fs`, `path`,
`process`, `thread`/`sync`, `json`, `net`/`http`, `crypto`, `testing`,
`reflect`, `system`) — documented in the handbook but not implemented.

### Phase 7b — Functional style — ⬜ not started

`fn gen`/`yield` and `|>` lex but are gated with Phase-7b diagnostics. Eager
collection `map`/`filter`/`reduce` exist (delivered with the sequence
pipeline), but the spec'd functional-style surface — lazy iterators,
generators implementing `Iterator`/`Iterable`, the pipe operator — is not
started.

### Phases 8–12 — ⬜ not started

| Phase | Content | State |
|---|---|---|
| 8 | `.zpkg`, `zirk.lock`, `zirk add/remove/install/update`, `package`, `publish`, reproducible builds | not started |
| 9 | Canonical formatter (a partial idempotent formatter exists in `zirk-cli`), linter, LSP, debugger | not started |
| 10 | Decorators (`fn dec` lexed, gated), Syntax API, `std.reflect` | not started |
| 11 | Fuzzing, differential testing, benchmark suite, full target matrix | not started |
| 12 | Self-hosting | long horizon |

## Findings: documentation/spec drift

1. **`README.md` is one phase behind.** It reads "Status: Phase 4e delivered;
   everyday data types in progress", names `array-list-tuple-duration-regex`
   as *the* active change, and claims "995 tests pass". Reality: Phase 5
   steps 0–3 are delivered, the active change is `concurrency-completion`,
   and 1337 tests pass. Task 9.5 of `concurrency-completion` already owns
   the README fix — but the gap exists *now*.
2. **`Channel` still in the `pending_type` table.** The
   `zirk-feature-phasing` spec states `Channel<T>` "SHALL NOT appear in the
   table once Phase 5 steps 1 to 3 are delivered", yet
   `crates/zirk-sema/src/types.rs:1107` keeps it (with a test asserting the
   stale behavior at line ~1229). It is dead code — the checker intercepts
   `Channel` in `resolve_type_atom` first — but it contradicts the spec it
   is meant to encode.
3. **Stale 4e roadmap checkbox** (see Phase 4 above): `Dependent<T>`/`Pin<T>`
   are delivered per the catalog; the roadmap bullet disagrees.
4. **Stale "Last updated" header** in `ZIRK_FEATURE_STATUS.md`: says
   "spread-rest-operators, 2026-09-13" while rows already document
   `typed-channels` and `universal-to-string` (2026-09-14).

## Known implementation limitations (documented, not regressions)

- `Float128` on Windows (MSVC soft-float linker crash); `Float128`
  formatting truncates through `f64`.
- `Map`/`Set` runtime does not trace GC references (i64-only payload).
- Generic contract lowering `partial`: trait defaults under substitution,
  user-defined generic function bodies.
- Self-referential enums rejected (need boxing — new `IrType`, alloc kind,
  GC integration).
- Escaping closures delivered; the residual single-capture/position limits
  are recorded in the catalog.
- `parallel` `reduce_ordered` deliberately sequential; `select` on channels
  is a follow-up.
- Volatile access, native unions, general slice provenance: out of scope.

## Tooling and ecosystem state

- **CLI:** `build`, `run`, `check` (frontend-only, no LLVM needed), plus
  `zirk-check` binary and `--list-targets`.
- **Editor:** VS Code extension v0.4.2 with syntax highlighting, snippets,
  `.zkinit` file-icon recognition; install copies to
  `~/.devin/extensions/christiansmith.zirk-lang-0.4.2/`.
- **Docs:** handbook complete across 14 sections (378 files) with an
  editorial coverage map; website sync via
  `scripts/sync-website-content.sh` (requires clean `../zirk-lang-site`).
- **Process:** OpenSpec workflow is real and enforced — 57 normative specs,
  ~80 archived changes, ADRs for cross-cutting decisions, a manually
  maintained feature-status catalog (whose own "generate from code"
  recommendation is still unimplemented).

## Recommended next steps (in dependency order)

1. **Fix the small drifts now** — README status block + test count, the
   `pending_type` `Channel` entry + its test, the 4e roadmap checkbox, the
   catalog's "Last updated" line. All are one-commit fixes and keep the
   status documents trustworthy.
2. **Execute `concurrency-completion`** — the only thing between the
   project and "Phase 5 complete". It is fully designed; work through its
   49 tasks in the written order (ADR/spec deltas → runtime → sema → IR/
   codegen → fixtures → docs → closeout).
3. **Then Phase 6** (`.zkinit` + multi-file + remaining CLI subcommands) —
   it unblocks real projects and is a prerequisite for meaningful Phase 7
   stdlib work beyond the native types.
4. **In parallel or after:** the zone-aware temporal family and the
   `Map`/`Set` GC-tracing gap are self-contained Phase 7 items that do not
   depend on Phase 6.

## Bottom line

The language's hard parts — the type system, memory model, error model, and
the structured-concurrency core — are done and verified. What remains for
Phase 5 is additive surface (combinators, threads, synchronization) on an
already-working executor and collector, plus the full data-race analysis.
The project is roughly at **60–65% of the roadmap's construction phases by
count** (phases 0–4 complete, 5 nearly so, 7 partially front-run), with the
genuinely risky engineering already behind it; phases 6+ are large in volume
but lower in conceptual risk — exactly as the roadmap predicted.
