# Default Values

Every Zirk type defines a default value, but the existence of a default does not authorize reading a local before flow analysis knows it is available.

Defaults are useful for storage initialization, generated layouts, and APIs that explicitly request them. They are not a substitute for domain modeling: use `T?` to represent meaningful absence and `Result<T, E>` to represent expected failure.

```zirk
mut retries: Int32 = 0;
mut selected: String? = null;
```

Prefer explicit initializers in ordinary source because they reveal intent. A generic API that obtains `T`'s default must still preserve the type's invariants.

Do not assume that the default for every reference-like type is `null`; `null` inhabits only nullable types.

---

**Previous:** [← Type Inference](./04-type-inference.md) · **Next:** [Definite Initialization →](./06-definite-initialization.md)
