# `fn dec`

A native decorator begins with `fn dec`:

```zirk
fn dec route(path: String) {
    class(target) { /* class transformation */ }
    method(target) { /* method transformation */ }
}
```

External parameters configure use; target blocks receive fixed typed contextual APIs. A decorator is compile-time behavior, not an ordinary runtime function.

---

**Previous:** [← Decorators](01-decorators.md) · **Next:** [ Decorator Targets](03-decorator-targets.md)
