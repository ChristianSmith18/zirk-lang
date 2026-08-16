# Public API

A package's public API consists of published types, signatures, traits, interfaces, decorator contracts, and documentation—not private implementation details.

Visibility, `share`, and package boundaries all participate. Public API is recorded as typed metadata in `.zpkg` and drives compatibility and dependent compilation.

Do not expose a type merely because an internal implementation happens to return it; design the boundary intentionally.

---

**Previous:** [← use and Globals](05-use-globals.md) · **Next:** [ Circular Dependencies](07-circular-dependencies.md)
