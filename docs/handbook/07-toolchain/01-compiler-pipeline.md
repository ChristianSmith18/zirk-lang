# Compiler Pipeline

Source passes through incremental lexing, parsing, name resolution, type/flow analysis, validated decorator expansion, typed portable IR, LLVM IR, object generation, and linking. Each stage emits source-aware diagnostics and invalidates only affected dependencies.

---

**Previous:** [← Toolchain](./README.md) · **Next:** [Lexer and Parser →](./02-lexer-and-parser.md)
