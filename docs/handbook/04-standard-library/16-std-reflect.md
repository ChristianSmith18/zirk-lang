# `std.reflect`

`std.reflect` exposes safe runtime type identity. It lets a program identify a
value's concrete type, compare type values, inspect broad type classification,
and observe generic arguments and declared type relationships. It does not
provide general structural reflection, dynamic member access, retained
decorators, or access to compiler syntax.

Frameworks that need runtime structure generate ordinary typed descriptors,
factories, accessors, or registries during decorator expansion. Generated values
obey the same typing, visibility, compatibility, permissions, and dead-code
rules as handwritten code.

> **Implementation status:** this chapter defines the target Zirk 1.x
> reflection boundary. Basic type identity is part of the language/runtime
> contract. Descriptor shapes belong to the libraries that generate them.

## Import

```zirk
import { Type, TypeKind } from std.reflect;
```

Values and declared types expose `type()` directly, so ordinary identity checks
do not require importing a global reflection object:

```zirk
mut runtime_type: Type = user.type();
mut declared_type: Type = User.type();
```

`value.type()` returns the concrete runtime type. `T.type()` returns the
identity of the declared type `T`:

```zirk
mut account: Account = AdminAccount();

account.type();      // AdminAccount.type()
Account.type();      // TypeKind.Interface
AdminAccount.type(); // TypeKind.Class
```

This distinction supports interface-typed values without hiding their concrete
runtime identity.

## Comparing types

`Type` has value equality. The readable `is` operation and ordinary equality
express the same identity test:

```zirk
if user.type().is(User) {
    stdout.println("concrete User");
}

if user.type() == User.type() {
    stdout.println("same type identity");
}
```

`Type.is(T)` accepts a type, not a string. It cannot be fooled by two unrelated
types with the same short name. Type comparison does not perform a cast and does
not weaken visibility or mutability.

## Names and ownership

A `Type` exposes:

```zirk
runtime_type.name
runtime_type.qualified_name
```

`name` is the declaration's short name. `qualified_name` includes the owning
project/package and logical file-module path. Given:

```text
shop-api/
├── init.zrk                 // project.name = "shop-api"
└── src/domain/user.zrk      // share class User
```

the values are conceptually:

```zirk
User.type().name;           // "User"
User.type().qualified_name; // "shop-api::domain.user::User"
```

The qualified form is diagnostic/reflection text, not import syntax. Source
imports continue to use the module rules:

```zirk
import { User } from "./domain/user";
```

Package version and integrity do not become part of the display name. The
loader/compiler tracks them in compatibility metadata. `qualified_name` is
stable within a package's public API but must not be used as a persistent
database or wire-protocol identifier. Applications that need such identity
declare an explicit versioned domain name.

## Type classification

`Type.kind` is a compiler-produced `TypeKind` value. The public cases are:

```zirk
enum TypeKind {
    Primitive;
    Class;
    AbstractClass;
    Interface;
    Trait;
    Record;
    ValueClass;
    Enum;
    Union;
    Function;
    Collection;
    Native;
}
```

The compiler derives the case from the declaration and constructed type; an
application does not instantiate or assign a kind manually:

```zirk
User.type().kind;       // TypeKind.Class
Repository.type().kind; // TypeKind.Interface
Point.type().kind;      // TypeKind.Record
Status.type().kind;     // TypeKind.Enum
```

Classification is intentionally broad. It is useful for diagnostics, tooling,
and guarded generic infrastructure, but it is not a substitute for typed
contracts or pattern matching over application values.

## Generic identity

Constructed generic types preserve their arguments:

```zirk
List<String>.type() != List<Int>.type();

mut list_type = List<String>.type();
list_type.is_generic;                  // true
list_type.generic_arguments[0];        // String.type()
```

`generic_arguments` is an immutable list of `Type` values in declaration order.
A non-constructed type has an empty list. Generic type equality includes the
generic declaration and every argument, including nested arguments.

This identity remains available even when monomorphization or representation
sharing lets compatible constructed types reuse machine code. Optimization
cannot make distinct checked types reflect as equal.

## Declared relationships

`implements` and `extends` query relationships already present in the type
system:

```zirk
if user.type().implements(Serializable) {
    stdout.println("serializable contract");
}

if AdminAccount.type().extends(BaseAccount) {
    stdout.println("class inheritance");
}
```

