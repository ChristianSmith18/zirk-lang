# Variadic Functions

The `...values` form accepts a variable number of arguments through one typed parameter.

```zirk
fn sum(...values: Int32): Int32 {
    return values.reduce(0, (total, value) => total + value);
}
```

Every supplied argument must satisfy the element type. Variadic input is a language-level collection contract, not an unsafe view of a native call stack.

Inside the function, `values` is an ordered, read-only iterable of `Int32`
values. It supports `for ... in` and the ordinary `Iterable<T>` operations:

```zirk
fn print_all(...values: String): Void {
    for value in values {
        stdout.println(value);
    }
}

print_all("one", "two", "three");
```

The iterable exists for the duration of the call. Convert it explicitly when a
stored `List<T>` is required.

Use it for homogeneous repeated values. Prefer a list parameter when callers already hold a collection or when collection ownership and laziness matter.

---

**Previous:** [← Named Arguments](05-named-arguments.md) · **Next:** [ Lambdas](07-lambdas.md)
