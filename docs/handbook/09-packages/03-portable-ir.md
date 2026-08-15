# Portable IR

Portable typed IR preserves generic specialization, safety checks, devirtualization, escape analysis, vectorization, and debug metadata without selecting a machine target.

The final build compiles package IR with application, stdlib, and runtime for one aligned target and ABI. IR version compatibility is checked before code generation.

---

**Previous:** [← Public API](./02-public-api.md) · **Next:** [Add, Remove, and Install →](./04-add-remove-install.md)
