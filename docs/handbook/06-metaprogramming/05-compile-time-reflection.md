# Compile-Time Reflection

Decorators inspect typed declarations through the target-specific Syntax API; they do not receive the compiler's private AST. All compiler-provided values originate in visible target bindings or match payloads. General `comptime {}` remains excluded.

A generic decorator expands once against its declaration and constraints, then specializes normally. Explicitly generated decorator applications enter a bounded later round; generated declarations are not decorated implicitly. Structural cycle detection and resource limits stop recursive expansion.

Expansion is incrementally cached by decorator implementation/version, arguments, typed target, configuration, grants, observed build inputs, and transitive dependencies. Unchanged fingerprints reuse validated output. External inspection requires matching application permission with `during: build` or `both`.

---

**Previous:** [← Syntax API](04-syntax-api.md) · **Next:** [ Runtime Reflection](06-runtime-reflection.md)
