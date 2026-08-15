# `this`

`this` denotes the current instance inside instance members.

Inside a lambda, `this.name` can also disambiguate a captured outer binding from
a lambda-local parameter or binding with the same name. When the lambda is
created inside an instance member, ordinary member access continues to resolve
against the surrounding instance; ambiguous capture/member cases require an
explicitly qualified name.

```zirk
fn rename(name: String): Void {
    this.name = name;
}
```

Qualification distinguishes a field from a same-named parameter. `this` is unavailable in static members and cannot escape a resource or concurrency boundary that would violate safety.

Zirk uses `this`, not `self`.

---

**Previous:** [← Instantiation](./04-instantiation.md) · **Next:** [Visibility →](./06-visibility.md)
