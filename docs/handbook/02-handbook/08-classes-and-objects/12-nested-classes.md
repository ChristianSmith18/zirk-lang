# Nested, Inner, and Local Classes

A class body may contain other class declarations, and a function, method, or
constructor body may declare a local class. Zirk 1.x has no anonymous classes:
a local class or a closure covers that use.

## Static nested classes

`class Nested {}` inside a class body is a static nested class. It is pure
namespacing: the canonical name is `Outer.Nested`, it has no access to the
enclosing instance, and it carries no implicit reference to it.

```zirk
class Outer {
    class Nested {
        value: Int32;
    }
}

inmut n = Outer.Nested();
```

Nested classes may themselves contain nested classes to arbitrary depth, and
member visibility modifiers apply to the nested class name.

## `inner` classes

`inner class Inner {}` captures the enclosing instance. The layout gains a
hidden `outer` field and every constructor gains a hidden leading parameter.
Inner methods may reference `outer` and access `private`/`protected` members of
the enclosing class; the outer class may likewise access private members of its
inner classes.

```zirk
class Outer {
    private label: String = "outer";

    inner class Inner {
        describe(): String {
            return outer.label;
        }
    }
}
```

Construction requires an enclosing instance: inside `Outer`, `Inner()` binds
`this`; outside, `o.Inner()` binds `o`. An `inner` class may not declare
`static` members — it carries no enclosing-independent state.

## Local classes

`class` is a valid statement inside any function, method, or constructor body.
A local class is visible only inside its block, receives a mangled canonical
name, and obeys all ordinary class rules (`extends`, `implements`,
`#override`, `final`). Local classes do not capture enclosing local variables
in Zirk 1.x — pass state through fields instead.

```zirk
fn make_nameable(): Nameable {
    class Tmp implements Nameable {
        name(): String { return "t"; }
    }
    return Tmp();
}
```

Anonymous-class syntax such as `Nameable() { ... }` is rejected; the diagnostic
points to a local class for named multi-method implementations and to a closure
for single-method contracts.

---

**Previous:** [← Cloning](11-cloning.md) · **Next:** [Interfaces and Traits →](../09-interfaces-and-traits/README.md)
