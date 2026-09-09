# Authority, status, and validation

## Authority order

Resolve language questions in this order:

1. `docs/ZIRK_SPEC_FINAL.md` decides Zirk 1.x identity, scope, exclusions, and
   cross-document conflict resolution.
2. Use the consolidated semantic owner for the affected domain:
   - `docs/CORE_LANGUAGE_SEMANTICS.md` — values/references/projections,
     callables, classes/contracts, generics, algebraic data, collections.
   - `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` — `Result`, exceptions,
     `Resource`, `match with`, application/library authority, approval.
   - `docs/MEMORY_AND_UNSAFE_SEMANTICS.md` — managed references, `Weak`, deep
     clone, pointers/views, `unsafe`, transaction rollback, `commit`.
   - `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` — `task`, cancellation,
     `select`, channels, `parallel`, `thread`, transfer/share, synchronization.
   - `docs/DECORATOR_SEMANTICS.md` — `fn dec`, target blocks, expansion,
     wrappers, ordering, generated APIs.
3. Read the specialized detail that has not been superseded:
   `docs/ZIRK_LANGUAGE_SPEC.md`, `docs/ZIRK_STDLIB_SPEC.md`,
   `docs/ZIRK_RUNTIME_SPEC.md`, and `docs/ZIRK_COMPILER_SPEC.md`.
4. Use the relevant `docs/handbook/` chapter for examples, teaching language,
   and navigable detail. `docs/handbook/13-appendices/09-normative-sources.md`
   records the same source order.

ADRs, `docs/01_plantilla_zirk.md`, roadmaps, archived OpenSpec material, and
old phase designs are context or rationale only. They never override a final
checkpoint. If equally authoritative sources still conflict, retain the
ambiguity, identify the conflicting passages, and ask for a language decision
or report it; do not silently choose a syntax or semantic rule.

## Normative target versus current compiler

The documentation has two layers:

- **Normative Zirk 1.x** defines the intended syntax and observable behavior.
- **Current implementation** is the compiler/runtime/library present in this
  checkout and can lag the normative target.

For any claim that code compiles or runs, inspect
`docs/init/ZIRK_FEATURE_STATUS.md` and validate in the current checkout:

```sh
zirk check path/to/file.zrk
zirk run path/to/file.zrk
zirk test
```

Use the narrowest applicable command. If working on compiler fixtures instead,
follow the repository test conventions in `AGENTS.md`; a documentation example
is not a passing fixture by itself. If `zirk` cannot be run, state that the
example is normative only or that executable status was not verified.

## Routing by request

| Need | Read before writing |
| --- | --- |
| Basic declarations, types, expressions, functions, classes, generic/data/collection code | `writing-zirk.md`, `CORE_LANGUAGE_SEMANTICS.md`, matching handbook chapter |
| Dates, zones, durations, parsing | handbook `03a-temporal/` and `ZIRK_STDLIB_SPEC.md` §8 |
| `Result`, `throws`, `try`, resources, manifest permissions | failure/permission semantics plus handbook `15-errors`, `16-resources`, `03-projects` |
| Tasks, channels, CPU work, threads, locking | structured-concurrency semantics plus handbook `18-concurrency` |
| Pointers, C ABI, atomic ordering | memory/unsafe semantics plus handbook `17-memory-and-safety` and `05-native-and-low-level` |
| A concrete `std.*` API | handbook `04-standard-library` chapter plus `ZIRK_STDLIB_SPEC.md`; verify its signature, errors, blocking/cancellation, and permission |
| Decorators or reflection | decorator semantics plus handbook `06-metaprogramming` |
| Modules, packages, project layout, dependencies | handbook `19-modules`, `03-projects`, `09-packages`, plus `ZIRK_COMPILER_SPEC.md` |
| Tests, CLI, formatting, diagnostics | handbook `08-testing`, `07-toolchain`, and `ZIRK_COMPILER_SPEC.md` §§8–10 |

## Explicit exclusions that must not be improvised

Zirk 1.x excludes WebAssembly/browser DOM integration, `async fn`, a public
event loop, a standalone `worker`, textual inline assembly, general
`comptime {}`, general `defer`, multiple class inheritance, ordinary function
overloading, `?` Result propagation, public ownership/reference-counting
semantics, and arbitrary decorator AST mutation. Use the specified replacement
(`task`, `match with`, generics/unions/distinct names, C ABI wrappers, etc.) or
surface the design gap.
