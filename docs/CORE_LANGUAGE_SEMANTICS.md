# Zirk Core Language Semantics

Status: **authorial source of truth**, accepted 16 August 2026.

This is the shortest complete reading path for compiler, runtime, library,
tooling, and documentation contributors. It describes final language behavior,
not current implementation availability. A phase may postpone a feature but
cannot redefine it. Older examples, inventories, and phase notes are
non-normative when they conflict with this checkpoint.

Errors, `Result`, resources, permissions, unsafe memory, and concurrency are
outside this checkpoint. They are finalized respectively by
[`ERROR_RESOURCE_PERMISSION_SEMANTICS.md`](ERROR_RESOURCE_PERMISSION_SEMANTICS.md),
[`MEMORY_AND_UNSAFE_SEMANTICS.md`](MEMORY_AND_UNSAFE_SEMANTICS.md), and
[`STRUCTURED_CONCURRENCY_SEMANTICS.md`](STRUCTURED_CONCURRENCY_SEMANTICS.md).
Those later checkpoints supersede shorter historical descriptions at their
interaction boundaries.

## 1. Values, references, places, and projections

- Scalars, tuples, records, enums, ranges, and declared value types copy as
  independent values.
- Classes, `String`, `Array`, `List`, `Map`, and `Set` are reference-backed.
  Assigning, passing, returning, or capturing a **complete variable** shares
  its referent.
- A place such as `users[0].name` on the left of `=` mutates original storage
  when permissions allow.
- Reading an attribute, index, slice, destructured component, pattern binding,
  returned projection, argument projection, or captured projection creates an
  independent value. A reference-backed result is deeply cloned and therefore
  requires `Clone`.

```zirk
mut users = [User(name: "Ada")]
mut alias = users              // shared whole reference
mut first = users[0]           // independent projected User
users[0].name = "Grace"        // place write changes users and alias
stdout.println(first.name)     // Ada
```

This is transitive: no nested mutable alias may leak through a projection. A
non-`Clone` reference projection is a compile-time error. Only an explicit
documented view can share a subregion. `inmut` prevents rebinding but can allow
referent mutation; `inmut::strict` freezes the complete reachable graph and
forbids a weaker alias. `clone()` makes a deep independent copy.

Comma-grouped declarations apply one explicit type and permission to every
name. Without explicit initializers, each name receives the type default;
otherwise initializer and destination counts must match exactly. In a
simultaneous assignment, all right-hand expressions evaluate once from left to
right before any write, then their values are assigned positionally. Duplicate
destinations, arity mismatch, incompatible values, rebinding `inmut` or
`inmut::strict`, and mutation through a strict referent are compile-time errors.
This gives `left, right = right, left` swap semantics without constructing a
tuple or exposing a partially updated state.

## 2. Functions and callable values

The native callable type is `Function(P...) => R`; `Fn(P...) => R` is its exact
and preferred alias. Functions, lambdas, bound or unbound methods, and objects
that explicitly implement the callable contract adapt by complete signature.

```zirk
inmut parse: Fn(String) => Result<User, ParseError> = parse_user
mut print: Fn(String) => Void = stdout.println
```

Labels in a callable type govern named invocation. Optional parameters use
`name?: T`; variadics use `...values: T` and expand with `...values`. The type
records optionality, not a concrete default expression. Parameters are
contravariant and results covariant. There is no implicit partial application.

Lambdas use contextual inference. Recursive lambdas require an explicit
binding type. Closures may escape and the compiler chooses efficient storage:

- immutable captured values are snapshots;
- a captured complete reference shares its referent;
- a captured projection is an independent deep snapshot;
- a captured binding written by a closure is lifted into one shared cell;
- assignment shares a closure environment; `clone()` deep-clones it;
- callable identity uses `is`; callables do not implement `==`.

A bound method retains its receiver; an unbound method receives it first.
Receiver-mutation metadata stays compiler-internal. Generators use `fn gen`,
start lazily, suspend at `yield`, clean up deterministically when abandoned, and
expose failures through their declared `Result`-shaped contract rather than
hiding them in completion. `value |> f(a)` lowers to `f(value, a)` and performs
no magical nullable, `Result`, async, or collection propagation.

## 3. Classes and contracts

Stored object data is called an **attribute**. Zirk has no `property` language
construct. Accessors are ordinary `get_name()` / `set_name(value)` methods.
Omitted attribute initializers use the declared type's default.

A concrete class `extends` at most one concrete class. It uses `super(...)` for
base construction and `super.method()` for inherited behavior. Public and
protected instance methods are virtual by default; private and static methods
are not. Overrides require `override fn`, exact compatible parameters, and may
return a covariant result.

An `abstract class` is a nominal requirement set: required attributes and
abstract functions, but no body, constructor, state, or layout. It is adopted
with `implements`, never `extends`. Interfaces contain behavior signatures.
Traits contain requirements and reusable bodies but no attributes,
constructors, or state. All three compose through `implements`. Trait conflicts
require an override and may select `TraitName.super.method()`. Derivation is
explicit and static contract requirements are deferred. `as` is a strict
checked cast; `as?` returns a nullable cast result.

## 4. Generics

