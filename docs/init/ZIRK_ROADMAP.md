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

## Phase 3b — Complete scalars, conversions and text

Phase 1 implemented one integer width, one Boolean and an opaque `String`.
Everything else the type system promises about scalars was never assigned a
phase. This one owns it:

- The remaining integer widths: `Int8`, `Int16`, `Int64`, `Int128` and the whole
  `UInt*` family, with fixed width and checked arithmetic.
- The binary floating family `Float16`, `Float32`, `Float64`, `Float128`, with
  `Float` aliasing `Float64`, explicit infinities and no valid `NaN` — an
  operation that would produce one is a controlled error.
- `Char` as exactly one Unicode grapheme, which may span several code points.
- Deep contextual conversion: `Float(3 / 4)` converts the operands before the
  division rather than converting its integer result.
- Bitwise and shift operators, levels 7 to 10 of the operator table.
- String interpolation, which needs the `to_string()` contract Phase 3 defines.

They arrive together because they depend on each other: contextual conversion
means nothing without `Float`, `Float` literals mean nothing without the family,
and interpolation needs Phase 3's contracts. Splitting them would mean
implementing each one twice.

**Output:** the complete scalar surface of the spec — every numeric width, real
`Char`, and the conversion rules that connect them.

---

## Phase 4 — Errors and memory

- `Result<T, E>` with exhaustive `match`.
- `try`/`catch`/`finally`, `fatalError`.
- Full implementation of the memory strategy decided in Phase 0.
- `unsafe {}`, `Pointer<T>`, memory-safety guarantees enforced by the compiler.
- `Resource<E>` and `match with`.
- `inmut::strict`: deep immutability with alias analysis. It lands here and not
  with the other two mutability forms because it is not a local read-only flag —
  it must prove that no accessible mutable alias of the reachable state exists,
  which is the same analysis the memory strategy needs.

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

Two families in this phase are **not** ordinary library objects, and the
distinction changes who implements them:

- The **temporal family** — `Date`, `Time`, `DateTime`, `Instant`,
  `ZonedDateTime`, `TimeZone`, `Duration` and `Period` — are compiler-known
  native immutable values with their own literals, operators and type rules.
  The lexer and the checker know them before `std.time` exists; what this phase
  adds is their implementation, their IANA zone data and their API, not their
  existence as types.
- The **collection family** — `Array<T>`, `List<T>`, `Map<K,V>`, `Set<T>` — are
  native reference types under the same rule.

`String` grapheme indexing also lands here, which is the moment ADR-005's
boundary was designed to protect. With indexing come the forms that depend on
it: slicing `[start:end:step]`, and `Range<T>`'s `.step(distance)` and
`.reverse()`.

Regex also lands here — the `re'pattern'` literal and regex patterns in `match`.
The literal is core syntax and the lexer knows it earlier, but it means nothing
without an engine to run it, and the engine is a library.

**Output:** real non-trivial applications (a CLI, a simple backend) writable in
Zirk using only the stdlib.

---

## Phase 7b — Functional style and generators

- Generators: `fn gen` with `yield`, preserving locals between yield points and
  implementing both `Iterator<T>` and `Iterable<T>`.
- The pipe operator `|>`.
- `map`, `filter` and `reduce` over collections, without mutating the source.

It lands after the collections it operates on and after the iteration contracts
Phase 3 defines. It is numbered `7b` rather than taking a number of its own so
packaging, developer experience and metaprogramming keep the numbers they have
had since the roadmap was written.

**Output:** the functional style of `LANGUAGE_SPEC.md` section 8 working over
the real collections.

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
