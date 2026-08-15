# Zirk — Master specification

Status: initial normative design
Date: 12 August 2026
Source extension: `.zrk`
CLI: `zirk`
Manifest: `init.zrk`
Lockfile: `zirk.lock`
Distributable package: `.zpkg`

> **Authorial language checkpoint — 15 August 2026.** The language author's
> numbered handbook annotations supersede older omissions and contradictory
> examples in the 12 August snapshot. The specialized language and standard
> library specifications now define exponentiation; complete range and slicing
> forms; classic `for`, single-statement `if`, and `do ... while`; regex
> literals and patterns; comma-grouped `match`; typed optional and iterable
> variadic parameters; optional `fn` lambdas; qualified closure captures; class
> field defaults; constructor signatures; operator methods; enum mappings;
> fixed arrays; generators; pure-function pipelines; standard-library
> convenience imports; and bound callable cloning. This checkpoint is
> normative even where the current compiler has not implemented the surface.

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
    stdout.println("Hola desde Zirk");
}
```

## 6. Foundational contracts

- Every value semantically belongs to a class; simple values may be represented
  inline.
- `mut` allows reassignment, `inmut` prevents reassigning the reference, and
  `inmut::strict` requires deep immutability.
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

## 7. Distribution and security

An `application` may declare globals and grants the final permissions. A
`library` cannot declare globals; it declares `requires` and
`compile_permissions`. Packages include a typed public API, a portable
intermediate representation, a manifest, documentation and a license. In the
final build, every piece is compiled for the same target.

Permissions are finite capabilities declared in `init.zrk`. `zirk prepare`
audits and may propose changes; `zirk build` is strict and grants no permissions
interactively. Tokens and secrets are never stored in `init.zrk` or in
`zirk.lock`.

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
