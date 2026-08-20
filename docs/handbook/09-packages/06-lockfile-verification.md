# Lockfile Verification

The lockfile pins versions and hashes. Installation verifies fetched content and rejects substitution, corruption, or mismatched metadata with the affected package identified.

The lockfile is not a secret store and cannot authorize permissions absent from `init.zrk`.

Each node records exact version, source/registry identity, content integrity,
dependency edges, and relevant native/IR metadata. Canonical serialization
keeps diffs deterministic. A project move does not rewrite dependency identity,
but it invalidates the separate location-bound permission approval.

Offline builds may use already verified cached bytes. A cache hit still checks
the lock integrity and package structure. `zirk update` is the only ordinary
operation that changes selected versions; `build`, `check`, and `test` do not.

---

**Previous:** [← Version Resolution](05-version-resolution.md) · **Next:** [ Publishing](07-publishing.md)
