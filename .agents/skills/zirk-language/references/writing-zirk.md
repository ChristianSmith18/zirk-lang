# Writing Zirk

This is a compact authoring reference. It captures the normal forms and the
semantic decisions most likely to affect code correctness. For a complete API
or a domain with a dedicated semantic source, follow the routing table in
`authority-and-validation.md`.

## File, declaration, and project shape

Zirk is case-sensitive. Source files end in `.zrk`; blocks use `{}`. The
parser permits omitted semicolons where unambiguous, but canonical source uses
them. Use `//` and `/* ... */` comments. Regex literals are `re'pattern'`.

```zirk
import { stdout } from std.io;
import { User, Role -> DomainRole } from "./domain/user";

share class UserService {
    find(id: UInt64): Result<User, UserError> { ... }
}

fn main(): Void {
    println("Hello from Zirk");
}
```

- `share` publishes a declaration outside its file/module.
- `import` brings names into scope. Local/package paths are quoted and omit
  `.zrk`; standard modules are unquoted (`std.io`).
- Importing a compiler-known stdlib object may expose its unambiguous
  convenience members (`import { stdout } from std.io; println("x");`).
- `use NAME;` is only for a global declared by an application in `init.zrk`.
- Name values/functions/methods/parameters/files with `snake_case`, types with
  `UpperCamelCase`, globals/constants with `UPPER_SNAKE_CASE`. Aliases use
  `Original -> Alias`.

Minimal application manifest:

```zirk
project {
    name: "my-app";
    version: "0.1.0";
    type: application;
    entry: "src/main.zrk";
}
```

`init.zrk` is a declarative DSL. Applications may declare `globals` and grant
`permissions`; libraries cannot declare globals and request capabilities with
`requires`. Privileged operations state `during: build`, `runtime`, or `both`.
Never put secrets or consent in this file.

## Bindings, values, references, and nullability

```zirk
mut count: Int32 = 0;
inmut name: String = "Zirk";
inmut::strict config: Config = Config();
mut left, right: Int32 = 3, 4;
left, right = right, left;
```

- `mut` permits rebinding and, for a reference, mutation through it.
- `inmut` prevents rebinding but can permit referent mutation.
- `inmut::strict` freezes the reachable graph; no mutable alias or
  inferred-mutating method call is permitted through it.
- A local cannot shadow a still-visible local or parameter. In a lambda name
  collision, the lambda parameter wins and `this.name` means the capture.
- Grouped declarations apply one type/mutability to all names. No initializer
  gives each the type default. Initializer count and assignment count must be
  exact; RHS values evaluate left-to-right before any LHS write.

Scalar/value data (including tuples, records, enums, ranges, and declared
value types) copies independently. Classes, `String`, `Array`, `List`, `Map`,
and `Set` are reference-backed. Assigning/passing/returning/capturing the
**complete reference variable** shares it. A read of an attribute, index,
slice, destructured component, pattern binding, returned/argument projection,
or projected capture is an independent logical value. If it is reference-backed
it is a deep clone and requires `Clone`; use an explicit view or transfer API
for a non-`Clone` member. The same projection on the left of `=` is a place
that writes original storage.

```zirk
mut users = [User(name: "Ada")];
mut alias = users;           // aliases complete reference
mut selected = users[0];     // independent projected User
users[0].name = "Grace";    // changes users and alias
```

Use `T?` for nullable values (`T | Null`), `?.` for safe access, and `??` for a
fallback. `null` is valid only for nullable types; Zirk has no `undefined`.

## Types, literals, conversion, and operators

Common families:

- Signed integers: `Int8`, `Int16`, `Int32`, `Int64`, `Int128`; `Int` and
  `Integer` alias `Int32`. Unsigned: `UInt8` through `UInt128`.
- Exact decimal and binary floating details have undergone normative
  checkpoints. Before selecting a fractional type, read the currently
  authoritative type passage in `ZIRK_SPEC_FINAL.md` and the relevant feature
  status; do not mix `Decimal`/`Float` naming from older examples blindly.
  Exact decimal and IEEE binary operations never combine implicitly; use an
  explicit constructor conversion at the boundary.
- `Boolean` is only `true`/`false`; no truthiness. `Char` is one Unicode
  grapheme. `String` is a mutable shared grapheme sequence. `Void`, `Never`,
  `Null`, `Object`, and the collection families are special/common types.
- Temporal values are distinct immutable types: `Date`, `Time`, `DateTime`,
  `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`, and `Period`.

Literal rules: `_` separates digits; scientific notation is valid. Integer
conversions to the exact decimal domain are implicit and exact; signed/unsigned
or lossy conversions are explicit. A constructor can establish deep context:
`Decimal(3 / 4)` evaluates that arithmetic in the decimal domain and
`String("value=" + 42)` contextualizes the concatenation tree. Cast with
`source as Type` or `<Type>source`; reinterpretation/removal of safety requires
`unsafe`.

