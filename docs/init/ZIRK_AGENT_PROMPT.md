# Initial prompt — Zirk

This document is the startup prompt for any working session (a coding agent, or
yourself picking the project back up after a while). Paste it whole at the start
of a new session, or store it as a persistent agent instruction if your tool
allows it.

---

## Language of the project

**Everything is written in English**: the normative specifications, this
document, the roadmap, the source code, its comments, the diagnostics the
compiler emits and the test names.

The only Spanish that remains is the working documentation around the project —
the ADRs in `docs/decisions/`, `README.md`, `CONTRIBUTING.md`, the OpenSpec
artifacts and the commit messages.

If you are an agent working on this repository: **write code, comments and
diagnostics in English.** The rationale is in
[ADR-006](../decisions/ADR-006-language-of-the-codebase.md).

## Project context

You are helping build **Zirk**, a compiled, object-oriented programming language
with static typing and inference, high level by default and with optional
low-level access. It compiles to native binaries via LLVM. Structured
concurrency (`task`/`await`) and multicore parallelism
(`parallel`/`parallel for`/`thread`) are first-class features.

Philosophy: **easy by default, explicit when you need control.**

## Source of truth

The following documents, in `docs/`, are the complete normative specification of
the language. For any doubt about syntax, semantics or scope, **these documents
override your own judgement, your memory of other languages, and "whatever
sounds reasonable"**:

- `docs/ZIRK_SPEC_FINAL.md` — scope, explicit exclusions, general philosophy.
- `docs/ZIRK_LANGUAGE_SPEC.md` — syntax, types, objects, control flow, errors.
- `docs/ZIRK_COMPILER_SPEC.md` — pipeline, IR, LLVM, targets, diagnostics, CLI.
- `docs/ZIRK_RUNTIME_SPEC.md` — memory, tasks, scheduler, threads, resources.
- `docs/ZIRK_STDLIB_SPEC.md` — modules and contracts of the standard library.

If two documents contradict each other: `ZIRK_SPEC_FINAL.md` defines scope and
exclusions; the specialized document defines the semantics of its own area.

**Explicit rule from the spec itself, and a working rule here:** every ambiguity
must produce a question or be documented — never resolved silently by inventing
unspecified behaviour. If something is needed to move forward and the spec does
not cover it, say so explicitly before deciding on your own.

The architecture decisions in `docs/decisions/` **carry the same weight as the
specs**. They record what was already decided and why, so it is not re-litigated
in every session.

## The most important scope rule in this prompt

The full spec describes Zirk in its mature form — it is the result of years of
work, not the starting point. **The full spec is not implemented in one go.**
Work advances in phases (see `docs/init/ZIRK_ROADMAP.md`). In each session the
goal is to advance the current phase — not to jump ahead to features of later
phases even though they are documented, defined, and tempting to implement
because "it is all right there".

If a genuine need for something from a later phase appears along the way, say so
explicitly instead of implementing it halfway or silently.

## Implementation stack

- **Compiler language: Rust.** The reason is not aesthetic: the optimization of
  the binaries Zirk produces is done by LLVM in the backend, not by the language
  the compiler is written in — `ZIRK_COMPILER_SPEC.md` section 5 already settles
  that. What does depend on the compiler's language is how fast and how safely
  something of this size can be built and maintained; Rust gives memory safety,
  safe parallelism for the compiler itself (relevant for the incremental
  compilation the spec asks for), and mature LLVM bindings.
- **Codegen backend: LLVM**, through `inkwell` (safe Rust bindings over
  `llvm-sys`).
- **Structure: a Cargo workspace**, one crate per pipeline stage instead of a
  monolithic binary — it mirrors the pipeline of `ZIRK_COMPILER_SPEC.md`
  section 2 directly, and lets each stage be tested in isolation.

## Phase 0 — complete

The foundations exist and are verified in CI on Linux (x86_64 and aarch64),
macOS aarch64 and Windows x86_64:

- a workspace of nine crates, one per pipeline stage plus `zirk-diagnostics` and
  `zirk-runtime`;
- `zirk-diagnostics` implementing the format of `ZIRK_COMPILER_SPEC.md`
  section 8;
