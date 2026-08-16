# Object Identity

`is` asks whether two references denote the same observable instance. It is available only for types whose identity is meaningful.

```zirk
if cached_user is requested_user {
    reuse_session();
}
```

Identity is not structural equality. Two records may hold equal fields while remaining distinct values; immutable value-like types may have no observable identity at all.

Do not use `is` as a performance shortcut for `==` unless the type contract explicitly supports that reasoning. Compiler representation changes must not alter observable identity semantics.

---

**Previous:** [← Structural Equality](03-structural-equality.md) · **Next:** [ Logical Operators](05-logical-operators.md)
