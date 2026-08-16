# Reproducible Builds

A release build should reproduce with the same source, lockfile, compiler/toolchain, flags, target, and external SDK inputs. Cache keys include compiler version, target, dependency API, IR version, and relevant configuration.

Reproducibility does not mean one binary works on every platform; each target remains a distinct controlled build.

---

**Previous:** [← Lockfile](12-lockfile.md) · **Next:** [Permission Approval and Audit →](14-permission-approval-and-audit.md)
