# Abstract Classes

An `abstract` class defines shared state or behavior while requiring subclasses to complete selected methods.

Abstract classes cannot be instantiated until a concrete subtype satisfies every required member. They are useful when variants share identity and implementation, not merely a behavior signature.

Use an interface for a pure contract and a trait for reusable behavior that need not establish a single class lineage.

```zirk
abstract class Shape {
    color: String;

    abstract fn area(): Decimal64;

    fn describe(): String {
        return "{this.color} shape with area {this.area()}";
    }
}

class Circle extends Shape {
    radius: Decimal64;

    fn area(): Decimal64 {
        return 3.14159 * this.radius ** 2;
    }
}
```

`Shape()` is invalid because an abstract class is not complete. A concrete
subclass is also invalid until it implements every inherited abstract method
with a compatible signature.

---

**Previous:** [← Inheritance](./07-inheritance.md) · **Next:** [Static Members →](./09-static-members.md)
