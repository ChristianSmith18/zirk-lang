# Lambdas

A lambda creates a callable value without a declaration name.

```zirk
inmut add = (a: Int32, b: Int32): Int32 => a + b;
inmut subtract = fn(a: Int32, b: Int32): Int32 => a - b;
inmut log = (message: String): Void => {
    stdout.println(message);
};
```

The leading `fn` is optional for a function expression. Both forms create the
same kind of callable value; use `fn` when it makes a dense expression easier
to recognize as a function.

Expression and block bodies follow the same parameter and result typing as declared functions. Context may infer parts of a lambda only when the answer is unambiguous.

An expected `Fn` type can infer parameter and result types. A recursive lambda
must give its binding an explicit `Fn(...) => R` type so the binding is known
while its body is checked. Lambdas may escape through arguments, returns, and
attributes; closure lifetime is automatic.

Use lambdas for short behavior passed to collection, scheduling, or callback APIs. Name a function when the behavior has an independent contract worth documenting or testing.

---

**Previous:** [← Variadic Functions](06-variadic-functions.md) · **Next:** [ Closures and Capture](08-closures-and-capture.md)
