---
name: zirk-language
description: Write, explain, review, or repair Zirk `.zrk` source and `init.zrk` manifests using the repository's normative language contracts. Use for application, library, test, and example code; not for Rust compiler implementation unless the task also changes Zirk source semantics.
---

# Zirk Language

Write Zirk that is both normatively correct and honest about the current
toolchain. Zirk is specified ahead of portions of its implementation: never
claim that an example runs merely because it is valid in the language design.

## Start here

1. Read [the authority and validation guide](references/authority-and-validation.md).
2. Read [the writing guide](references/writing-zirk.md) before authoring or
   repairing ordinary language code. It contains the high-value syntax,
   semantic invariants, and invalid substitutes that commonly leak in from
   TypeScript, Python, Rust, Java, or C#.
3. Read the topic-specific source documents named in the authority guide for
   resources, unsafe/native access, concurrency, decorators, permissions,
   packages, or standard-library APIs. Do not guess an API signature from a
   familiar language.

Use repository-relative paths from the Zirk checkout. The normative documents
are the source of truth; this skill is a decision guide and compact writing
reference, not a replacement for them.

## Working rules

- Preserve `.zrk` syntax and Zirk names in code; use the official formatter's
  style (braces, semicolons, `snake_case` for values/functions/files and
  `UpperCamelCase` for types).
- Begin a new executable project with `init.zrk`, `src/main.zrk`, and `fn
  main(): Void { ... }`. `init.zrk` is declarative, never executable code.
- Choose an explicit failure channel: `Result<T, E>` for expected operational
  failure, `throws` for exceptional recoverable failure, and `fatalError` only
  for unrecoverable state. Do not invent a `?` propagation operator.
- Respect reference boundaries: complete reference assignment aliases; a read
  of an attribute, index, slice, destructured component, pattern binding, or
  projected capture is an independent value (deep clone when reference-backed).
  A projection used as an assignment place writes original storage.
- Check feature availability separately from normative validity. When a
  runnable result is requested, run `zirk check <source>` (or `zirk check` in
  a project) and report the exact outcome. If the binary is unavailable or a
  requested construct is not implemented, say so rather than substituting
  another language feature.
- Keep permissions in the application `init.zrk`; libraries use `requires`.
  A declaration requests authority but never constitutes user consent.
- Prefer the simple safe form. Enter `unsafe {}` only for an operation in its
  closed low-level set, and use `commit {}` inside it before an irreversible
  effect. Use `match with` for `Resource<E>` ownership and structured `task`
  scopes/channels for concurrent work.

## Completion checklist

Before handing over code, check the applicable items:

- Names, declarations, types, nullability, mutation permissions, and named
  call labels match the writing guide.
- Every `Result` is consumed; every explicit thrown exception is caught or
  declared; every resource is closed by `match with` or explicitly transferred.
- Cross-task/thread/channel values meet transfer or sharing rules; no mutable
  alias, mutex guard, dependent view, or tentative unsafe state crosses an
  invalid boundary.
- Imports use quoted local/package paths and unquoted `std.*` modules; only
  `share` exports declarations and only `use` exposes declared globals.
- For docs or examples, label normative-but-unimplemented material rather than
  presenting it as executable. For executable claims, include the verification
  command and result.
