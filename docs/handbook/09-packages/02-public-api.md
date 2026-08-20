# Public API

`public.api` records published types, signatures, traits, interfaces, decorator contracts, decorator-generated public declarations/descriptors, and documentation. Compatibility analysis compares these typed contracts rather than linker names or source layout; erased applications are not runtime API.

Permission and error behavior are part of the public promise even when not encoded by one function type.

Compatibility analysis distinguishes additive, source-breaking, behavior-
breaking, and authority-widening changes. Removing or narrowing a shared
declaration, changing generic variance/constraints, widening `throws`, changing
resource responsibility, or adding a required permission can require a major
version even if a native symbol happens to retain the same spelling.

Generated public declarations carry decorator provenance and are compared like
handwritten API. Private layout, optimizer choices, and mangled symbols are not
public contracts. `zirk prepare` produces a reviewable API diff before package
publication.

---

**Previous:** [← Package Anatomy](01-package-anatomy.md) · **Next:** [ Portable IR](03-portable-ir.md)
