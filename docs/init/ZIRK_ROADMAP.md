# Zirk — Construction roadmap

This roadmap translates the five normative specs — which describe a mature Zirk
— into a real order of construction. Each phase has a verifiable goal: you do
not move to the next one until the current goal actually runs, not merely "is
almost there".

No real language was ever built by implementing its full spec in one pass. This
document is the discipline that keeps this one from trying.

---

## Phase 0 — Decisions before writing code

This is not a coding phase but a decision phase whose consequences cascade into
everything else. Write them as short ADRs, one per decision, in
`docs/decisions/`.

- [x] **Memory strategy.** The spec promises automatic memory without exposing
  ownership (`RUNTIME_SPEC.md` section 9) and the absence of data races on
  shared globals (`LANGUAGE_SPEC.md` section 2). Define explicitly: generational
  GC, reference counting with cycle detection, regions, or a hybrid? This is the
  highest-leverage decision of the project — it determines how closures,
  `parallel for` and `Resource<E>` behave later on.
- [x] **Workspace layout.** Proposed crates: `zirk-lexer`, `zirk-parser`,
  `zirk-ast`, `zirk-sema` (name resolution + type checker), `zirk-ir`,
  `zirk-codegen-llvm`, `zirk-diagnostics`, `zirk-cli`.
- [x] **LLVM sanity check.** Before writing a line of Zirk, confirm that
  `inkwell` generates, links and runs a trivial native binary from pure Rust.
  This step depends on nothing about the language — it validates that the
  toolchain works on your machine and in CI before building on top of it.
- [x] **Diagnostic format.** Implement the format of `COMPILER_SPEC.md`
  section 8 as its own crate from day one (`zirk-diagnostics`), even if only the
  lexer uses it at first. Migrating the format after five other layers already
  use it is far more expensive than starting right.

**Phase output:** ADRs written, workspace created, `cargo build` compiles a
binary that invokes LLVM and produces a "hello world" executable written
directly in Rust (with no Zirk parser yet).

**Status: complete.** Verified in CI on Linux (x86_64, aarch64), macOS aarch64
and Windows x86_64. The decisions live in `docs/decisions/` as ADR-001 to
ADR-006.

---

## Phase 1 — Zirk 0.1: minimal end-to-end pipeline

**Goal:** `zirk run` over a `.zrk` containing
`fn main(): Void { stdout.println("..."); }` really compiles through LLVM and
runs as a native binary.

Language subset: `fn main`, `Void`, `String`, `Int32`, `Boolean`, literals,
basic arithmetic, `if`/`else`, `mut`/`inmut` variables, and a minimal hardcoded
`println` (not the full stdlib yet).

Explicitly out: generics, classes, `Result`, concurrency, decorators,
multi-file modules, `init.zrk`.

**Phase output:** a real `.zrk`, with real syntax from the spec, compiling to a
real native binary. This is the milestone that validates that the whole
architecture (lexer → parser → types → IR → LLVM → binary) works end to end —
everything that follows extends this backbone rather than building a new one.

---

## Phase 2 — Core language surface

- Complete control flow: `for`, `for ... in`, `while`, `loop`, `break`,
  `continue`, `if` as an expression.
- Complete functions: optional, named and variadic parameters, default values,
  closures/lambdas.
- `match` with basic exhaustiveness (over simple enums).
- Nullability: `T?`, `?.`, `??`.
- Modules within a single crate: basic `share`/`import`, without `init.zrk` yet.

**Output:** programs with several functions, real control flow and closures —
still without classes or concurrency.

---

## Phase 3 — Objects and the type system

- `class`, `construct`, visibility (`public`/`private`/`protected`), single
  inheritance, interfaces, traits.
- Generics with `from` (constraints).
- Records, value classes, algebraic enums, unions.
- Casts (`as`, `<T>`, `unsafe` casts).

**Output:** the object-oriented subset of the spec working, including basic
generics.

---

## Phase 4 — Errors and memory