```zirk
fn max<T from Comparable<T> & Clone>(left: T, right: T): T
class Cache<K from Hashable & Equatable, out V> { ... }
```

Constraints may name interfaces, traits, abstract requirement classes,
concrete bases, or native contracts. Inference uses arguments, receiver,
expected result, callable context, and constraints, but never guesses between
ambiguous solutions. Explicit arguments resolve ambiguity. Defaults are
trailing only. Generic methods are allowed; constructors do not introduce a
separate generic parameter list, so factories cover that need.

Generics are invariant unless declared `out` or `in`; every use position is
checked and mutable APIs normally force invariance. Recursive constraints are
legal. `Box<T>` supplies managed indirection for an otherwise infinite inline
layout. Runtime type identity preserves all type arguments. Bodies are checked
once against constraints; final code may be monomorphized and equivalent code
shared without erasing observable identity. Associated and higher-kinded types
are deferred. Generic projection APIs require `Clone` when they return an
independent reference-backed result.

## 5. Algebraic data and matching

`Tuple(A, B)` is the type of `(a, b)`. Access is `result.n`, where `n` is a
compile-time integer literal. Tuples do not slice. Contracts derive component-wise.

Records are nominal immutable values with named construction and type defaults
for omitted members. They may have non-mutating methods, but no custom
constructor, inheritance, or mutation. Equality and hashing are structural.

Traditional enums are data-only and expose `.name`, `.value`, `to_string()`,
`from_name()`, and `from_value()`. Mappings are unique; no implicit index,
ordering, or user method exists. Algebraic enums can carry generic payloads,
which are extracted only inside `match`, never by direct destructuring.

Unions normalize order, duplicates, subsumed members, and `Never`; only common
compatible capabilities are accessible before narrowing. Every statement and
expression `match` over a closed domain is exhaustive. Patterns support values,
types, enum variants, unions, regex, alternatives, and nesting. Guards are not
part of Zirk. Proven unreachable patterns are errors. Bindings are projected
copies. Direct destructuring supports records and tuples only; enum and rest
destructuring are invalid. `Never` unifies with every branch result type.

## 6. Collections and iteration

`Array`, fixed arrays, `List`, `Map`, and `Set` are reference containers;
`Range` and tuples are values. Whole-variable assignment aliases, while index,
slice, entry, iterator, destructuring, and returned component reads project
independent values.

`String`, tuple, array, and list indexing accepts negatives. Missing direct
access is a typed controlled error. `get` returns `Result`; a family may also
offer an explicitly nullable `get_or_null`.

`String`, arrays, and lists slice with `[start:end:step]`; tuple, map, and set do
not. End is exclusive. Positive or omitted step defaults to `(0, length, 1)`;
a negative step defaults to `(length - 1, before-first, negative-step)`. Thus
`[::]` copies all, `[::-1]` reverses, and `[n:w]` uses step `1`. Step zero and
explicitly out-of-bounds indexes are errors: Zirk does not clamp explicit
bounds. Slices are deep independent collections. Replacement must have exactly
the same length; list resizing uses an explicit structural operation.

Array/list equality is ordered, map equality compares mappings, and set
equality compares membership. Required element/key contracts control whether
an operation exists.

```zirk
enum Iteration<T> { Item(T), Done }
interface Iterable<out T> { fn iterator(): Iterator<T> }
interface Iterator<out T> { fn next(): Iteration<T> }
```

Ordinary loops yield independent projected values. Structural mutation
invalidates active iterators deterministically. Explicit read-only views may
share storage for a checked lifetime; mutable iteration is deferred. `Range`
is a finite lazy reusable value iterable. Collection transformations are eager;
iterator adapters are single-pass/lazy and require an explicit materialization
terminal. Task-aware streams remain a separate contract so iteration never
hides `await`. List growth is automatic and allocation capacity is not a public
source-level API.

## 7. Mandatory contributor reading order

1. [`ZIRK_SPEC_FINAL.md`](ZIRK_SPEC_FINAL.md): authority and invariants.
2. This checkpoint: accepted cross-feature semantics.
3. [`ERROR_RESOURCE_PERMISSION_SEMANTICS.md`](ERROR_RESOURCE_PERMISSION_SEMANTICS.md): accepted failure, cleanup and authority semantics.
4. [`ZIRK_LANGUAGE_SPEC.md`](ZIRK_LANGUAGE_SPEC.md): normative language.
5. [`ZIRK_RUNTIME_SPEC.md`](ZIRK_RUNTIME_SPEC.md): runtime and enforcement.
6. [`ZIRK_STDLIB_SPEC.md`](ZIRK_STDLIB_SPEC.md): native and library contracts.
7. [`ZIRK_COMPILER_SPEC.md`](ZIRK_COMPILER_SPEC.md): checking and tooling.
8. [`handbook/README.md`](handbook/README.md): teaching and examples.
9. `openspec/specs/` plus an active change's artifacts: implementable rules.

`01_plantilla_zirk.md`, roadmaps, archived changes, and old phase discussions
preserve provenance; they do not outrank the sources above.

---

**Previous:** [Master Specification](ZIRK_SPEC_FINAL.md) · **Next:** [Errors, Resources, and Permissions](ERROR_RESOURCE_PERMISSION_SEMANTICS.md)
