# Lexer and Parser

The lexer preserves tokens and spans; the parser builds syntax suitable for error recovery and tooling. Incremental snapshots reuse unchanged structure. Malformed input produces localized diagnostics without fabricating valid semantics.

---

**Previous:** [← Compiler Pipeline](01-compiler-pipeline.md) · **Next:** [ Name Resolution](03-name-resolution.md)
