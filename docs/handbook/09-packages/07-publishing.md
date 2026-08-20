# Publishing

`zirk prepare` audits public API, documentation, license, permissions, targets, native artifacts, and reproducibility before `zirk package` or `zirk publish`.

Published versions are immutable. A release must not contain credentials, undeclared generated files, incompatible IR, or undocumented capability requirements.

The publish workflow is `prepare` → review → `package` → verify archive →
`publish`. Preparation runs tests/documentation policy, public API compatibility,
license and README checks, target/native validation, permission requirement
audit, secret/path scan, and reproducibility checks.

Publishing signs or authenticates the upload without storing credentials in
the project. Registries reject an existing name/version with different bytes.
A compromised release is yanked with an auditable reason; its immutable record
is not silently replaced.

---

**Previous:** [← Lockfile Verification](06-lockfile-verification.md) · **Next:** [ Native Dependencies](08-native-dependencies.md)
