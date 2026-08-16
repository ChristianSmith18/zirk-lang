# Visibility

Members can be `public`, `private`, or `protected`. Public is the default.

`private` confines implementation details to their declaration boundary. `protected` exposes extension points to subclasses. `public` contributes to the API that packages and compatibility checks must preserve.

Prefer the narrowest visibility consistent with the abstraction. Publishing a declaration from a file additionally requires `share`; member visibility does not itself export the containing type.

---

**Previous:** [← this](05-this.md) · **Next:** [ Inheritance](07-inheritance.md)