- `Result<T, E>` with exhaustive `match`.
- `try`/`catch`/`finally`, `fatalError`.
- Full implementation of the memory strategy decided in Phase 0.
- `unsafe {}`, `Pointer<T>`, memory-safety guarantees enforced by the compiler.
- `Resource<E>` and `match with`.

**Output:** complete error handling and the central promise of the spec — safe
code with no use-after-free, no uncontrolled null deref and no UB — verifiable
with tests.

---

## Phase 5 — Concurrency and parallelism (the hardest and least trodden part)

This is the phase of highest technical risk in the project — build it in
sub-steps, not in one go:

1. `task`/`await` on a custom single-threaded executor first (structured
   concurrency without real parallelism yet).
2. `Channel<T>`, `sync`, `Atomic<T>`.
3. `thread` (real OS threads).
4. `parallel`/`parallel for` on a multicore pool.
5. Static analysis of unsafe mutable captures in `parallel`/`thread` (the
   data-race-freedom guarantee of the spec, bounded to globals — this is not
   general data-race freedom).

**Output:** the five concurrency primitives of `RUNTIME_SPEC.md` working with
the minimum guarantees the spec promises.

---

## Phase 6 — Project system and CLI

- `init.zrk` as a declarative DSL (its own parser, not reusing the Zirk parser).
- `project`, `build_targets`, `globals`, `permissions`, `compile_permissions`.
- CLI: `new`, `init`, `run`, `build`, `check`, `test`.
- Real cross-compilation to the targets of the spec (`COMPILER_SPEC.md`
  section 6).

**Output:** real multi-file projects, with a manifest and cross-compilation.

---

## Phase 7 — Stdlib

Order suggested by real dependency, not by the order they appear in the spec:

`std.io` (already partially covered) → `std.collections` → `std.fs`/`std.path` →
`std.time` → `std.process` → `std.task`/`std.thread`/`std.sync` (wrapping
Phase 5) → `std.json` → `std.net`/`std.http` (the largest of them all) →
`std.crypto` → `std.testing` (`@test`/`@e2e`/`@bench`) → `std.reflect` →
`std.system`.

**Output:** real non-trivial applications (a CLI, a simple backend) writable in
Zirk using only the stdlib.

---

## Phase 8 — Packaging and distribution

- `.zpkg`, `zirk.lock`, `zirk add/remove/install/update`, `zirk package`,
  `zirk publish`.
- Reproducible builds (`COMPILER_SPEC.md` section 7).

---

## Phase 9 — Developer experience

- A canonical, idempotent formatter.
- A linter sharing parser/types with the compiler.
- An LSP with incremental snapshots.
- A debugger with symbols and mapping back to `.zrk`.

This phase is large in volume of work but low in conceptual risk — nothing here
is new territory, it is high-volume engineering.

---

## Phase 10 — Metaprogramming

- Decorators (`fn dec`), a versioned and immutable Syntax API.
- Advanced `std.reflect`.

It is left for the end on purpose: it depends on the rest of the compiler being
stable, because decorators touch nearly every layer (parser, types, IR).

---

## Phase 11 — Production hardening

- Fuzzing, differential debug/release testing, a public benchmark suite
  (`COMPILER_SPEC.md` section 1).
- Full coverage of the target matrix.

---

## Phase 12 — Self-hosting (long horizon, years)

Rewrite the compiler in Zirk once the language is mature and stable enough. It
is the same path Rust followed (rustc started in OCaml), and Go (its compiler
started in C), and Zig (started in C++). None of these projects self-hosted from
day one, and Zirk should not attempt it prematurely either.

---

## How to use this roadmap

- A phase counts as complete when its "Output" runs with real tests, not when
  the code "is almost there".
- If mid-phase there is a temptation to bring something forward from a later
  phase, note it as a pending item and continue with the current phase — it is
  not implemented halfway.
- This is a living document: if a phase turns out to be badly sized (too large,
  or with unforeseen dependencies), it is adjusted here rather than improvised
  in the code.
