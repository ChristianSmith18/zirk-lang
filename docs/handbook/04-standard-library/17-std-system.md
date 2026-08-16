# `std.system`

`std.system` exposes portable process, target, and signal information without stabilizing runtime internals. Environment values, signals, and sensitive data require their declared capabilities.

`exit(code)` terminates immediately and is reserved for boundaries. Returning from `main` permits structured task completion, resource closure, stream flush, and orderly shutdown.

---

**Previous:** [← std.reflect](16-std-reflect.md) · **Next:** [ Native and Low-Level Programming](../05-native-and-low-level/README.md)
