# Source Files

Zirk source code lives in UTF-8 text files with the `.zrk` extension. A file participates in a project described by `init.zrk`; it is not an independently interpreted script.

```text
catalog/
├── init.zrk
└── src/
    ├── main.zrk
    └── product.zrk
```

Declarations belong to their file unless published with `share`. Consumers import published names explicitly. This boundary makes public API review possible without treating every declaration in a directory as globally visible.

```zirk
// product.zrk
share record Product {
    id: UInt64;
    name: String;
}
```

Filename conventions use `snake_case`. Case matters: `product.zrk` and `Product.zrk` are distinct names even on a filesystem that happens to compare them loosely. Avoid relying on platform-specific casing behavior.

`init.zrk` is a typed project manifest with a reserved role; ordinary application declarations belong under the configured source tree. The entry path identifies the file containing `main`.

---

**Previous:** [← Program Structure](./README.md) · **Next:** [Lexical Rules →](./02-lexical-rules.md)