Use `==`/`!=` for structural equality and `is` only for observable identity.
Use boolean-only `&&`, `||`, `!`; arithmetic `+ - * / % **`; compound forms
including `**=`; conventional prefix/postfix `++`/`--`; and ternary
`condition ? yes : no`. `String + String` concatenates and `String * Integer`
repeats with a checked non-negative count. Overflow, invalid bounds, and zero
division are controlled typed failures, never undefined behavior. Do not add
ad-hoc operators: custom types use the language's reserved operator contracts
such as `_add` and `_subtract`.

## Flow, match, ranges, and collections

Headers may omit parentheses (canonical) or include them. Available forms are
`if`/`else`, expression `if`, `while`, `do { ... } while condition;`, `loop`,
traditional `for mut i = 0; test; step {}`, `for item in iterable {}`, `break`,
and `continue`. A one-statement effect-only conditional is valid:

```zirk
if closed return;
mut label: String = match result {
    Ok(value) => "Value: {value}";
    Error(error) => "Error: {error}";
};
```

`match` is exhaustive over closed domains; expression branches unify to a
compatible type. Patterns include values, types, enum/union cases, records and
payloads, `re'...'` full-string regexes, comma-grouped alternatives, and `_`.
No guard syntax exists. Pattern bindings are independent projections. Destructure
records/tuples directly only when irrefutable; destructure algebraic enums in
`match`; rest destructuring is not initially supported.

`start..end` excludes the end, `start..=end` includes it, ranges can use
`.step(positive_nonzero)` and `.reverse()`. Slices are `[start:end:step]`, with
omitted/negative components; `[::]` copies all and `[::-1]` copies reversed.
Slices are independent deep collections, never views; slice assignment has the
same element count and never resizes. `Array<T>` has fixed length; `List<T>` is
resizable; use `Map<K,V>` and `Set<T>` for keyed/unique data. Ordinary iteration
returns independent projected values and structural mutation invalidates an
existing iterator. `view(slice)` is the explicit bounded read-only shared view.

Use collection `map`, `filter`, and `reduce` without assuming they mutate the
source. `value |> f(a)` means `f(value, a)` only; it does not unwrap nullable or
Result values, await work, or map a collection. A generator is `fn gen`, starts
lazily, yields with `yield`, and exposes declared Result-shaped failure.

## Functions, closures, and data types

```zirk
fn add(a: Int32, b: Int32): Int32 {
    return a + b;
}

fn parse(input: String, base?: Int32, ...parts: String): Result<Int32, ParseError> { ... }
inmut formatter: Fn(String) => String = (text): String => text.trim();
```

`Function(P...) => R` and preferred `Fn(P...) => R` are callable types. They
can record labels, optionality and `throws`. Optional parameter spelling is
`name?: T`; variadic spelling is `...values: T` and call expansion is
`...values`. Optional positional parameters follow required ones. Defaults are
concrete-function behavior, not callable-type behavior. There is no implicit
partial application and no traditional function/method overloading.

Lambdas may begin with `fn` or omit it. Contextually typed lambdas can omit
annotations; otherwise parameters and block result must be explicit. Capturing
an immutable value snapshots it; a complete reference aliases; a projection
snapshots/deep-clones; a written mutable capture becomes a shared managed cell.
Closure assignment shares its environment and `clone()` deep-clones it when
possible. A bound method keeps its receiver; `Type.method` is unbound with the
receiver first. Callables use `is`, not `==`.

```zirk
class User implements Serializable {
    inmut id: UInt64;
    name: String;

    construct(id: UInt64, name: String) {
        this.id = id;
        this.name = name;
    }

    display_name(): String { return this.name; }
}

mut user = User(id: 1, name: "Ada");
```

There is no `new`, property declaration, or implicit accessor dispatch; use
`construct`, `this`, and ordinary `get_name`/`set_name` methods. Class fields
default to `public mut`. In type bodies methods omit `fn`; type-body `fn` is an
error. Named arguments use `label: value`; after the first named argument, all
remaining arguments are named. A matching simple visible identifier can use
the shorthand `label:`.

A concrete class `extends` at most one concrete class and may `implements`
interfaces, traits, and abstract classes. Abstract classes are requirement sets
with no state/layout/constructor. Traits offer reusable bodies but no state or
constructors. An inherited concrete method/default is replaced only under a
standalone `#override` marker; satisfying a signature-only requirement needs no
marker. `final` seals a class/method. Classes can contain static nested
`class`, instance-capturing `inner class`, and block-scoped local classes.

Use `<T>` generics and `from A & B` constraints:

```zirk
fn copy_all<T from Clone & Serializable>(values: Iterable<T>): List<T> { ... }
```

Defaults trail required type parameters. Parameters are invariant by default;
declare `out T` only in output positions and `in T` only in input positions.
Use `Box<T>` for recursive inline data. Associated and higher-kinded types are
not initially included.

