# Type Aliases

A type alias gives an existing type another name without creating a distinct runtime type.

```zirk
type UserLookup = Result<User, LookupError>;
```

Aliases improve readability for long generic, union, or callable types. Because the aliased and original types are equivalent, an alias does not enforce a domain boundary.

Use a record or class when accidental interchange must be rejected. Avoid chains of aliases that obscure the actual public contract.

An alias preserves the complete operator and member set of its target because it is the same static type. It cannot hide an unsafe operation or add a capability.

## API

An exact alias — the same static type with the complete operator and member
set of its target. Alias lowering for use in executable programs is delivered.

```zirk
type UserLookup = Result<User, LookupError>;
type Handler = Fn(String) => Void;
```

No distinct runtime type, no hidden unsafe operation, no added capability.
There is no alias-specific member surface: an alias exposes exactly the
target's API.

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| *(all members of the aliased type)* | — | Aliases are the same static type | implemented |

---

**Previous:** [← Associated Values](05-associated-values.md) · **Next:** [ Union Types](07-union-types.md)
