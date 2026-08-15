# Declaring Classes

Use `class` to define identity-bearing state and behavior.

```zirk
class User {
    public inmut id: UInt64;
    public mut name: String;
}
```

Fields carry their own binding mutability. Classes are inheritable by default in Zirk 1.x; use composition where a relationship is not truly substitutable.

---

**Previous:** [← Classes and Objects](./README.md) · **Next:** [Fields and Properties →](./02-fields-and-properties.md)
