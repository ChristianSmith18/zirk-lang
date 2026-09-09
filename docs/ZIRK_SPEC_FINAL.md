# Zirk — Master specification

Status: living normative design for Zirk 1.x
Initial publication: 12 August 2026
Last normative checkpoint: 20 August 2026
Source extension: `.zrk`
CLI: `zirk`
Manifest: `init.zrk`
Lockfile: `zirk.lock`
Distributable package: `.zpkg`

> **Authorial language checkpoint — 15 August 2026.** The language author's
> numbered handbook annotations supersede older omissions and contradictory
> examples in the 12 August snapshot. The specialized language and standard
> library specifications now define exponentiation; complete colon-step range,
> range-expansion, fixed-array, and slicing
> forms; classic `for`, single-statement `if`, and `do ... while`; regex
> literals and patterns; comma-grouped `match`; typed optional and iterable
> variadic parameters; optional `fn` lambdas; qualified closure captures; class
> field defaults; constructor signatures; operator methods; enum mappings;
> fixed arrays; generators; pure-function pipelines; standard-library
> convenience imports; and bound callable cloning. This checkpoint is
> normative even where the current compiler has not implemented the surface.

> **Authorial type-system checkpoint — 15 August 2026.** Zirk distinguishes
> compiler primitives, native value types, native reference types,
> user-defined value/reference types and special types under the conceptual
> `Object` root. `Float` is an exact base-ten decimal (the default fractional
> type, no `NaN`, no infinity); the IEEE 754 binary family is
> `Float16`–`Float128` with `Float == Float64` and a
> `f` literal suffix, also with no valid `NaN`. Explicit `Float(...)` and
> `String(...)` constructors establish deep contextual evaluation for their
> contained arithmetic or concatenation tree. `Char` is exactly one Unicode
> grapheme. `String` is a mutable shared reference governed by `mut`, `inmut`
> and transitive `inmut::strict`, supports checked grapheme mutation and
> slicing, cloning, concatenation and repetition. The sealed temporal family
> contains `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`,
> signed exact `Duration` and calendar `Period`. These decisions supersede all
> earlier Decimal, immutable-String, code-point Char and undifferentiated Date
> descriptions.

> **Authorial core-language checkpoint — 16 August 2026.** The writable
> callable type is `Function(P...) => R`, conventionally `Fn(P...) => R`;
> compatible functions, lambdas and methods adapt to it and closures may escape
> under compiler-managed storage. Assigning, passing, returning or capturing a
> whole reference shares it, while reading an attribute, index, slice,
> destructured component or pattern binding produces an independent deep clone;
> the same projection used as a place mutates original storage. Zirk has
> attributes and ordinary `get_`/`set_` methods, not properties. Abstract
> classes are nominal requirement sets adopted through `implements`; only
> concrete classes use `extends`. Generics support combined `from A & B`
> constraints, defaults, declared `in`/`out` variance, recursion and managed
> `Box<T>`. Tuples, records, data-only enums, normalized unions, exhaustive
> guard-free matching, collection copying, `Iteration<T>` and Python-shaped
> omitted slice components follow the detailed language specification. These
> are final language semantics even when delivery belongs to a later phase.

> **Authorial failure, resource, and permission checkpoint — 16 August 2026.**
> `Result<T,E>` is mandatory for expected failure; explicit exceptions are
> checked through `throws`, while typed implicit `RuntimeError` safety failures
> remain catchable without signature noise. Pattern-shaped `catch`, exact
> rethrow, immutable throwable identity, causes, suppressed failures, and lazy
> traces are final. `match with` closes `Resource<E>` exactly once, composes
> body/close failures, and permits only explicit safe transfer. Libraries use
> `requires`, applications use `permissions`, and `during` separates build from
> runtime authority. Consent is a signed external record bound to project name
> and canonical location plus exact requester fingerprints; manifest edits do
> not grant authority. Validation is incremental and reapproval follows any
> permission widening, requester update, project rename, or project move.

