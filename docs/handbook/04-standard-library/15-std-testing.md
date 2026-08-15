# `std.testing`

`@test` is valid only in `.spec.zrk`; `@e2e` belongs in `test/*.e2e.zrk`; `@bench` runs through `zirk bench`. Tests gain no automatic private access or undeclared permissions and are excluded from release binaries.

Assertions cover equality, identity, truth, nullability, results, exceptions, collections, and approximate decimals with values, diffs, and source locations.

---

**Previous:** [← `std.crypto`](./14-std-crypto.md) · **Next:** [`std.reflect` →](./16-std-reflect.md)
