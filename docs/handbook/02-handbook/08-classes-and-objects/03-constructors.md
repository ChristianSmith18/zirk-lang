# Constructors

The constructor is named `construct` and establishes a valid instance.

```zirk
construct(id: UInt64, name: String) {
    this.id = id;
    this.name = name;
}
```

Every required field must be available before construction completes. Validation that can fail should use an explicit factory returning `Result` when constructor failure cannot be expressed safely.

Zirk has no `new` keyword and does not use destructors as its general resource-cleanup model.

## Promoted constructor parameters

Member modifiers on a class constructor parameter declare the field and assign
the argument before the constructor body runs. This short form is equivalent
to the explicit long form:

```zirk
class User {
    construct(public name: String, private inmut id: UInt64) { }
}
```

Use promotion for data carried only by construction. Accessing a promoted field
from another method emits a legibility warning; declare the field explicitly
when it is a broader part of the class API. Promotion requires one constructor
and cannot be used for a variadic parameter.

A class with no `construct` receives an implicit `construct() {}`. It is an
implementation convenience, not a source member: outlines and formatting keep
the class as written. An implicit subclass constructor calls an accessible
zero-argument base constructor and warns to make that `super()` explicit.

A class may declare multiple `construct` members when their parameter lists are
not identical. Resolution considers arity, parameter types, and named
arguments:

```zirk
class User {
    id: UInt64;
    name: String;

    construct(id: UInt64, name: String) {
        this.id = id;
        this.name = name;
    }

    construct(name: String) {
        this.id = generate_user_id();
        this.name = name;
    }
}
```

Two constructors with the same effective parameter signature are invalid.
Constructor selection must remain unambiguous after optional and named
arguments are considered. This constructor-specific facility does not enable
general function overloading.

Field initializers declared with `field: Type = expr;` run in declaration
order before the constructor body, and a `super()` call always precedes both
when the class extends a base. A field the constructor assigns uses that
assignment; the initializer only fills fields the constructor leaves
untouched.

---

**Previous:** [← Attributes and Accessor Methods](02-fields-and-properties.md) · **Next:** [Instantiation →](04-instantiation.md)
