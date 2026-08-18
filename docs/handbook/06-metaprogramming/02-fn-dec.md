# `fn dec`

A non-repeatable decorator begins with `fn dec`:

```zirk
fn dec Route(path: String) {
    class(target) {
        match target.transform {
            Inspect(context) => { validate_path(path); }
            Augment(builder) => { builder.register_route(path); }
            _ => {}
        }
    }
}
```

External parameters type-check each use. Target blocks introduce `target`; compiler services such as `context` and `builder` exist only when bound by a matched payload.

`repeatable fn dec` groups contiguous applications once:

```zirk
repeatable fn dec Middleware(name: String) {
    function(target) {
        match target.transform {
            Inspect(context, applications) => {
                for application in applications {
                    validate_name(application.arguments.name);
                }
            }
            _ => {}
        }
    }
}
```

`applications` is explicit, immutable, typed, and source-ordered. The `for` introduces `application`; concrete configuration is read through `application.arguments`. The decorator decides duplicate and cardinality policy. Separated applications are invalid.

Optional `requires decorators [...]`, `before decorators [...]`, and `after decorators [...]` clauses validate dependencies between different decorators. They never reorder source. Self-reference and direct or indirect cycles are compile-time errors.

---

**Previous:** [← Decorators](01-decorators.md) · **Next:** [ Decorator Targets](03-decorator-targets.md)
