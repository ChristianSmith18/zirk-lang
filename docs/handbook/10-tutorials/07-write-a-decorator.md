# Write a Decorator

This tutorial builds validation, wrapping, and a static framework registry. Every compiler value is introduced explicitly.

## Validate an attribute

```zirk
fn dec NonEmpty() {
    attribute(target) {
        match target.transform {
            Inspect(context) => {
                if !target.type.implements(Length) {
                    target.error("@NonEmpty requires a value with length.");
                }
            }
            Augment(builder) => {
                builder.add_validator((value): Result(Void, ValidationError) => {
                    if value.length == 0 {
                        return Error(ValidationError("must not be empty"));
                    }
                    return Ok();
                });
            }
            _ => {}
        }
    }
}
```

`target` comes from `attribute(target)`, `context` from `Inspect`, and `builder` from `Augment`. Inspection commits nothing if it fails. Generated validators re-enter ordinary typing and safety analysis.

## Wrap a callable

```zirk
fn dec Logged() {
    function(target) {
        match target.transform {
            Wrap(wrapper) => {
                match target.wrap {
                    Before() => { logger.info("starting " + target.name); }
                    After(result) => { logger.info("completed " + target.name); }
                    Catch(error) => { logger.error(error.to_string()); throw error; }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
```

The wrapper preserves the callable signature and makes any logger permission or failure visible. Use `After(result, transform)` only when replacement is required; its `transform` capability is compatible, non-escaping, and one-shot. Use exclusive `Around(next)` when one decorator must own the complete invocation.

## Group repeated middleware

```zirk
repeatable fn dec Middleware(name: String) {
    function(target) {
        match target.transform {
            Inspect(context, applications) => {
                for application in applications {
                    validate_name(application.arguments.name);
                }
            }
            Wrap(wrapper, applications) => {
                for application in applications {
                    wrapper.add(application.arguments.name);
                }
            }
            _ => {}
        }
    }
}
```

Contiguous applications arrive once in source order. Duplicate and count policy belongs here, not in a self-referential `requires` clause.

## Generate a framework registry

```zirk
fn dec Get(path: String) {
    method(target) {
        match target.transform {
            Inspect(context) => { validate_handler(target); }
            Augment(builder) => {
                builder.add_registry_entry(
                    RouteDescriptor(HttpMethod.Get, path, target.callable),
                );
            }
            _ => {}
        }
    }
}
```

The resulting `RouteDescriptor` is an ordinary typed runtime value. `@Get` itself is erased; no runtime annotation scan occurs. Class decorators can generate module/controller/component registrations, attribute decorators validation/serialization, and parameter decorators path/body/injection binding. This supports NestJS-, Spring-, FastAPI-, and Angular-like frameworks without module or constructor targets.

## Test the contract

Test a valid expansion, every unsupported target, malformed configuration, source-order dependency, missing requirement, direct and indirect cycle, duplicate policy, generated public conflict, generic declaration, override without reapplication, source-mapped diagnostic, build permission denial, cache invalidation, and absence of retained decorator metadata.

---

**Previous:** [← Call a C Library](06-call-a-c-library.md) · **Next:** [ Reference](../11-reference/README.md)
