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
- `docs/CORE_LANGUAGE_SEMANTICS.md` — callables, projections, objects,
  generics, data, collections, and iteration.
- `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` — expected/exceptional failure,
  resources, permissions, and approval.
- `docs/MEMORY_AND_UNSAFE_SEMANTICS.md` — managed memory, dependent references,
  pointers, transactional unsafe rollback, and irreversible commit.
- `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` — tasks, scopes, cancellation,
  aggregation, selection, transfer, parallelism, and synchronization.

If two documents contradict each other: `ZIRK_SPEC_FINAL.md` defines scope and
exclusions; the specialized document defines the semantics of its own area.
The four consolidated semantic documents are the newest authoritative
checkpoints for their named areas and supersede shorter historical examples.

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

## Phase 3 — complete

**Zirk is object-oriented.** `class`, `construct`, visibility
(`public`/`private`/`protected`), single inheritance with `super`/`override`,
interfaces and traits (with reusable default bodies), generics with `from`
constraints (specialized per instantiation, not erased), records and value
classes (inline, no allocation), algebraic enums with associated data, and
checked casts (`as`) all compile to a native binary and run.

The three debts Phase 2 deferred to this phase are retired: `?.` (both a field
and a method through it), `+` on `String`, and `for ... in` over a type's own
`Iterable<T>` — ranges and `String` now implement it like any other type would,
so Phase 2's corpus keeps compiling unchanged.

Decisions taken during the phase live in `docs/decisions/` as ADR-012 and
ADR-013, and in the `design.md` of `fase-3-objects-and-type-system` as D1 to
D11.

### What this phase deliberately left pending

Reported with **E0423** the same way Phase 2's pending work was, so nothing
reaches a backend that cannot compile it:

- **Function types have no syntax yet** (design D9): a closure infers its type
  locally and can be called, but cannot be annotated as a parameter, return or
  field type, and cannot escape the function that created it. The semantics
  beyond that limit are already decided (D9) for whichever future phase
  implements the syntax.
- **A generic contract or enum instantiation** works end to end only for the
  language's own `Iterable<T>`/`Iterator<T>`/`Iteration<T>` (needed by
  `for ... in`); a user's own generic contract or enum stays gated the same
  way it was before this phase.
- **`abstract class`** type-checks completely — a concrete class adopts its
  requirements with `implements` and `override fn`, and conformance is
  verified — but a value typed *through* the abstract class would need
  dynamic dispatch through whichever concrete class adopted it, and that path
  does not exist yet.
- **A record or value class implementing a contract** is checked for real
  conformance, and its own methods dispatch statically when called directly on
  the concrete type — but it has no descriptor to carry the contract's own
  table, so reaching one through the contract type is not compilable yet.
- **Structural equality on a record or value class** (`==`/`!=`) type-checks
  without a reserved method — the language derives it from every field — but
  lowering the comparison itself does not exist yet.
- **`unsafe {}` and reinterpreting casts** stay out of scope, same as
  Phase 2's `?.`: they need machinery later phases own.

