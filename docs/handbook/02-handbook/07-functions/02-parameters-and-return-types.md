# Parameters and Return Types

Each parameter associates a local name with an accepted type. The return annotation constrains every reachable `return` and value-producing path.

```zirk
fn describe(id: UInt64, active: Boolean): String {
    return active ? "active:{id}" : "inactive:{id}";
}
```

Arguments do not undergo silent lossy conversion. Returning `null` from `String` is invalid; use `String?` only when absence belongs to the contract.

Parameter bindings are local to the function. Mutation of a binding, mutation of an object, and transfer of a resource are separate questions governed by the parameter and type contracts.

## Inferring the return type

A function or method **with a body** may omit the `: Type` annotation. The checker reads the body's `return`s and applies the same unification rule used for `if`-expression branches:

```zirk
fn double(n: Int32) {
    return n * 2; // inferred: Int32
}

fn main() {
    stdout.println(double(21)); // no `return` at all: inferred Void
}
```

The rules:

- Every `return expr;` in the body must agree on one type; a mix like `Int32` and `String` is rejected.
- A bare `return;` does not participate in unification, but it must still fit the inferred type — `return;` in a body that infers `Int32` is an error, exactly as it is under an explicit annotation.
- A body that never returns a value infers `Void`; one where every path diverges infers `Never`.
- The explicit annotation stays a contract when written: the inferred type and the annotation must match.
- Inference cannot resolve a recursion cycle on its own — `fn a() { return a(); }` has no anchor. Annotate at least one member of the cycle.
- Signatures without a body still require the annotation: `interface` and `abstract` members, `extern` declarations and `fn dec` have nothing to infer from.
- The entrypoint rule is unchanged: `fn main() { }` infers `Void` and is valid; a `main` whose body returns a value is not.

Tooling sees the inferred type: `zirk check --types` reports each declaration with `"inferred": true`, and the editor hover shows the resolved signature either way.

---

**Previous:** [← Declaring Functions](01-declaring-functions.md) · **Next:** [ Optional Parameters](03-optional-parameters.md)
