# Type Aliases

A type alias gives a type expression another name without creating a distinct
runtime type. It can name a primitive, structural shape, union, intersection,
callable, or generic application.

```zirk
type Outcome<T, E> = { value: T } | { error: E };
type TextOutcome = Outcome<String, String>;
type Handler = Fn(String) => Void;
```

Aliases improve readability for long generic, union, or callable types. Because the aliased and original types are equivalent, an alias does not enforce a domain boundary.

Use a contract when structural compatibility is intended, or a class when accidental interchange must be rejected. Avoid chains of aliases that obscure the actual public contract.

An alias preserves the complete operator and member set of its target because
it is the same static type. Generic aliases may declare constraints and
trailing defaults. Substitution preserves canonical union and intersection
identity, while diagnostics and editor queries can retain the written alias
name. An alias cannot hide an unsafe operation or add a capability.

## API

An alias is transparent: it has the same static type and complete member set
as its target.

```zirk
type Outcome<T, E> = { value: T } | { error: E };
type TextOutcome = Outcome<String, String>;
type Handler = Fn(String) => Void;
```

No distinct runtime type, no hidden unsafe operation, no added capability.
There is no alias-specific member surface: an alias exposes exactly the
target's API.

Type-level operators can build reusable aliases from existing structural
types. Zirk supports `keyof`, indexed access, conditional types with `infer`,
mapped types, finite template-literal types, and the standard utility aliases.
These expressions are evaluated during checking and have no runtime value.
They obey visibility and deterministic computation limits. See the
[generic type operators](../11-generics/09-type-operators.md) reference.

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| *(all members of the aliased type)* | — | Aliases are the same static type | implemented |

---

**Previous:** [← Associated Values](05-associated-values.md) · **Next:** [ Union Types](07-union-types.md)
