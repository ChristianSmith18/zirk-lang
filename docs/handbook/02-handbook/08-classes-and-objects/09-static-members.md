# Static Members

Static members belong to the class rather than to one instance. They suit named factories, constants, and behavior whose contract does not need `this`.

`static` applies to methods and fields:

```zirk
class Math {
    static add(a: Int32, b: Int32): Int32 { return a + b; }
}

class Counter {
    static value: Int32 = 42;
}
```

Static members are reached through the type name, not through an instance:

```zirk
stdout.println(Math.add(2, 3));
stdout.println(Counter.value);
```

Accessing a static member through an instance (`counter.value`, `m.x`) is a
compile-time error; the class name keeps ownership visible at the call site.
Static methods never dispatch virtually and a static body cannot reference
`this`. Visibility and `mut`/`inmut` apply to static members exactly as they
do to instance members.

> **Implementation status:** `static` methods and fields on `class` are
> implemented, including `this`-free bodies and class-name access. `static`
> members on `record` and `enum` declarations are specified but not yet
> implemented.

Static mutable state is not a substitute for application globals and remains subject to concurrency safety. Prefer explicit dependencies over hidden process-wide state.

---

**Previous:** [← Abstract Classes](08-abstract-classes.md) · **Next:** [ Object Identity](10-object-identity.md)
