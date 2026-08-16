# Inheritance

A class may extend one class and implement multiple interfaces or traits. Multiple class inheritance is excluded from Zirk 1.x.

Inheritance should preserve substitutability: callers using the base contract must remain correct for the subtype. Reuse alone is better served by composition or traits.

Classes are inheritable by default. Zirk 1.x does not define `final`; API authors should rely on visibility and contract design rather than a nonexistent sealing keyword.

```zirk
class User {
    name: String;

    construct(name: String) {
        this.name = name;
    }

    override fn label(): String {
        return this.name;
    }
}

class Admin extends User {
    permissions: List<String>;

    fn label(): String {
        return "Admin: {this.name}";
    }
}

inmut user: User = Admin("Cristian");
stdout.println(user.label()); // Dynamic dispatch calls `Admin.label`.
```

Inherited attributes keep their visibility. Public and protected instance
methods dispatch virtually by default; private and static methods do not. A
redefinition must use `override fn`, preserve parameter types exactly, and may
narrow its result covariantly. Use `super(...)` for base construction and
`super.method()` for inherited behavior.

---

**Previous:** [← Visibility](06-visibility.md) · **Next:** [ Abstract Classes](08-abstract-classes.md)
