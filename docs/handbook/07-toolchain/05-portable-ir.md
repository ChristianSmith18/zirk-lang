# Portable IR

Typed target-independent IR preserves generic, identity, safety, escape, vectorization, and debug information. Its version participates in cache and package compatibility; final specialization occurs with the application target.

## Contract

Every value and operation carries a type and source location. Control flow is
explicit basic blocks and terminators, so verification can reject missing
returns, invalid branches, type mismatches, and recovered frontend nodes before
LLVM sees them. The IR records object/enum layouts, callable effects, reference
provenance, dependent lifetimes, task scopes, cancellation edges, transfer and
share facts, unsafe transaction boundaries, and irreversible commit points.

The IR is portable, not universal machine code. It avoids host pointer widths,
linker names, and platform calling conventions until target lowering. Generic
code may remain parameterized in a package; the application build specializes
the reachable combinations for one target.

## Package and cache compatibility

`portable.ir` is versioned independently of the source language and target ABI.
Its cache key includes compiler version, IR version, target-relevant flags,
dependency public APIs, configuration, and expansion fingerprints. An unknown
or incompatible IR version is a diagnostic, never a best-effort decode.

Optimization may prove a safety check or rollback journal unnecessary, but the
optimized IR must preserve the same overflow, identity, resource, cancellation,
permission, and error behavior. The independent verifier runs after lowering
and after transformations that can invalidate those invariants.

> **Implementation status:** typed three-address IR and verification exist for
> the implemented compiler subset. Versioned portable package IR, complete
> safety/concurrency metadata, and stable serialization remain phased work.

---

**Previous:** [← Diagnostic Recovery](04b-diagnostic-recovery.md) · **Next:** [ LLVM Backend](06-llvm-backend.md)
