# Why Portable IR?

Portable typed IR lets packages preserve generics, safety facts, and optimization opportunities until the consuming application selects a target. All code can then align to one ABI and architecture.

The trade-off is IR-version compatibility and a larger trusted compiler surface, addressed through explicit versions, hashes, validation, and reproducible builds.

---

**Previous:** [← Why Permissions?](10-why-permissions.md) · **Next:** [ Class or Record?](12-class-or-record.md)
