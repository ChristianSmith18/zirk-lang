# `inmut`

`inmut` prevents rebinding a name after initialization.

```zirk
inmut service_name: String = "catalog";
```

This is invalid:

```zirk
service_name = "billing";
```

The diagnostic should identify the immutable binding and suggest `mut` only if reassignment is intentional.

`inmut` fixes the reference or value held by the binding; it respects the mutability contract of the referred type. It does not automatically make an entire reachable object graph immutable. Use `inmut::strict` when transitive modification must be forbidden.

Default to `inmut`. Stable names reduce the number of states a reader and compiler must track, while preserving mutation through APIs that explicitly permit it.

---

**Previous:** [← `mut`](./01-mut.md) · **Next:** [Strict Immutability →](./03-strict-immutability.md)
