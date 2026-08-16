# How Zirk Compiles

Zirk turns `.zrk` source into a native executable through a staged pipeline. Understanding the stages helps you interpret diagnostics and choose the right command when something goes wrong.

```text
source → tokens → syntax tree → names → types and flow
       → validated expansion → portable typed IR
       → LLVM IR → object files → linked binary
```

## Frontend stages

The incremental lexer identifies tokens. The parser builds syntax structure. Module and name resolution connect each reference to a declaration. Type checking and flow analysis then verify operations, narrowing, initialization, nullability, exhaustiveness, and other semantic rules.

These stages power `zirk check`, formatting, linting, and language-server features without invoking LLVM. Shared persistent structures, symbol interning, content-addressed caches, and a fine-grained dependency graph allow an edit to invalidate only affected files and symbols.

## Validated metaprogramming

Decorators and tools use a public, immutable, versioned Syntax API rather than mutating compiler internals. Generated transformations return to parsing, resolution, typing, and validation. Diagnostics retain a relationship between original and generated source.

Compile-time code does not receive unrestricted filesystem, network, or process access. A package must declare `compile_permissions`, and the application build audits the resulting capability set.

## Portable typed IR

After semantic checks, the compiler produces a typed intermediate representation independent of the final target. It preserves information needed for generic specialization, devirtualization, escape analysis, safety checks, vectorization, and debug metadata.

Distributed `.zpkg` packages contain public API information and portable IR rather than one machine-specific implementation. During the final application build, package code, the standard library, runtime, and application are aligned to the same target and ABI.

## Native backend and linker

LLVM is the sole initial backend. It lowers the typed IR for the selected architecture and produces object code. A linker and applicable platform SDK finish the Mach-O, ELF, or PE binary.

Debug builds prioritize source correspondence and complete symbols. Release builds can apply stronger optimization, dead-code removal, link-time optimization, vectorization, and size improvements without changing language guarantees.

## Why errors appear at different times

- Lexical and parser diagnostics report malformed source structure.
- Resolution diagnostics report missing, ambiguous, private, or cyclic names.
- Type and flow diagnostics report incompatible operations or unsafe paths.
- Target checks report unsupported SDKs or incompatible native dependencies.
- Linker diagnostics concern symbols or native artifacts not resolved earlier.
- Runtime errors concern behavior that cannot be decided statically.

The toolchain should diagnose a problem at the earliest stage with enough context to fix it. A target incompatibility discovered from package metadata should not be deferred to an opaque linker failure.

## Normative source

[Zirk Compiler and Tooling Specification](../../ZIRK_COMPILER_SPEC.md), §§1–8.

---

**Previous:** [← Your First Zirk Program](05-first-program.md) · **Next:** [ Next Steps](07-next-steps.md)
