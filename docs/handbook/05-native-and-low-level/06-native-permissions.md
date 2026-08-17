# Native Permissions

Native libraries can perform effects invisible to ordinary type analysis. Manifests therefore declare native artifacts, target restrictions, and required capabilities. `unsafe` never grants filesystem, network, process, or environment access.

`zirk prepare` audits the boundary; `zirk build` remains strict and noninteractive.

`commit` acknowledges irreversibility; it is not authorization. Filesystem,
network, process, shell, environment, secret, and native scopes are checked
before the effect even when the call occurs inside unsafe code.

---

**Previous:** [← Name Mangling](05-name-mangling.md) · **Next:** [ Intrinsics](07-intrinsics.md)
