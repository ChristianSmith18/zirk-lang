# Fields and Properties

Fields store instance state. Visibility and `mut`/`inmut` determine who can access or rebind it.

Class fields are `public mut` by default. These declarations are equivalent:

```zirk
name: String;
public mut name: String;
```

Write the modifiers when they communicate a non-default guarantee:

```zirk
private mut balance: Float64;
public inmut account_id: UInt64;
```

Expose state directly only when its invariants survive every permitted assignment. Properties or methods should validate changes, represent computed values, or hide storage. `inmut` fixes a field binding; `inmut::strict` is required for transitive immutability.

---

**Previous:** [← Declaring Classes](01-declaring-classes.md) · **Next:** [ Constructors](03-constructors.md)