Data forms: immutable tuples `(a, b)` / `Tuple(A, B)` with literal component
access `tuple.0`; immutable nominal `record` values; traditional enums;
data-only algebraic enums with typed cases; `type Alias = ...`; and normalized
unions `A | B`. Enums have no user methods: write ordinary functions with
`match` for domain behavior. Unions expose only common compatible capabilities
until narrowed.

## Errors, resources, permissions, and safety

Expected operational failure is `Result<T, E>` (`Ok(value)` or `Error(error)`).
Consume it through exhaustive `match` or a Result method (`map`, `and_then`,
`get_or`, `or_throw`, etc.); ignoring it is a compile error. There is no `?`.
Use explicit `throws X` for recoverable exceptional failure and guard-free
pattern catches; catch or propagate declared exceptions. `throw;` only inside a
catch preserves the original throwable. `finally` always runs but cannot
replace an active outcome with return/break/continue/throw/fatalError.

```zirk
try {
    synchronize();
} catch NetworkError.Timeout(duration) {
    retry_after(duration);
} finally {
    metrics.flush();
}
```

Use `match with` for successful `Resource<E>` acquisition. It closes once on
normal flow, error, exception, cancellation, return, break, or continue. It
closes grouped acquisitions right-to-left. Resources cannot escape a managed
scope except through an explicit `TransferableResource` transfer; they are not
ordinary `Clone` values.

```zirk
match File.open("users.json") with file {
    Ok(file) => process(file.read_all());
    Error(error) => report(error);
}
```

Safe code has no use-after-free, null dereference, data race, or undefined
behavior. Raw `Pointer<T>`, native casts/operations, volatile access, unsafe
atomics, and manual safety contracts require `unsafe {}`. Prefer checked
`NativeSlice<T>`/`NativeSliceMut<T>` over raw pointer plus length. A regular
unsafe block rolls back managed/validated-range writes on controlled failure;
it cannot await, spawn, publish tentative state, or perform an irreversible
effect. Put the irreversible portion after explicit `commit {}`. `unsafe` does
not bypass type, scope, mutability, or permission checks.

## Structured concurrency

There is no `async fn`, detached task, public event loop, or standalone
`worker`. Use a typed, scoped `task` and `await`:

```zirk
task scope {
    mut user_task = task { load_user(); };
    mut audit_task = task { load_audit(); };
    mut user = await user_task;
    mut audit = await audit_task;
}

mut events = Channel<Event>(capacity: 8);
await events.send(event);
mut next = await events.receive();
```

Scope exit awaits children; failure cancels siblings, waits for cleanup, then
propagates the primary failure. A `Result.Error` remains an ordinary fulfilled
task value. Use `await value timeout 5s`, bounded `cancellation shield`,
`Task.all`, `Task.first`, `Task.settled`, and fair `select` only according to
their documented failure/cancellation policies.

`parallel`/`parallel for` are finite CPU work and preserve documented ordered
results; reductions require an associative combiner. Use `thread` only for an
OS-thread boundary or blocking/native affinity, and always join structurally;
run legacy blocking work with `await task.blocking(fn() { ... })`.

At task/channel/parallel/thread boundaries values and projections copy;
complete strict immutable references can share; exclusive mutable references
can transfer; clones are independent; synchronized types may share. The
compiler derives `Transfer` and `Share`; do not write them as ordinary traits.
Prefer channels/transfer/strict references. Use a mutex/lock scoped API for
shared mutation and never hold an ordinary mutex across `await`. Atomics protect
single operations; their default ordering is sequentially consistent and weaker
ordering requires `unsafe`.

## Decorators and standard library

Decorators are compile-time declarations: `fn dec Name(...)`, applied with
`@Name(...)`; repeatables are `repeatable fn dec`. They target only `class`,
`attribute`, free `function`, concrete/bodyless `method`, and `parameter`.
Their typed phases are `Inspect -> Augment -> Wrap`; wrappers choose normal
`match` forms such as `Before()`, `After(result)`, `Catch(error)`, or exclusive
`Around(next)`. Decorators are erased after expansion. `#override` is a built-in
member marker, not a decorator. Read `DECORATOR_SEMANTICS.md` before creating
one: generated API, build effects, hygiene, ordering, and provenance matter.

For `std.*`, import the owning object/type and read the corresponding handbook
chapter before using it. Standard APIs prefer typed `Result`, explicit scoped
permissions, cancellation for waits, and documented cross-platform behavior.
Important modules include `std.io`, `std.fs`, `std.path`, `std.process`,
`std.text`, `std.collections`, `std.time`, `std.task`, `std.thread`, `std.sync`,
`std.parallel`, `std.net`, `std.http`, `std.json`, `std.crypto`, `std.testing`,
`std.reflect`, `std.system`, and `std.environment`.
