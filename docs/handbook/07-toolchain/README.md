# Toolchain

The `zirk` toolchain shares one incremental frontend across checking, formatting, linting, editor services, documentation, and native compilation. LLVM is invoked only when code generation is needed.

Begin with the [Compiler Pipeline](01-compiler-pipeline.md), then follow the
frontend through [Lexer and Parser](02-lexer-and-parser.md),
[Name Resolution](03-name-resolution.md), [Type Checker and Flow
Analysis](04-type-checker.md), [Decorator
Expansion](04a-decorator-expansion.md), and [Diagnostic
Recovery](04b-diagnostic-recovery.md). Each chapter distinguishes the current
implemented subset from the target architecture.

---

**Previous:** [← Security and Permissions](../06-metaprogramming/09-security-and-permissions.md) · **Next:** [ Compiler Pipeline](01-compiler-pipeline.md)
