---
name: zirk-language
description: Write, explain, review, or repair Zirk `.zrk` source and `.zkinit` manifests using the repository's normative language contracts. Use for application, library, test, and example code; not for Rust compiler implementation unless the task also changes Zirk source semantics.
---

# Zirk Language

Write Zirk that is normatively correct and honest about the compiler in this
checkout. The language specification leads implementation in some domains: do
not claim a program runs until it is checked with the current local binary.

## Start and route by concept

1. Read [authority and validation](references/authority-and-validation.md).
2. Identify the user's data, control-flow, failure, safety, or project
   need. Read only the matching category below before writing code.
3. For a concrete `std.*` API, read its handbook owner too; never infer an API
   from another language.

| User intent | Read |
| --- | --- |
| Bindings, scalar values, nullability, conversions | [Foundations](references/foundations.md) |
| Arrays, lists, ranges, indexing, slices, iteration | [Collections and iteration](references/collections-and-iteration.md) |
| Conditions, loops, `match`, functions, lambdas | [Control flow and callables](references/control-flow-and-callables.md) |
| Classes, records, enums, unions, interfaces, generics | [Data modeling and generics](references/data-modeling-and-generics.md) |
| `Result`, `throws`, resources, permissions | [Failures, resources, and authority](references/failures-resources-and-authority.md) |
| Dates, times, durations, temporal parsing | [Authority guide](references/authority-and-validation.md) + handbook `03a-temporal/` |
| Decorators, generated APIs, reflection | [Authority guide](references/authority-and-validation.md) + `DECORATOR_SEMANTICS.md` |
| Files, imports, `.zkinit`, tests, executable verification | [Projects, modules, and validation](references/projects-modules-and-validation.md) |

Read [writing-zirk](references/writing-zirk.md) only when a request spans
several categories and needs a compact cross-category checklist.

## Authoring decision process

- Infer the desired value and its lifetime first. For example, “a list of 1 to
  100” means a resizable `List<Int32>`; `List(1..=100)` is shorter and clearer
  than a manual loop. If the length must not change, choose `Array<Int32>` or
  a fixed declaration such as `Int32[100]` instead.
- Prefer the narrowest language form that expresses the request. Use a range
  for a regular finite sequence, `for item in iterable` to consume it, and a
  traditional `for mut i = ...` only when the update is not a range.
- Keep Zirk naming and syntax: braces, semicolons, `snake_case` values and
  functions, `UpperCamelCase` types. Do not import TypeScript/Python/Rust/C#
  syntax by analogy.
- Treat whole reference assignment as aliasing; reads from an attribute,
  index, slice, destructuring, pattern, or projected capture are independent
  values. A projection on the left side of `=` writes original storage.
- Select one explicit failure channel: `Result<T, E>` for expected outcomes,
  `throws` for exceptional recoverable failure, and `fatalError` only for an
  unrecoverable invariant. There is no `?` propagation operator.
- If the request says “function”, first choose a top-level `fn`, a method, a
  lambda, or a generic callable value; if it says “object”, choose `class`,
  `record`, or a collection from the semantics, not from the noun alone.

## Delivering runnable code

When execution matters, consult feature status and validate with the checkout's
binary. Prefer `./target/debug/zirk` so a stale or unrelated `zirk` on `PATH`
cannot invalidate the result. Rebuild it when source is newer than the binary:

```sh
LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo build -p zirk-cli -p zirk-runtime
./target/debug/zirk check path/to/file.zrk
./target/debug/zirk run path/to/file.zrk
```

Report an unsupported or unverified construct plainly. Do not silently replace
the requested behavior with a familiar but different language feature.