> **Authorial memory and concurrency checkpoint — 17 August 2026.** Managed
> memory remains strategy-neutral and safe references use compiler-checked
> dependent lifetimes without public lifetime syntax. `Weak<T>` upgrades to
> `Option<T>`; bounded native views are preferred to raw pointers; deep cloning
> preserves graph topology. Ordinary `unsafe` blocks transactionally journal
> managed and validated-range writes and roll them back on controlled failure;
> irreversible effects require an explicit `commit` boundary. Tasks are typed,
> scoped and never orphaned. Failure cancels siblings, cancellation and timeouts
> await cleanup, `Task.settled` preserves every outcome, and fair `select`
> coordinates tasks, channels, timers and cancellation. Transfer/share safety
> is compiler-derived, and safe code is data-race free.

Contributors implementing these decisions must next read the consolidated
[`CORE_LANGUAGE_SEMANTICS.md`](CORE_LANGUAGE_SEMANTICS.md) checkpoint before
consulting phase plans or historical inventories.

Then read [`ERROR_RESOURCE_PERMISSION_SEMANTICS.md`](ERROR_RESOURCE_PERMISSION_SEMANTICS.md)
before implementing failures, cleanup, external effects, manifests, packages,
or permission tooling.

Read [`MEMORY_AND_UNSAFE_SEMANTICS.md`](MEMORY_AND_UNSAFE_SEMANTICS.md) before
implementing allocation, references, cloning, native interop, pointers, unsafe
operations, or rollback. Read
[`STRUCTURED_CONCURRENCY_SEMANTICS.md`](STRUCTURED_CONCURRENCY_SEMANTICS.md)
before implementing tasks, channels, parallelism, threads, synchronization, or
race analysis.

> **Language of this document.** Every normative specification, and the codebase
> itself, is written in English. See
> [decisions/ADR-006-language-of-the-codebase.md](./decisions/ADR-006-language-of-the-codebase.md).

## 1. Identity

Zirk is a compiled, general-purpose, object-oriented language, statically typed
with inference, high level by default and with optional low-level access. It
compiles to native binaries and treats explicit concurrency and multicore
parallelism as first-class features.

Its philosophy is:

> Easy by default, explicit when you need control.

The name comes from transforming the name *Crist*: reversing its sound yields
*Sirc*, and its letters are then modified until reaching **Zirk**, preserving
that sound with an identity of its own.

## 2. Initial scope

Zirk 1.x targets backend applications, CLIs, desktop, systems and native
libraries. Binaries are standalone and require no installation of Node.js,
Python, Java or any other language.

Initial targets, where the combination is supported by LLVM, the linker and the
dependencies:

- Windows, Linux and macOS.
- `x86`, `x86_64`, `armv7` and `aarch64`.
- Mach-O, ELF and PE.
- cross-compilation through `zirk build --target <target>`.

`build_targets` in `init.zrk` allows producing several targets. An explicit
`--target` takes precedence. With neither, the host is detected.

## 3. Outside the initial scope

The following are not part of Zirk 1.x:

- a WebAssembly target or native browser/DOM integration;
- `@runtime`, `@target`, `@host` or `@platform` directives;
- a `worker` as an independent primitive: it is composed from `task`, `thread`
  and `Channel<T>`;
- `async fn`: `task` and `await` express asynchrony;
- a public or manually managed event loop;
- inline textual assembly;
- general `comptime {}`;
- general `defer`;
- multiple class inheritance;
- traditional function overloading;
- the `?` propagation operator for `Result`;
- ownership or reference counting as public semantics;
- a custom backend, or several backends up front.

These exclusions must not be reinterpreted as gaps an implementation is free to
fill in.

## 4. Normative documents

This specification is split into:

- [ZIRK_LANGUAGE_SPEC.md](./ZIRK_LANGUAGE_SPEC.md): syntax, types, objects,
  control flow, errors, modules, metaprogramming and safety.
- [ZIRK_COMPILER_SPEC.md](./ZIRK_COMPILER_SPEC.md): frontend, Syntax API, IR,
  LLVM, targets, diagnostics and tooling.
- [ZIRK_RUNTIME_SPEC.md](./ZIRK_RUNTIME_SPEC.md): memory, execution, tasks,
  scheduler, I/O, threads, resources, cancellation and shutdown.
- [ZIRK_STDLIB_SPEC.md](./ZIRK_STDLIB_SPEC.md): modules and minimum contracts of
  the standard library.
