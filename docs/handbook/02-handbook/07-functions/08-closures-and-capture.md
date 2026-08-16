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

Immutable captured values are snapshots. Capturing a complete reference shares
its referent, while capturing an attribute, index, slice, or other projection
makes an independent deep snapshot. If a closure writes a captured binding, the
compiler lifts that binding into one shared cell observed by all closures that
capture it. Closures may escape; the compiler chooses stack, inline, or managed
storage without changing these rules. Concurrent mutable capture can require
synchronization or be rejected. A resource cannot escape its `match with`
lifetime indirectly through a closure.

The compiler chooses closure representation; capture behavior is observable semantics and cannot change because of optimization.

Methods can be stored as callable values together with their bound receiver:

```zirk
mut print: Fn(String) => Void = stdout.println;
print("Hello");
```

`print` remains bound to `stdout`; `mut` permits rebinding the variable.
Assigning it shares callable identity and environment. Calling `clone()` on a
callable instead deep-clones its captured environment when all parts are
cloneable. Callables compare identity with `is` and do not support `==`.

---

**Previous:** [← Lambdas](07-lambdas.md) · **Next:** [ Mutability in Parameters](09-mutability-in-parameters.md)
