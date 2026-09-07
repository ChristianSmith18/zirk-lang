# Inheritance

A class may extend one class and implement multiple interfaces or traits. Multiple class inheritance is excluded from Zirk 1.x.

Inheritance should preserve substitutability: callers using the base contract must remain correct for the subtype. Reuse alone is better served by composition or traits.

Classes are inheritable by default. Declaring `final class` seals a class against `extends` (it may still implement contracts and be instantiated), and a `final` method cannot be overridden. `final` combined with `abstract` on the same class is contradictory and rejected; `final` never applies to fields, constructors, interfaces, traits, or records.

```zirk
class User {
    name: String;

    construct(name: String) {
        this.name = name;
    }

    label(): String {
        return this.name;
    }
}

class Admin extends User {
    permissions: List<String>;

    #override
    label(): String {
        return "Admin: {this.name}";
    }
}

inmut user: User = Admin("Cristian");
stdout.println(user.label()); // Dynamic dispatch calls `Admin.label`.
```

Inherited attributes keep their visibility. Public and protected instance
methods dispatch virtually by default; private and static methods do not.
Replacing an inherited concrete implementation requires the `#override` member
marker, written on its own line above the method; the method must preserve
parameter types exactly and may narrow its result covariantly. `#override`
applies only when an inherited implementation is actually replaced: satisfying
an `abstract class` or `interface` requirement takes no marker (writing one is
a warning), and a `#override` that overrides nothing is an error. The marker
never applies to fields, constructors, nested classes, or `static` members. Use
`super(...)` for base construction and `super.method()` for inherited behavior.

---

**Previous:** [← Visibility](06-visibility.md) · **Next:** [ Abstract Classes](08-abstract-classes.md)
