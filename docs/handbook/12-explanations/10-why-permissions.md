# Why Permissions?

Types cannot reveal every filesystem, network, process, environment, native, or compile-time effect. Finite manifest capabilities make those boundaries reviewable before build and deployment.

Libraries request; applications grant. Compile-time permissions are separate because a safe runtime package can still execute risky build logic.

---

**Previous:** [← Why the C ABI?](./09-why-c-abi.md) · **Next:** [Why Portable IR? →](./11-why-portable-ir.md)
