# Package Security

Review source identity, hashes, declared capabilities, native code, transitive dependencies, and update scope. Applications and packages use one `requires` list, while exact resource consent is recorded separately by the trusted runtime tooling.

Reproducible builds, immutable releases, minimal capabilities, audit output, and lock verification reduce—but do not eliminate—supply-chain risk.

Treat a package update as a requester change. The permission fast path is
invalidated when package version, integrity, transitive path, build code, native
artifact, or required scope changes. The trusted CLI shows the exact delta and
never accepts a package-edited manifest as consent.

The current compiler does not expose filesystem or process operations to
packages, and decorators cannot access external host resources. Network,
environment, and secret operations are checked at their runtime boundary.
Package archives are parsed with size/count/path limits. Secrets are redacted
from diagnostics and forbidden from package, lock, cache metadata, and approval
history. Non-interactive runs fail closed when exact consent is missing.

---

**Previous:** [← Native Dependencies](08-native-dependencies.md) · **Next:** [ Tutorials](../10-tutorials/README.md)
