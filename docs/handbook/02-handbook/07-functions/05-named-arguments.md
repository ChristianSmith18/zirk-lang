# Named Arguments

Named arguments use `name: value` to make call-site intent visible.

```zirk
inmut result = open_connection(
    host: "localhost",
    retries: 3,
);
```

Names must match declared parameters, cannot be repeated, and remain subject to type checking. The signature defines whether positional and named forms may be mixed and in which order.

Named arguments are valuable for Boolean flags, similar adjacent types, and optional settings. They do not replace a configuration record when many values evolve together.

---

**Previous:** [← Default Parameters](04-default-parameters.md) · **Next:** [ Variadic Functions](06-variadic-functions.md)