- [CORE_LANGUAGE_SEMANTICS.md](./CORE_LANGUAGE_SEMANTICS.md): consolidated
  callables, references, objects, generics, data, and collection semantics.
- [ERROR_RESOURCE_PERMISSION_SEMANTICS.md](./ERROR_RESOURCE_PERMISSION_SEMANTICS.md):
  consolidated failure, cleanup, resource, and authority semantics.
- [MEMORY_AND_UNSAFE_SEMANTICS.md](./MEMORY_AND_UNSAFE_SEMANTICS.md): managed
  memory, references, native views, pointers, unsafe rollback, and commit.
- [STRUCTURED_CONCURRENCY_SEMANTICS.md](./STRUCTURED_CONCURRENCY_SEMANTICS.md):
  tasks, cancellation, aggregation, select, channels, parallelism, and races.
- [DECORATOR_SEMANTICS.md](./DECORATOR_SEMANTICS.md): decorator declarations,
  targets, phases, composition, erasure, and generated framework API.

If two documents contradict each other, this master specification defines scope
and exclusions; the specialized document defines the semantics of its own area.
Any remaining ambiguity must produce a diagnostic or be documented before being
implemented, never resolved silently.

## 5. Minimal project

```text
my_app/
├── init.zrk
├── zirk.lock
├── src/
│   └── main.zrk
└── test/
```

```text
project {
    name: "my-app";
    version: "0.1.0";
    type: application;
    entry: "src/main.zrk";
}
```

```text
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Hello from Zirk");
}
```

## 6. Foundational contracts

- Every value belongs under the conceptual `Object` root; simple values may be
  represented inline without becoming heap objects.
- On references, `mut` permits reassignment and referent mutation, `inmut`
  prevents reassignment but permits referent mutation, and `inmut::strict`
  requires deep immutability and forbids mutable aliases to the same referent.
- `null` inhabits only `T?` types; there is no `undefined`.
- `==` compares structurally and `is` checks identity on reference types.
- `Result<T, E>` represents expected failures; exceptions represent recoverable
  exceptional situations; `fatalError` terminates on unrecoverable state.
- Memory is automatic. Stack, heap, escape analysis, moves and RC are internal
  decisions.
- Safe code admits no use-after-free, null dereference, data races or undefined
  behaviour.
- Pointers and unsafe casts require `unsafe {}`.
- Tasks use structured concurrency; `parallel` requests multicore CPU work;
  `thread` represents a real operating-system thread.
- The runtime may use an event reactor internally, but Zirk exposes no global
  event loop.
- `share` publishes declarations, `import` brings them in, and `use` enables
  globals from `init.zrk`.
- Ordinary local shadowing is rejected; an explicit lambda capture collision
  uses `this.name`.
- All arrays have fixed length; `List<T>` is the resizable sequence.
- A class field without modifiers is `public mut`; constructors may have
  distinct signatures although ordinary functions remain non-overloaded.
- Whole references alias; projections read as values deep-clone, while
  projections used as assignment places retain direct access to storage.

## 7. Distribution and security

An `application` may declare globals and grants the final permissions. A
`library` cannot declare globals; it declares `requires`. The application uses
`permissions` with per-operation `during: build | runtime | both`. Packages include a typed public API, a portable
intermediate representation, a manifest, documentation and a license. In the
final build, every piece is compiled for the same target.

Permissions are finite capabilities declared in `init.zrk`, but declaration is
not consent. A signed external approval binds exact authority/requesters to the
project name and canonical location. Trusted interactive commands may show and
apply a narrow manifest diff only after explicit consent; unchanged signed
fingerprints take an incremental fast path. CI is noninteractive and deployed
programs never prompt. Tokens and secret values are never stored in `init.zrk`,
`zirk.lock`, diagnostics, or approval history.

## 8. Completeness criterion

A conforming implementation must accompany every feature with:

1. a grammar;
2. typing rules;
3. observable semantics;
4. compile-time diagnostics;
5. runtime errors;
6. valid and invalid examples;
7. interaction with mutability, concurrency and targets;
8. conformance tests.

The historical design checkpoint is not normative where it contains questions,
alternatives or text marked as pending.
