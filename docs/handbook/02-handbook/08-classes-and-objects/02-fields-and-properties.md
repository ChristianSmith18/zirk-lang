# Attributes and Accessor Methods

Attributes store instance state. Visibility and `mut`/`inmut` determine who can
access or rebind it. Zirk has no `property` declaration.

Class attributes are `public mut` by default. These declarations are equivalent:

```zirk
name: String;
public mut name: String;
```

Write the modifiers when they communicate a non-default guarantee:

```zirk
private mut balance: Float64;
public inmut account_id: UInt64;
```

An attribute without an initializer receives the default value of its declared
type; there is no reserved `default` expression. Validation, computed access,
and storage hiding use ordinary methods named by convention:

```zirk
fn get_balance(): Float64 { return this.balance; }
fn set_balance(value: Float64): Void { ... }
```

They are called with parentheses. `inmut` fixes an attribute binding;
`inmut::strict` is required for transitive immutability.

---

**Previous:** [← Declaring Classes](01-declaring-classes.md) · **Next:** [ Constructors](03-constructors.md)
