# Named Arguments

Named arguments use `name: value` to make call-site intent visible.

```zirk
inmut result = open_connection(
    host: "localhost",
    retries: 3,
);
```

Names must match declared parameters, cannot be repeated, and remain subject to type checking. The signature defines whether positional and named forms may be mixed and in which order.

When a visible variable has exactly the parameter name, `name:` abbreviates
`name: name`:

```zirk
inmut url = "https://api.example.com";
inmut timeout = 5s;

client.get(timeout:, url:);
```

The shorthand accepts a simple identifier only and may be mixed with explicit
named arguments:

```zirk
client.get(url:, timeout: 10s, headers:);
```

After the first named argument, abbreviated or explicit, remaining arguments
must also be named. A repeated label, a missing same-named variable or an
expression such as `config.url:` is invalid; write `url: config.url` instead.

Named arguments are valuable for Boolean flags, similar adjacent types, and optional settings. They do not replace a configuration record when many values evolve together.

---

**Previous:** [← Default Parameters](04-default-parameters.md) · **Next:** [ Variadic Functions](06-variadic-functions.md)
