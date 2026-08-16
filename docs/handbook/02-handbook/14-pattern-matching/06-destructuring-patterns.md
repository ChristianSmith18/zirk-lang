# Destructuring Patterns

Destructuring patterns match a composite shape and bind selected fields.

```zirk
match event {
    UserCreated({ id, name }) => {
        audit(id, name);
    }
    _ => ignore(event);
}
```

`UserCreated(user)` would bind the complete payload as `user`.
`UserCreated({ id, name })` instead matches that payload and binds only its
selected fields. This resembles the value handed to a JavaScript callback, but
it is a pattern binding: the names exist only in the selected branch and no
function call occurs.

Field names and types are checked against the matched record or enum variant. Bindings remain local to the branch and may be renamed with `field -> local_name`.

Destructure only the data the decision uses. Binding many fields can couple the branch to representation details unnecessarily.

---

**Previous:** [← Union Patterns](05-union-patterns.md) · **Next:** [ match as a Statement](07-match-as-statement.md)
