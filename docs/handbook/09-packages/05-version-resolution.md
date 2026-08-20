# Version Resolution

Resolution selects one compatible graph from declared constraints, package metadata, target requirements, and existing lock state. Failure reports the constraint chain rather than guessing or silently downgrading.

`zirk update` explicitly recalculates locked versions; ordinary locked builds do not.

Resolution is deterministic for the same registry snapshot, constraints,
target, feature/configuration inputs, and existing lock state. It rejects
dependency cycles that cannot be represented safely, incompatible native
targets, withdrawn/integrity-conflicting releases, and requester permission
requirements the application cannot grant.

Conflict diagnostics show each path and constraint. The solver never resolves
failure by selecting an unrequested prerelease, weakening integrity, dropping a
dependency requirement, or silently granting authority.

---

**Previous:** [← Add, Remove, and Install](04-add-remove-install.md) · **Next:** [ Lockfile Verification](06-lockfile-verification.md)