- object emission verified for the nine targets of the spec;
- the LLVM sanity check as a permanent test: it generates IR, emits an object,
  links and runs a native binary.

Before installing the toolchain, read `docs/TOOLCHAIN.md`. On Windows it is
mandatory: no official LLVM distribution works with `llvm-sys`.

## Phase 1 — complete

**Zirk compiles and runs.** This is the reference program of the roadmap,
compiled to a native binary and executed:

```zirk
fn main(): Void {
    stdout.println("Hola desde Zirk");
}
```

The whole spine works end to end: lexer → parser → name resolution and type
checking → typed IR → LLVM → object → link → a process that runs. The
corresponding decisions live in `docs/decisions/` as ADR-006 and ADR-007.

Implemented subset: `fn`, `Void`, `Int32`, `Boolean`, `String`, `mut`/`inmut`,
literals, arithmetic with overflow checks, comparison, logic, `if`/`else`,
calls, `return` and `stdout.println` as an intrinsic.

Available commands: `zirk build <file.zrk>` and `zirk run <file.zrk>`, over a
single file, with artifacts written to `build/`.

## Phase 2 — complete

**Zirk has a language surface.** Programs with several functions, real control
flow, closures and more than one file compile to a native binary and run.

Added on top of Phase 1: `for`, `for ... in` over ranges, `while`, `loop`,
`break`, `continue`, `if` as an expression, `match` with enforced exhaustiveness
over enums without associated data, `T?` with `null` and `??`, closures with
capture, optional/default/named parameters, compound assignment and increment,
and `share`/`import`/`use` across the files of a crate.

Decisions taken during the phase live in `docs/decisions/` as ADR-010, and in
the `design.md` of `fase-2-core-language-surface` as D1 to D10.

Short-circuiting, left pending by Phase 1, is done: `&&` and `||` lower to
blocks and do not evaluate their right operand when the left already decides.

### What this phase deliberately left pending

Each of these is reported today with **E0423**, so nothing reaches a backend
that cannot compile it. That diagnostic list is the pending work, made
executable: if it compiles, it works.

- **`?.`** needs a type with members, and classes are Phase 3 (design D8).
- **Variadic parameters** collect into a sequence, and there is no collection
  type until Phase 3 brings `List`.
- **Iterating a `String`** needs the runtime to expose access by character.
- **`+` on `String`** is not concatenation: it arrives with the operator
  contracts of Phase 3.

Two more, which produce no diagnostic because they are not user-visible:

- **A crate has one namespace.** Two declarations cannot share a name even in
  different files. Per-module namespacing belongs with the project system of
  Phase 6.
- **The IR is not versioned.** `ZIRK_COMPILER_SPEC.md` section 4 requires it for
  `.zpkg`; it is Phase 8 work, noted here so it is not discovered late.

## Next phase: Zirk 0.3 — objects and the type system

Phase 3 of `docs/init/ZIRK_ROADMAP.md`: `class`, `construct`, visibility,
single inheritance, interfaces, traits, generics with `from`, records, value
classes, algebraic enums, unions and casts.

The enum of this phase is meant to be **extended** by Phase 3 with associated
data, not replaced (design D1). The same goes for `for ... in`, which becomes a
real trait once traits exist (design D3), and for closures, which will be able
to escape once function types have syntax (design D10).

## Expected working style

- Prefer getting something to compile and run end to end over completeness of a
  single layer. A thin but complete pipeline is worth more right now than an
  exhaustive parser with no backend attached.
- Every new feature should ideally come with a minimal grammar, one valid test
  case and one invalid test case — as the spec itself requires in
  `ZIRK_SPEC_FINAL.md` section 8.
- Diagnostics should aim at the format of `ZIRK_COMPILER_SPEC.md` section 8
  (code, location, cause, help) from day one, even if the content is basic — it
  is far easier to keep the format from the start than to migrate it later.
- Ask before making high-impact architecture decisions (memory strategy, crate
  structure, internal IR format) that are not already settled in
  `docs/init/ZIRK_ROADMAP.md` phase 0 or in `docs/decisions/`.
