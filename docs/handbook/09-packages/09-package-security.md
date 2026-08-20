# Package Security

Review source identity, hashes, permissions, compile-time access, native code, transitive dependencies, and update scope. Runtime and compile permissions are separate attack surfaces.

Reproducible builds, immutable releases, minimal capabilities, audit output, and lock verification reduce—but do not eliminate—supply-chain risk.

Treat a package update as a requester change. The permission fast path is
invalidated when package version, integrity, transitive path, build code, native
artifact, or required scope changes. The trusted CLI shows the exact delta and
never accepts a package-edited manifest as consent.

Build scripts/decorators run only after the build-phase authority sweep. Package
archives are parsed with size/count/path limits. Secrets are redacted from
diagnostics and forbidden from package, lock, cache metadata, and approval
history. CI uses preapproved exact policy and never prompts.

---

**Previous:** [← Native Dependencies](08-native-dependencies.md) · **Next:** [ Tutorials](../10-tutorials/README.md)
