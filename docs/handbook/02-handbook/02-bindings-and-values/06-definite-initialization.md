# Definite Initialization

Flow analysis prevents a binding from being read along a path where no value is available.

```zirk
mut message: String;

if ready {
    message = "ready";
} else {
    message = "waiting";
}

stdout.println(message);
```

Both branches assign `message`, so the final read is safe. Removing the `else` makes the read invalid because `ready` may be false.

The diagnostic should identify the uninitialized path. Correct the control flow, provide an initializer, return from the missing path, or model absence with `String?`; do not suppress the check.

Definite initialization composes with `Never`: a branch that cannot return does not need to assign a value used afterward.

---

**Previous:** [← Default Values](./05-default-values.md) · **Next:** [Destructuring →](./07-destructuring.md)
