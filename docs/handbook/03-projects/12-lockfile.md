# Lockfile

`zirk.lock` records exact versions and hashes. Locked builds use those values exactly; `zirk update` is the explicit operation that recalculates resolution.

Commit application lockfiles. Never store secrets in them. Verification failure must identify the affected package and refuse substituted content.

---

**Previous:** [← Dependencies](11-dependencies.md) · **Next:** [ Reproducible Builds](13-reproducible-builds.md)
