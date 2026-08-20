# Package Anatomy

A `.zpkg` contains manifest, `public.api`, `portable.ir`, README, license, and documentation. It excludes private source details not required by the contract and never embeds secrets.

The manifest records identity, version, dependencies, requirements, targets, and native artifacts.

`public.api` is the compatibility surface; `portable.ir` is the typed
implementation consumed by the final application build. Documentation and
license are first-class package entries so registries and offline tooling do
not need hidden source access.

Every entry is covered by the package integrity manifest. Paths are normalized,
duplicate/case-conflicting names are rejected, and extraction cannot escape its
destination. Generated files must declare their origin. Credentials, signed
permission approvals, local caches, target build directories, and developer
absolute paths are forbidden package content.

---

**Previous:** [← Packages](README.md) · **Next:** [ Public API](02-public-api.md)