A note on something this phase resolved rather than left pending: **there is
no ordinary shadowing.** Any local declaration — `let`/`mut`, a function or
method parameter, a `for ... in` binding, a `match` pattern's own — that
shares a name with one still visible, in the same block, a nested one, or
across a lambda's own capture boundary, is rejected
(`codes::ORDINARY_SHADOWING`, D10). `ZIRK_SPEC_FINAL.md` section 6 is the
normative source ("Ordinary local shadowing is rejected; an explicit lambda
capture collision uses `this.name`"), which settles what had been, until
closeout, an open contradiction between `design.md`'s D10 and an older
Phase 2 corpus program that tested the opposite — the merged
`zirk-type-system` spec had already stated the general rule beforehand. A
field is never affected — it is always read as `this.name`, never a bare
name, so there is nothing for a local to shadow.

## Phase 3b — complete

**Zirk's scalars, conversions and text are complete.** The ten integer widths
(`Int8`…`Int128`, `UInt8`…`UInt128`), the binary floating family
(`Float16`/`Float32`/`Float64`/`Float128`, `Float` aliasing `Float64`), `Char`
as one Unicode extended grapheme cluster, bitwise/shift operators, deep
contextual conversion (`Float(3 / 4)`, `String("x=" + 42)`), and a real
`to_string()` contract that `print`/`println` and string interpolation
(`"{expr}"`) both route through, all compile to a native binary and run.

Decisions taken during the phase live in `docs/decisions/` as ADR-014, and in
the `design.md` of `fase-3b-scalars-and-text` as D1 to D3; its `tasks.md`
carries the resolution of every open question section by section.

### Follow-up: the remaining loose ends were closed

A second pass retired five of the seven gaps first left open at the end of
the phase:

- **`for ... in` over `String`** now compiles — it walks the string's own
  graphemes with a byte offset, the runtime answering "is there a next
  grapheme, and how many bytes is it" (`zirk_str_grapheme_len_at`) and
  building the `Char` from that range (`zirk_str_grapheme_slice`); the loop
  itself threads the offset the same way a range loop already threads its
  own counter, needing no new IR instruction that mutates anything.
- **`Float16` now has `to_string()`**: it prints by widening to `Float32`
  first, through the same `fpext` a `Float16 → Float32` `as` already uses —
  always exact, since every `f16` value is representable in `f32` without
  loss, so `f32`'s own `Display` prints the same value `f16` held, not an
  approximation of it.
- **`to_string()` through a contract reference** now dispatches — a value
  reached through a contract that declares `to_string()` calls through the
  object's own dispatch table for it, the exact shape any other contract
  method call already had (`CallContract`).
- **Explicit `value.to_string()` on a native scalar** is now callable
  (`myInt.to_string()`, `"already".to_string()`), the explicit spelling of
  the same conversion `println`/interpolation already reached implicitly.

### What is still pending

- **Integer literal width inference from a simple assignment context**
  (`mut x: Int8 = 5;` without `as`) is not implemented — a literal always
  types `Int32` unless an explicit `as` or a deep contextual conversion
  supplies the width. There is no literal suffix syntax for it either (only
  `Float` has one, e.g. `1.5f32`); this was a deliberate scope decision
  during the phase's own literal-suffix task, not an oversight.
- **`Float128` arithmetic is unverified on Windows**: LLVM lowers `fp128`
  operations to soft-float library calls (`__addtf3` and similar) the MSVC
  toolchain this project's CI links against does not provide the way
  `glibc`/`libSystem` do on Linux/macOS — it crashed the Windows runner
  (an access violation, not a failing assertion) rather than just failing a
  test, so `Float128` is excluded from the cross-platform corpus.
- **`Float128` still has no `to_string()`**: unlike `Float16`, there is no
  lossless narrower width to widen through instead — an `f64` cannot
  represent every `f128` value exactly the way `f32` can every `f16` — and
  it is not a stable Rust primitive in this toolchain either way. Printing or
  interpolating one is rejected at compile time with a clear diagnostic, not
  silently wrong.
- **An enum cannot implement `to_string()`**, or any method at all — enums
  have no method table yet (`EnumType` has no `methods` field, unlike
  `ClassType`). Pre-existing debt from Phase 3, not new here, and larger than
  a `to_string()`-specific fix — it needs enum methods to exist at all.

## Next phase: Zirk 0.4 — errors and memory

Phase 4 of `docs/init/ZIRK_ROADMAP.md`: `Result<T, E>` with exhaustive
`match`, `try`/`catch`/`finally`, `fatalError`, the full memory strategy
decided in Phase 0 (safe/weak/dependent references, deep clone graph
semantics, automatic bounded native pinning), `unsafe {}`/`Pointer<T>`/native
slices, transactional write journals with explicit irreversible `commit`,
`Resource<E>`/`match with`, and `inmut::strict`'s alias-analysis-backed deep
immutability.

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
