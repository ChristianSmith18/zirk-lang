# Compile-Time Reflection

Decorators can inspect explicitly authorized declarations and metadata through the Syntax API. Compile-time reflection is scoped to this model; general `comptime {}` is excluded from Zirk 1.x.

Reflection must preserve visibility and cannot read arbitrary project files
without an application `permissions` grant marked `during: build`.

---

**Previous:** [← Syntax API](04-syntax-api.md) · **Next:** [ Runtime Reflection](06-runtime-reflection.md)
