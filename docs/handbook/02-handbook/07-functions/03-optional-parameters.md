# Optional Parameters

Appending `?` to a parameter name makes the argument optional; appending `?` to its type makes the supplied value nullable. These are different contracts.

```zirk
fn greet(name: String, prefix?: String): String { /* ... */ }
```

The call may omit `prefix`, but whenever it is supplied it must be a `String`.
An optional parameter still requires a type annotation. Optional positional
parameters follow every required positional parameter so omission cannot shift
the meaning of later arguments.

Optionality and nullability are independent:

```zirk
fn locate(query: String, region?: String?): Result<Location, SearchError> {
    // `region` may be omitted; when present, its value may also be `null`.
}
```

Named calls make omission explicit: `greet(name: "Ada")`.

Avoid optional parameters when omission changes the operation into a different conceptual function; a distinct name is clearer.

---

**Previous:** [← Parameters and Return Types](02-parameters-and-return-types.md) · **Next:** [ Default Parameters](04-default-parameters.md)
