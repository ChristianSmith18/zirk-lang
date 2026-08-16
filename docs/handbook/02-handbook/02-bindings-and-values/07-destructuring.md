# Destructuring

Destructuring binds parts of a composite value by pattern. It is useful when the structure itself communicates which data is required.

```zirk
inmut { name, version } = project_info;
```

Aliases use the same arrow convention as imports when the local name should differ:

```zirk
inmut { name -> project_name, version } = project_info;
```

The pattern must be compatible with the value's type and cannot silently ignore requirements of an exhaustive context. A misspelled field should produce a type-aware diagnostic rather than create a new binding with `null`.

Use direct property access when only one value is needed; destructuring is most effective when several related fields enter the same local scope.

---

**Previous:** [← Definite Initialization](06-definite-initialization.md) · **Next:** [ Shadowing](08-shadowing.md)
