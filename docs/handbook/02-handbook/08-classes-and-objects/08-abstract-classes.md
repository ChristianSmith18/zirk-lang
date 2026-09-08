# Abstract Classes

An `abstract` class defines a nominal requirement set. It may require attributes
and abstract methods, but cannot contain state, layout, a constructor, or a
method body.

Abstract classes cannot be instantiated. A concrete class adopts the
requirements with `implements`, not `extends`.

Use an interface for a pure contract and a trait for reusable behavior that need not establish a single class lineage.

```zirk
abstract class Shape {
    abstract area(): Decimal;
}

class Circle implements Shape {
    radius: Decimal;

    area(): Decimal {
        return 3.14159 * this.radius ** 2;
    }
}
```

`Shape()` is invalid, as are `class Circle extends Shape` and an abstract method
body. A concrete implementer is invalid until it provides every requirement.
Satisfying an abstract requirement replaces no inherited implementation, so it
takes no `#override` marker — writing one there is a warning.

---

**Previous:** [← Inheritance](07-inheritance.md) · **Next:** [ Static Members](09-static-members.md)
