# Decorator Targets

Each target block declares which syntax category the decorator supports, such as class or method. Applying it elsewhere is a compile-time error at the use site.

One decorator may support several targets, but each receives only operations valid for that category. This prevents unchecked casts into compiler internals.

---

**Previous:** [← fn dec](02-fn-dec.md) · **Next:** [ Syntax API](04-syntax-api.md)