These operations accept declared types, not names. `implements` covers the
language's applicable interface, trait, and contract conformance. `extends`
covers class inheritance. They do not enumerate implementations, reveal private
members, or manufacture a value satisfying the relationship.

## Casts remain separate

Reflection observes identity; casts perform checked conversion or narrowing:

```zirk
if value.type().is(User) {
    mut user = value as User;
    stdout.println(user.name);
}
```

`std.reflect` adds no second dynamic casting system. Invalid checkable casts
retain the language's controlled failure behavior, while representation casts
remain confined to `unsafe`.

## Explicit generated descriptors

Runtime structure is opt-in and library-owned. A decorator can generate an
ordinary static method and descriptor:

```zirk
@Serializable()
class User {
    mut name: String;
    mut age: UInt8;
}

mut descriptor = User.descriptor();
```

The call uses ordinary static-method syntax: `User.descriptor()`, not
`User::descriptor()`. A JSON library might generate a
`JSONTypeDescriptor<User>`, an HTTP framework a `RouteDescriptor`, and an
injection library a typed factory. Zirk does not force them into a universal
descriptor containing every member.

Conceptually generated access remains typed:

```zirk
JSONField<User, String>(
    name: "name",
    read: fn(user) => user.name,
);
```

The descriptor contains only information requested by the library and permitted
by normal visibility. A public generated descriptor appears in `public.api`,
documentation, and compatibility checks. A private unused descriptor can be
eliminated.

## Deliberate exclusions

Zirk 1.x does not provide general operations equivalent to:

```zirk
Reflection.decorators(User)
Reflect.create(type)
Reflect.invoke(value, "method_name")
Reflect.get(value, "field_name")
Reflect.set(value, "field_name", replacement)
```

It also does not automatically expose `fields`, `methods`, `constructors`,
`attributes`, or decorator applications on `Type`. This prevents reflection
from bypassing privacy, strict immutability, constructor validation, error
contracts, or permission inference.

If dynamic-looking framework behavior is needed, expansion generates a typed
registry, factory, match, or accessor. Consequently, missing members and
incompatible signatures fail during compilation instead of becoming stringly
typed runtime errors.

## Decorator erasure

Zirk 1.x has no `runtime fn dec`, automatic decorator retention, general
`Reflection.decorators(...)`, or unrestricted `comptime {}` block. Decorators
execute through the controlled Syntax API during compilation, in the established
`Inspect -> Augment -> Wrap` phases, and are erased after expansion.

This means runtime code cannot recover a decorator's identity, arguments, or
applications unless the decorator explicitly generated ordinary data for that
purpose. It also means arbitrary application code cannot execute inside the
compiler merely by entering a block.

The boundary enables smaller native binaries, deterministic and cacheable
builds, dead-code elimination, static framework startup, and strict
encapsulation. It does not prevent NestJS-, Spring-, FastAPI-, or Angular-style
frameworks: those frameworks generate routes, providers, validators, factories,
and metadata registries as typed code.

## Layout and native information

`Type` does not expose runtime size, alignment, field offsets, object headers,
calling convention, or optimized representation. These properties can differ
by target, build profile, specialization, and compiler version. Stable native
layout belongs to explicit ABI/native APIs and may require `unsafe`.

Logical properties such as numeric bounds remain part of their owning type's
ordinary public contract rather than general reflection.

## Compatibility and permissions

Basic identity queries require no permission and perform no external effect.
Generated descriptors grant no authority. A decorator that reads files, uses
the environment, or performs another build effect still requires its declared
build permission; runtime use of generated code carries the permissions of the
operations it actually calls.

Types crossing compatible packages or dynamic libraries retain semantic
identity through compiler and package metadata, not display-string comparison.
An incompatible definition is rejected during package/API/ABI validation rather
than producing two ambiguous runtime types.

## Syntax API is separate

The compiler's immutable Syntax API represents source and generated declarations
during decorator expansion. It is not exported through `std.reflect`, cannot be
stored for runtime traversal, and cannot be used to mutate compiler internals.
See the [Metaprogramming](../06-metaprogramming/README.md) unit for that compile-
time model.

---

**Previous:** [← std.testing](15-std-testing.md) · **Next:** [std.system →](17-std-system.md)
