# Differences from Python

Zirk is statically typed before execution, has no dynamic rebinding across unrelated types, no universal `None`, no duck-typed public contracts, and no interpreter dependency at deployment.

Imports, permissions, resources, targets, dependencies, and errors are compiler-visible project contracts rather than runtime conventions.

Zirk retains Python-like String repetition (`"ja" * 3`) and expressive slicing, but String is grapheme-indexed and mutation is permission-controlled. Integer division with `/` truncates toward zero rather than using Python's floor division behavior. Numeric mixing and conversion are statically checked, and temporal meanings use distinct types rather than one general-purpose datetime object.

---

**Previous:** [← Differences from TypeScript](04-differences-from-typescript.md) · **Next:** [ Differences from Rust](06-differences-from-rust.md)
