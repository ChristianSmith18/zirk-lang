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
private mut balance: Float;
public inmut account_id: UInt64;
```

An attribute may declare an initializer with `field: Type = expr;`:

```zirk
class Box {
    width: Int32 = 10;
    height: Int32 = 20;

    construct() {}
}
```

Initializers run in declaration order before the constructor body. An
initializer cannot reference `this`, and for inherited fields the `super()`
call still comes first. A constructor that assigns the field wins over the
initializer:

```zirk
construct(w: Int32) { this.width = w; } // height keeps 20
```

An attribute without an initializer receives the default value of its declared
type; there is no reserved `default` expression. Validation, computed access,
and storage hiding use ordinary methods named by convention:

```zirk
fn get_balance(): Float { return this.balance; }
fn set_balance(value: Float): Void { ... }
```

They are called with parentheses. `inmut` fixes an attribute binding;
`inmut::strict` is required for transitive immutability.

---

**Previous:** [← Declaring Classes](01-declaring-classes.md) · **Next:** [ Constructors](03-constructors.md)
