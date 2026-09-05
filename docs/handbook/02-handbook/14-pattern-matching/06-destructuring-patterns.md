# Destructuring Patterns

Direct destructuring supports records and tuples. It binds independent
projected values.

```zirk
inmut { id, name } = user_record;
inmut (status, count) = response_tuple;
```

Inside `match`, tuple patterns select a branch and bind the components:

```zirk
match point {
    (0, 0) => origin();
    (x, 0) => axis_x(x);
    (0, y) => axis_y(y);
    (x, y) => point_at(x, y);
}
```

An algebraic enum is not destructured directly. Its payload is selected and
extracted only inside `match`; a nested record or tuple pattern may then inspect
that payload.

Attribute names and types are checked. Bindings remain local and may be renamed
with `attribute -> local_name`. Rest destructuring is not in the initial model.

Destructure only the data the decision uses. Binding many fields can couple the branch to representation details unnecessarily.

---

**Previous:** [← Union Patterns](05-union-patterns.md) · **Next:** [ match as a Statement](07-match-as-statement.md)
