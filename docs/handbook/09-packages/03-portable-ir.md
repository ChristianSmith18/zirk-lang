# Portable IR

Portable typed IR preserves generic specialization, safety checks, devirtualization, escape analysis, vectorization, and debug metadata without selecting a machine target.

The final build compiles package IR with application, stdlib, and runtime for one aligned target and ABI. IR version compatibility is checked before code generation.

Portable does not mean trustworthy. The consumer verifies package integrity,
parses the versioned format defensively, validates typed IR invariants, and
re-runs target/safety checks before optimization. Unknown features or versions
produce a package diagnostic rather than falling back to an older meaning.

Generic specialization and target layout happen in the consuming build. A
package may also declare native artifacts, but those are separate target-
specific inputs and never masquerade as portable IR.

---

**Previous:** [← Public API](02-public-api.md) · **Next:** [ Add, Remove, and Install](04-add-remove-install.md)
