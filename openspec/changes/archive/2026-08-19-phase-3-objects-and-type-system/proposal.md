## Why

Phase 2 completed the language surface that **does not depend on objects**: control flow, functions with closures, `match`, nullability, and modules. That is already enough to write programs of modest size, but it cannot model a domain: there is no way to say what a `User` is, nor for two types to share a contract.

`ZIRK_ROADMAP.md` sets this phase as:

> `class`, `construct`, visibility, single inheritance, interfaces, traits. Generics with `from`. Records, value classes, algebraic enums, unions. Casts.

This is the phase where the language stops being a procedure and gains a type system. By the end, the object-oriented subset of the spec works, including basic generics.

### It also retires Phase 2 debts that were waiting precisely for this

Three of Phase 2's pending items were deferred **naming this phase as their condition**, and not closing them here would turn them into debt with no deadline:

- **`?.`** (D8 of Phase 2) was waiting for a type with members to exist.
- **`+` on `String`** was waiting for operator contracts, which `ZIRK_LANGUAGE_SPEC.md` section 4 requires as the only overload path.
- **`for ... in` over user-defined types** (D3 of Phase 2) was waiting for traits.

The later normative source also fixes that `String` is a shared mutable reference and that its native operators include concatenation and repetition. This phase implements the contracts that express them, but the full temporary family and its APIs belong to the standard library phase.

## What Changes

### Classes

- `class` with fields and methods, `construct` as constructor, `this` as the current instance.
- Visibility `public` / `private` / `protected`; a field with no modifiers is equivalent to `public mut`.
- Multiple `construct` declarations, resolved by types, arity, and named arguments, even reordered.
- Single inheritance: a class extends at most one class. Classes are inheritable by default; there is `abstract` for classes and methods, and there is no `final`.

### Contracts

- **Interfaces**: signatures with no implementation, combinable without limit.
- **Traits**: may include reusable implementation.
- **Operator contracts**: the only way to overload `+`, `==`, and their kin, via reserved methods such as `_add` and `_subtract`, without altering precedence or arity (`ZIRK_LANGUAGE_SPEC.md` section 4). Native types cannot be reopened.
- **`Iterable<T>` / `Iterator<T>`**: what makes `for ... in` work over user-defined types and retires the closed protocol from Phase 2.

### Data types

- **Algebraic enums**: extend the traditional `enum` —whose default observable value is the exact case name and which admits `->` mappings— with associated values; `match` gains nested destructuring.
- **Records**: immutable, with structural semantics.
- **Value classes**: no observable identity, storable inline.
- **Unions** `A | B`, and aliases with `type`.

### Generics

- Type parameters `<T>` on functions, classes, and data types.
- Constraints with `from`, checked at the use site.

### Casts

- Postfix `as` and prefix `<T>`, with the checkable form failing in a controlled way.

### Explicitly out of scope

- **Casts that reinterpret memory.** `ZIRK_LANGUAGE_SPEC.md` section 11 requires them inside `unsafe {}`, and `unsafe` belongs to section 13 alongside the rest of the low-level tier. Reinterpreting memory before a memory model exists (Phase 4) would mean nothing.
- **The collections library.** `List<T>`, `Map<K,V>`, and `Set<T>` become *expressible* once generics arrive, but implementing them is Phase 7. Along with them, the variadic parameter from Phase 2 keeps waiting.
- **Generators (`fn gen`), `|>`, and the functional style** from section 8 beyond `Iterable`/`Iterator`.
- **Decorators and reflection** (section 12, Phase 10), errors and `Result` (Phase 4), concurrency (Phase 5).
- **The memory strategy.** This phase needs to allocate and will do so through an abstract operation, without choosing a strategy: that is Phase 4, and ADR-003 fixes it.

## Capabilities

### New Capabilities

- `zirk-classes`: class declaration, constructors, fields, methods, visibility, and single inheritance.
- `zirk-contracts`: interfaces, traits, operator and iteration contracts, and their verification.
- `zirk-generics`: type parameters, `from` constraints, and their checking.
- `zirk-data-types`: records, value classes, algebraic enums, unions, and aliases.
- `zirk-object-memory`: the boundary through which an object is allocated, without naming a strategy (ADR-003).

### Modified Capabilities

- `zirk-grammar`: syntax for all of the above, and removal of the phase diagnostics that covered it.
- `zirk-type-system`: nominal types, subtyping via inheritance and interfaces, member resolution, `?.`, and unification with generics.
- `zirk-ir-lowering`: representation of objects, field access, method calls, and dispatch.
- `zirk-native-codegen`: object layout, method tables, and the indirect call that dynamic dispatch needs.
- `zirk-runtime-io`: `String` stops being an opaque intrinsic and starts implementing the contracts this phase defines.

## Impact

**Modified crates:** all of the pipeline. `zirk-runtime` gains object allocation.

**No new external dependencies.**

**Risks:**

- **This phase has to allocate, and the memory strategy belongs to Phase 4.** This is the central tension: an object with identity outlives the frame that created it, so Phase 2's trick —captures inside the value, on the stack (D10)— does not work. See design.
- **Object layout and the method table are a contract with Phase 8**, just as the shape of the IR was: they travel inside the `.zpkg`. Deciding them poorly is expensive later.
- **Generics tempt growth toward a full system.** The spec asks for `<T>` with `from` constraints and specialization "where appropriate"; it does not ask for variance, associated types, or higher-order types. What is not requested is left out and noted.
- **The scope is the largest in the roadmap so far.** Nine new language constructs against Phase 2's five, and all of them touch each other.
