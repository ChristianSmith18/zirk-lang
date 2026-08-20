# Zirk Decorator Semantics

Status: normative design source for Zirk 1.x. This document defines the language contract that compiler, tooling, framework, and handbook implementations must follow. Decorators are not implemented by the current compiler phase unless the feature-status documentation says otherwise.

## 1. Model

A decorator is typed compile-time behavior declared with `fn dec`. It inspects a supported declaration and may add validated syntax or wrap executable behavior. It is not a runtime function value, cannot be assigned to a variable, and does not survive compilation as an object or annotation.

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
                builder.add_validator((value): Result<Void, ValidationError> => {
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

Expansion results re-enter name resolution, type checking, flow/effect analysis, safety validation, interface emission, and code generation. A decorator cannot bypass those stages or access the compiler's private AST. Zirk 1.x has no general `comptime {}` block.

## 2. Declaration and application

`fn dec Name(parameters) { target-blocks }` declares a non-repeatable decorator. `repeatable fn dec Name(parameters) { target-blocks }` declares a decorator whose contiguous applications are processed as one ordered group.

```zirk
@NonEmpty()
mut display_name: String;

@Route("/users")
fn list_users(): Result<List<User>, HttpError> {
    // ...
}
```

Parameters describe the typed configuration of each application. Effects used while evaluating arguments are build effects and follow section 13.

## 3. Supported targets

Zirk 1.x has exactly five decorator targets:

| Target | Applies to | Important boundary |
| --- | --- | --- |
| `class` | Concrete and abstract class declarations | Module, controller, service, component, and entity remain framework roles expressed by class decorators. |
| `attribute` | Stored attributes, including record and value-class storage | Zirk has no property/accessor target; controlled access uses ordinary `get_` and `set_` methods. |
| `function` | Free functions | May inspect, augment compatible API, or wrap execution. |
| `method` | Concrete methods and bodyless abstract/interface/trait signatures | A bodyless signature can be inspected but not wrapped. |
| `parameter` | Function, method, lambda, and initialization parameters | This expresses initialization injection and parameter binding. |

There are no `module`, `abstract_class`, `interface`, `trait`, `record`, `value_class`, `enum`, `enum_case`, `property`, `accessor`, or `construct` targets. Meaningful member behavior uses the five targets. Class decorators may generate typed factories.

## 4. Expansion phases

Each target block receives a typed immutable `target`. Compiler services arrive only through explicit variant payloads:

```zirk
match target.transform {
    Inspect(context) => { /* validate and diagnose */ }
    Augment(builder) => { /* add compatible typed API */ }
    Wrap(wrapper) => { /* wrap executable behavior */ }
}
```

The global order is `Inspect -> Augment -> Wrap -> normal compiler validation`. If inspection emits an error, no later change from that application is committed. `Inspect` cannot mutate; `Augment` uses target-specific builders; `Wrap` exists only for executable targets with bodies.

No compiler-provided binding is implicit. `context`, `builder`, `wrapper`, and repeatable `applications` exist only where named in a matched payload.

## 5. Wrapper phases

Within `Wrap`, behavior is selected through the normal match syntax:

```zirk
Wrap(wrapper) => {
    match target.wrap {
        Before() => { audit.start(target.name); }
        After(result) => { audit.success(target.name); }
        Catch(error) => { audit.failure(target.name, error); throw error; }
        _ => {}
    }
}
```

- `Before()` runs before the target.
- `After(result)` observes a non-`Void` result without replacing it; `Void` uses `After()`.
- `After(result, transform)` exposes a non-escaping, one-shot replacement capability. `transform(value)` requires a compatible type.
- `Catch(error)` may rethrow, throw another visible error, or recover with a compatible value. Unhandled errors remain visible.
- `Around(next)` controls the whole invocation and cannot coexist ambiguously with the other wrapper forms.

Variant arity is exact. `_` consumes one payload without binding it, including `After(result, _)` or `After(result, _, error)`.

### Result transformation

```zirk
fn dec NormalizeName() {
    function(target) {
        match target.transform {
            Wrap(wrapper) => {
                match target.wrap {
                    After(result, transform) => {
                        transform(result.trim());
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
```

### Recovery

```zirk
fn dec RecoverMissing(default_user: User) {
    function(target) {
        match target.transform {
            Wrap(wrapper) => {
                match target.wrap {
                    Catch(error) => {
                        match error {
                            UserError.NotFound => return default_user;
                            _ => throw error;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
```

## 6. Contract preservation

Wrapping preserves name, visibility, parameters, labels, defaults, variadics, generics, constraints, return type, receiver mutability, override slot, documentation, source mapping, and callable identity. Additional inferred errors and permissions remain visible.

Augmentation may add compatible declarations or contracts. It cannot delete or silently rename user declarations, reduce visibility, change an existing public signature, remove errors or permissions, replace a body without provenance, or weaken typing, mutability, ownership, permission, or safety rules.

Generated public API appears in documentation, `public.api`, compatibility analysis, language tooling, and source maps.

## 7. Composition order

Expressions evaluate top-to-bottom; the closest decorator is the innermost wrapper:

```zirk
@Authorized(Role.Admin)
@Cached(5.minutes)
fn report(): Result<Report, ReportError> { /* ... */ }
```

This composes as `Authorized(Cached(report))`; authorization receives the invocation before cache access. The compiler never silently reorders applications.

## 8. Dependencies and ordering

```zirk
fn dec OpenApi()
    requires decorators [Route]
{
    function(target) { /* ... */ }
}

fn dec Authorized(role: Role)
    before decorators [Cached]
{
    function(target) { /* ... */ }
}
```

`requires` demands presence on the same target. `before` demands appearance above the named decorator; `after` demands appearance below it. A contradiction is a compile-time error with a source-reordering suggestion.

A decorator cannot name itself in these clauses. Direct and indirect cycles are rejected with the complete cycle and declaration spans. Same-decorator cardinality belongs to repeatable inspection, not `requires`.

## 9. Repeatable decorators

Contiguous applications form one logical expansion:

```zirk
@Middleware("auth")
@Middleware("cache")
@Middleware("logging")
fn handler(): Result<Response, HttpError> { /* ... */ }
```

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

`applications` is immutable, source-ordered, and typed from the declaration. `application` is introduced by `for`; concrete values use `application.arguments.name`. No free `applications` or ambiguous singular configuration value is injected.

Applications must be contiguous. The decorator decides whether duplicates are meaningful, combined, ignored, warned about, or rejected, and validates cardinality from `applications`.

## 10. Hygiene, conflicts, and recursive expansion

Private generated bindings receive hygienic identities. Public names require explicit builder operations. A collision with user API or another decorator is an error naming both origins; there is no last-writer-wins rule.

Generated declarations do not acquire decorators implicitly. Explicit generated applications enter a later bounded round. Structural cycle detection rejects self-regeneration before hard time, memory, and round limits.

## 11. Generics, inheritance, and overrides

A decorator expands once on a generic declaration and its constraints. The validated form then specializes normally; it is not expanded per monomorphization.

An inherited, non-overridden method retains its wrapper. A new `override fn` does not inherit decorator applications and must reapply desired behavior explicitly.

## 12. Diagnostics and provenance

Decorators emit `target.error`, `target.warning`, and `target.note` against concrete target or argument spans. They cannot suppress compiler errors. Generated-code diagnostics identify the generated declaration, decorator declaration and application, target, expansion path, actionable source span, and repair when known.

```text
error[E72xx]: generated method `validate` returns the wrong type
  --> src/user.zrk:3:1
   |
 3 | @Validated()
   | ^^^^^^^^^^^^ generated by this application
   = target: class `User`
   = generated return: Boolean
   = required return: Result<Void, ValidationError>
```

## 13. Build effects, permissions, and caching

Decorator arguments and expansion use the existing permission model. Privileged operations require an application grant with `during: build` or `both`. A library declares `requires` and cannot grant itself authority; runtime authority does not imply build authority.

Every observed value becomes a build input. The fingerprint includes decorator implementation/version, arguments, typed target, configuration, grants, observations, and transitive dependencies. Matching fingerprints reuse validated expansion; changes invalidate affected nodes only.

Permission or requester changes use signed approval before execution. Secrets cannot enter generated public structure, source, IR, diagnostics, logs, manifests, lockfiles, or cache keys.

## 14. Runtime behavior and framework generation

Zirk 1.x has no `runtime fn dec`, automatic decorator retention, or `Reflection.decorators(...)`. Runtime needs are met by explicitly generated ordinary typed descriptors or registries.

### Routing and injection

```zirk
@Controller("/users")
class UserController {
    @Get("/{id}")
    fn find(
        @Path("id") id: UserId,
        @Inject() service: UserService,
    ): Result<User, HttpError> {
        return service.find(id);
    }
}
```

Expansion can generate:

```zirk
HttpRouter.register(
    method: HttpMethod.Get,
    path: "/users/{id}",
    handler: UserController::find,
    parameters: [
        HttpParameter.path("id", UserId),
        HttpParameter.inject(UserService),
    ],
);
```

```zirk
@Service()
class UserService {
    fn init(@Inject() repository: UserRepository) {
        // Parameter target, not a construct target.
    }
}
```

### Validation and serialization

```zirk
class CreateUser {
    @Length(min: 2, max: 100)
    mut name: String;

    @Email()
    @SerializedName("email_address")
    mut email: String;
}
```

Decorators may generate validators and a typed `TypeDescriptor(CreateUser)` containing only required data. NestJS-like modules/controllers, Spring-like services/entities, FastAPI-like handlers, and Angular-like components/directives all fit the five targets. Framework build tools may consume generated API without new language targets.

## 15. Normative exclusions

Zirk 1.x excludes arbitrary AST mutation, general compile-time blocks, automatic override inheritance, implicit compiler bindings, automatic ordering repair, self/circular dependencies, noncontiguous repeatable grouping, silent public conflicts, automatic decorator metadata, and reflection that bypasses visibility, mutability, permissions, or safety.

## 16. Implementation reading order

Read this document first, then the OpenSpec decorator requirements, grammar/compiler/permission specifications, and metaprogramming handbook. If historical examples conflict, this document wins until specialized specifications are synchronized.
