# Globals

Application globals live only in the manifest's `globals` block and initialize deterministically before `main`. A failure prevents entry execution and emits a deterministic diagnostic.

Consumers opt in with `use`. Shared mutable access requires synchronization; prefer explicit dependencies and immutable configuration.

---

**Previous:** [← Entry Point](./05-entry-point.md) · **Next:** [Runtime Permissions →](./07-runtime-permissions.md)
