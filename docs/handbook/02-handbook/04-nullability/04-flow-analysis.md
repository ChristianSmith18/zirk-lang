# Flow Analysis

After code proves a nullable value is non-null, flow analysis narrows it to the underlying type on that path.

```zirk
if user != null {
    stdout.println(user.name);
}
```

The proof has a scope and can be invalidated by mutation or aliasing that changes the value. The compiler must not retain a narrowing when intervening behavior could restore `null`.

Early returns often produce the clearest narrow path:

```zirk
if user == null {
    return;
}
stdout.println(user.name);
```

Matching a union provides the same principle with explicit alternatives and exhaustiveness.

---

**Previous:** [← Null Coalescing](03-null-coalescing.md) · **Next:** [ Common Nullability Errors](05-common-nullability-errors.md)
