# Version Resolution

Resolution selects one compatible graph from declared constraints, package metadata, target requirements, and existing lock state. Failure reports the constraint chain rather than guessing or silently downgrading.

`zirk update` explicitly recalculates locked versions; ordinary locked builds do not.

---

**Previous:** [← Add, Remove, and Install](./04-add-remove-install.md) · **Next:** [Lockfile Verification →](./06-lockfile-verification.md)
