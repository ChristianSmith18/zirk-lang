# Diagnostic Codes

Diagnostics use a stable code such as `E1234`, severity, source location, focused span, cause, and repair guidance when available. Categories cover lexing/parsing, names/modules, typing/flow, safety/resources, permissions, concurrency, targets/packages, decorators, backend/linking, and tooling warnings.

Decorator diagnostics distinguish unsupported targets, missing dependencies, invalid visible order, self-dependencies, full cycles, noncontiguous repeatable groups, recursive expansion, denied build effects, generated public conflicts, and invalid generated syntax. They show the decorator application, target, expansion path, generated declaration, and actionable original span.

The specification defines the format but not a complete code registry. Until implemented, documentation must not invent permanent number assignments. Generated diagnostics include original and expansion locations.

---

**Previous:** [← Attributes and Decorators](06-attributes-and-decorators.md) · **Next:** [ CLI Commands](08-cli-commands.md)
