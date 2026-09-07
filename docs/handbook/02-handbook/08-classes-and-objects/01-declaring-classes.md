# Declaring Classes

Use `class` to define identity-bearing state and behavior.

```zirk
class User {
    public inmut id: UInt64;
    public mut name: String;
}
```

Fields carry their own binding mutability. Classes are inheritable by default in Zirk 1.x; declare `final class` to seal a class against `extends`, and use composition where a relationship is not truly substitutable. Methods are declared without `fn`, directly as `name(params): Return { ... }`.

## API

Reference type with observable identity (`is`), mutable state by field
contract, lifecycle, single-class inheritance plus contracts, and polymorphic
dispatch (delivered by `fase-3-abstract-dispatch`).

### Members

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `type` | `Type` | Universal member | specified |
| declared fields | declared types | Field contract decides mutability | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Session(...)` | `Session` | Construction through the declared `construct` signature | implemented |
| `obj.clone()` | `Session` | Deep clone; compiler-derived when every field is `Clone`, preserves internal sharing and cycles via a runtime memoization map; rejected naming the offending field when the graph reaches `Pointer`/`Resource`/non-`Clone` | implemented |
| `obj.to_string()` | `String` | Universal member | implemented |
| `obj is other` | `Boolean` | Reference identity | implemented |
| user methods / `abstract` members | varies | Dynamic dispatch through `abstract class` | implemented |

Assignment shares the reference; reads through attribute/index/slice/
destructuring/argument/return/closure capture are independent projections
requiring `Clone` when reference-backed.

### Examples

```zirk
class Session { token: String; active: Boolean; }

mut a = Session(token: "t", active: true);
mut b = a;                 // shares
a is b;                    // true
mut c = a.clone();         // deep independent graph
a is c;                    // false
```

---

**Previous:** [← Classes and Objects](README.md) · **Next:** [Attributes and Accessor Methods →](02-fields-and-properties.md)
