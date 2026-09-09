# Control flow and callables

Use canonical braces and semicolons. Conditions require `Boolean`; there is no
truthiness.

## Select control flow from intent

| Need | Form |
| --- | --- |
| Branch with effects | `if condition { ... } else { ... }` |
| Choose a value | `if condition { value } else { other }` or ternary |
| Exhaustive closed-domain branching | `match value { ... }` |
| Iterate a collection/range | `for item in iterable { ... }` |
| Non-regular counter update | `for mut i = start; test; step { ... }` |
| Condition-controlled repetition | `while condition { ... }` |
| Deliberate endless loop | `loop { ... }` with `break` |

`match` expressions must unify branch types and patterns are guard-free. Use
`break` and `continue` only inside a loop.

## Functions and lambdas

```zirk
fn add(left: Int32, right: Int32): Int32 {
    return left + right;
}

inmut format: Fn(String) => String = (text): String => text.trim();
```

Top-level functions use `fn`; methods in type bodies omit it. Optional
parameters are `name?: T`, variadics are `...values: T`, and named calls use
`label: value`. There is no conventional overloading or implicit partial
application: use different names, unions, or generics.

Choose the callable form deliberately:

- `fn name(...)` is a named top-level function and the normal reusable API.
- `name(...): T { ... }` inside a type is an instance/static method; do not
  write `fn` there.
- `(args): T => expression` or `fn(args): T => expression` is a lambda.
- `Fn(A, B) => R`/`Function(A, B) => R` is a value type for passing a callable.
- `fn gen` is a lazy generator and uses `yield`; read the iteration source
  before promising generator behavior in the current compiler.

Functions do not overload by signature. Use a distinct name, a generic, an
optional/named parameter, or a union and `match` instead. A non-`Void` function
must return a compatible value on every path.

Before writing a closure that captures mutable state, read the source guide on
captures. Whole-reference captures alias while projections become independent
values.
