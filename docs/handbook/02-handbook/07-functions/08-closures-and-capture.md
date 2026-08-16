# Closures and Capture

A lambda that refers to surrounding bindings is a closure. Capture must preserve mutability, lifetime, concurrency, and resource rules.

```zirk
inmut prefix = "item";
inmut format = (id: UInt64): String => "{prefix}:{id}";
```

When a lambda declares a local with the same name as a captured binding, the
unqualified name denotes the lambda-local value and `this.` selects the capture:

```zirk
inmut prefix = "outer";
inmut format = (prefix: String): String => "{this.prefix}:{prefix}";
```

Here `this.prefix` is the captured outer binding and `prefix` is the lambda
parameter. `this.` is needed only for this collision; an unambiguous capture is
read directly by name. Within a class lambda, member resolution remains tied to
the surrounding instance and the compiler diagnoses any ambiguous use.

Capturing stable immutable data is straightforward. Capturing mutable state into concurrent work can require synchronization or be rejected when safety cannot be proven. A resource cannot escape its `match with` lifetime indirectly through a closure.

The compiler chooses closure representation; capture behavior is observable semantics and cannot change because of optimization.

Methods can be cloned into a local callable value together with their bound
receiver:

```zirk
inmut print = clone(stdout.println);
print("Hello");
```

`print` retains the same parameter, return, error, effect, and permission
contract as `stdout.println`, and it remains bound to `stdout`. `clone` here
copies the callable value; it does not clone the stream object or its external
handle. A method that cannot form a safe callable value is diagnosed rather
than partially copied.

---

**Previous:** [← Lambdas](07-lambdas.md) · **Next:** [ Mutability in Parameters](09-mutability-in-parameters.md)
