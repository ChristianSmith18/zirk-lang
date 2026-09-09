# Zirk — Language specification

<p align="center">
  <img src="assets/zirk-clean.svg" alt="Zirk Logo" width="160" height="160" />
</p>

This document defines the public semantics of `.zrk` source code.

## 1. Lexical structure

Zirk is case-sensitive. Blocks use `{}`. The semicolon is optional for the
parser where there is no ambiguity, but the official formatter adds it. Both
`//` and `/* ... */` are supported; documentation uses multi-line documentation
comments.

Regex literals use `re'pattern'`. The lexer recognizes `**`, `**=`, `..=`,
`do`, `gen`, and `yield`. An unterminated regex is diagnosed at its opening
delimiter.

Conventions:

- variables, functions, methods, parameters and files: `snake_case`;
- classes, interfaces, traits, records and enums: `UpperCamelCase`;
- constants and globals: `UPPER_SNAKE_CASE`;
- named parameters: `name: value`;
- aliases in imports and destructuring: `Original -> Alias`.

## 2. Variables, scopes and globals

```text
mut count: Int32 = 0;
inmut NAME: String = "Zirk";
inmut::strict CONFIG: Config = Config();
```

`mut` allows reassignment and, for a reference, mutation of its referent.
`inmut` freezes the binding but still permits mutation through the reference.
`inmut::strict` freezes both binding and reachable state. A strict reference
cannot yield a mutable alias and cannot be acquired while an accessible mutable
alias exists; `clone()` may create an independent mutable value when supported.
Uppercase is a convention, not semantics encoded in the name.

Inference is allowed where it is unambiguous. Every type has a default value;
flow analysis prevents reading a variable that is not yet available.

Reference boundaries are syntactic and observable. Assigning, passing,
returning or capturing a variable that contains a complete reference shares
that reference. Reading a projection such as `user.name`, `users[index]`, a
slice, destructured component or pattern binding produces an independent
logical value; a reference-valued projection therefore performs a deep clone
and requires `Clone`. A projection used as a place, as in
`users[index].name = "Grace"`, retains direct access to original storage.

Comma-grouped binding syntax supports declarations and simultaneous assignment.
A declaration such as `mut first, second: String;` creates two independent
bindings of the same declared type and initializes each with that type's
default. Initializers must have exactly the same arity as the declared names:

```zirk
mut left, right: Int32 = 3, 4;
```

Simultaneous assignment also requires exact arity:

```zirk
left, right = right, left;
```

Every right-hand expression is evaluated exactly once, from left to right,
before any destination is written. Each produced value is then checked against
its corresponding destination and committed from left to right. A destination
may appear only once. Rebinding an `inmut` or `inmut::strict` binding is a
compile-time error. A projected place may be updated through a reference only
when its referent permissions allow it; `inmut::strict` therefore rejects every
such internal write. The syntax is assignment, not tuple construction or
destructuring, and a count mismatch is always a compile-time error.

There are block, function, file and module scopes. A file symbol does not leave
it unless published with `share`.

An ordinary local cannot hide another still-visible local or parameter. Lambda
parameters are the deliberate capture exception: the plain name selects the
lambda-local binding and `this.name` selects the captured outer value.

Only an `application` may declare globals, and exclusively in the `globals`
block of `init.zrk`:

```text
globals {
    inmut APP_NAME: String = "App";
    mut REQUEST_COUNT: Atomic<UInt64> = Atomic(0);
}
```

Each consumer uses `use APP_NAME;`. A mutable global accessed from `parallel` or
`thread` must be protected with `sync` or `Atomic<T>`; otherwise it is a compile
error.

## 3. Type system

Typing is static with inference. `Object` is the conceptual semantic root; it
does not require every value to be heap allocated or boxed. Types are grouped
as compiler primitives, native values, native references, user-defined values
or references, and special types. Capabilities such as equality, comparison,
hashing, cloning, iteration and arithmetic come from contracts.

Fundamental families:

- signed `Int8`, `Int16`, `Int32`, `Int64`, `Int128`, with `Int` and `Integer`
  aliasing the default `Int32`;
- unsigned `UInt8`, `UInt16`, `UInt32`, `UInt64`, `UInt128`;
- `Decimal`, an exact base-ten decimal (a 128-bit integer coefficient with a
  decimal scale, up to 38 significant digits); it is the default fractional
  type and has no width family, no `NaN` and no infinity; `Dec` is an alias;
- binary `Float16`, `Float32`, `Float64`, `Float128`,
  with `Float` aliasing `Float64` — IEEE 754 for graphics, DSP,
  numerics and C interop, written with an `f` literal suffix (`1.5f`, `1.5f32`);
- primitive `Boolean`, exactly `true` or `false`;
- primitive `Char`, exactly one Unicode grapheme, even when composed of
  multiple code points and bytes;
- native reference `String`, a mutable Unicode grapheme sequence with shared
  aliasing and an adaptive internal index/cache;
- immutable native temporal values `Date`, `Time`, `DateTime`, `Instant`,
  `ZonedDateTime`, `TimeZone`, `Duration` and `Period`;
- special `Void`, `Never` and `Null`, plus `Object` and collection families.

There is no numeric truthiness. `Boolean?` admits `null`; `Boolean` does not.
There is no `undefined`.

Literals admit scientific notation (`1e2`) and `_` as a separator
(`1_000_000`). A suffix-less fractional literal is an exact `Decimal`; an `f`
suffix (`1.5f`, `0.1f128`) makes it a `Float`; an optional width (`f16`,
`f32`, `f64`, `f128`) selects the `FloatN` type. `Decimal` is the default
fractional type and takes no suffix. A suffix-less literal that
needs more than 38 significant digits to be exact is a compile error that
suggests the `f` suffix. `Decimal` `+ - *` and integer `**` are exact; `/`, `%`,
`sqrt` and fractional `pow` round half-to-even to a 28-significant-digit
budget. `Decimal` has no `NaN` and no infinity — coefficient overflow and
division by zero are controlled errors. `Float` keeps IEEE 754 behavior:
explicit positive and negative infinity, no valid `NaN`, and a `NaN`-producing
operation as a controlled error. `Decimal` and `Float` never combine in one
operation — an explicit `Float(x)` / `Decimal(x)` conversion is required.
Duration suffixes are `ns`, `us`, `ms`, `s`, `m`, `h`, `d` and `w`; calendar
months and years use `Period`.

Ordinary overflow produces a controlled error; wrapping, saturating and checked
variants must be explicit operations. Safe widening may be implicit where
unambiguous; signed/unsigned and lossy conversions are explicit. Every integer
converts implicitly and exactly to `Decimal`; mixed integer and `Decimal`
arithmetic produces `Decimal`, and mixed integer and `Float` arithmetic
produces `Float`.

An explicit constructor may establish a deep contextual domain for the
compatible operator tree directly inside it. `Decimal(3 / 4)` converts operands
before division and yields `0.75`; `String("value=" + 42)` converts operands
before concatenation. Context does not mutate operands or cross into a called
function's body.

## 4. Nullability, equality and operators

`T?` is equivalent to `T | Null`. Safe access uses `?.` and the fallback value
uses `??`.

- `==` and `!=`: structural equality.
- `is`: same instance, only for types with observable identity.
- Comparators: `<`, `<=`, `>`, `>=`, per the contracts of the type.
- Logical: `&&`, `||`, `!`, booleans only.
- Arithmetic and compound: `+`, `-`, `*`, `/`, `%`, `**`, `+=`, `-=`, `*=`, `/=`, `%=`, `**=`.
- Increment: `count++`, `count--`, `++count`, `--count`, preserving conventional
  postfix/prefix semantics.
- Ternary: `condition ? when_true : when_false`.

Integer division truncates toward zero and remainder keeps the dividend sign,
so `-10 / 3 == -3` and `-10 % 3 == -1`. `String + String` concatenates;
`String * Integer` and `Integer * String` repeat with a checked non-negative
count (`"ja" * 3 == "jajaja"`). A negative/non-integer count or impossible
allocation is a controlled error. Boolean admits only equality and
short-circuit logical operators. Native types expose no undocumented operator.

Operators may only be overloaded through contracts defined by the language; an
overload cannot alter precedence or arity.

User-defined types implement those contracts with reserved methods such as
`_add` and `_subtract` in safe code. Application code cannot reopen native
types or replace their fundamental behavior; `unsafe` remains for memory and
ABI operations, not ordinary operator implementation.

## 5. Control flow and pattern matching

`if`/`else`, traditional `for`, `for ... in`, `while`, `do ... while`, `loop`,
`break` and `continue` are included.

Parentheses around a control-structure header are **optional** in every one of
these forms and in `match`. The canonical style, which the formatter produces
and the documentation uses, omits them; writing them is valid and changes
nothing:

```text
while pending { }        // canonical
while (pending) { }      // equally valid
```

`if` may be an expression when every branch produces compatible types. The
ternary is preferred for a short value choice. An effect-only `if` may govern
one immediate statement without braces:

```text
if closed return;
```

The traditional loop is `for mut i = 0; i < 10; i++ { ... }`. A post-condition
loop is `do { ... } while condition;` and always executes its body once.

`match` over a closed domain is exhaustive in both statement and expression
form. A statement produces `Void`; an expression unifies branch results, with
`Never` compatible with every result:

```text
mut message: String = match result {
    Ok(value) => "Value: {value}";
    Error(error) => "Error: {error}";
};
```

Patterns admit compatible values, types, enum variants, unions, complete-match
regex literals, comma-grouped alternatives, wildcards and nested record/payload
forms. Pattern guards are not part of Zirk: conditional logic belongs in the
branch body. A demonstrably unreachable branch is an error. Pattern bindings
are projection reads and therefore independent values. Records and tuples may
be destructured directly when irrefutable; an algebraic enum is unpacked only
inside `match`, and rest destructuring is not initially supported.

Commas group alternative patterns before one `=>` body. A regex literal is a
string pattern, and nested patterns compose:

```text
match input {
    re'^[0-9]+$' => parse_number(input);
    _ => reject(input);
}

match event {
    UserCreated({ id, name }) => audit(id, name);
    _ => ignore(event);
}
```

`match with` acquires a `Resource<E>` and guarantees it is closed when any
branch ends, including error, exception, `return` or cancellation:

```text
mut first_line: String = match File.open("data.txt") with file {
    Ok(file) => file.read_line();
    Error(error) => "";
};
```

The resource is closed before the value is delivered and cannot escape directly
or indirectly.

## 6. Functions and closures

```text
fn add(a: Int32, b: Int32): Int32 {
    return a + b;
}
```

The native callable type is `Function(P...) => R`; `Fn(P...) => R` is its exact
alias and the conventional spelling. Parameter labels may be written when a
callable publishes named invocation. Optional parameters use `name?: T`, a
variadic uses `...values: T`, and defaults belong to the concrete function, not
the callable type. Parameters are contravariant and results covariant; expected
failure remains explicit in a result such as
`Fn(String) => Result<User, ParseError>`.

Local inference, typed optional parameters (`name?: String`), nullable types
(`String?`), default values, named parameters and variadics (`...values`) are
supported. Optional positional parameters follow required ones. Inside a
function, a variadic is an ordered read-only `Iterable<T>` valid for the call.
There is no traditional overloading; unions, generics or different names are
used instead.

Lambdas are equivalent to function values:

```text
inmut ADD = (a: Int32, b: Int32): Int32 => a + b;
inmut SUBTRACT = fn(a: Int32, b: Int32): Int32 => a - b;
inmut ACTION = (): Void => {
    stdout.println("ok");
};
```

A contextually typed lambda may omit parameter/result annotations; without a
complete context its parameters and block result must be explicit. Closures may
escape through parameters, returns, attributes and collections. Immutable
value captures are snapshots, a whole reference capture shares its referent,
a projected capture snapshots the independent projected value, and a mutable
binding written by closures is lifted into one shared compiler-managed cell.
Escape analysis selects inline/stack or managed storage without changing
behavior. Assignment shares a closure environment; `clone()` deep-clones it
when all captures are cloneable. Callables have identity through `is` and no
structural equality.

The leading `fn` is optional on a lambda. When a lambda-local name collides with
a capture, `this.name` selects the capture. Naming a function produces its
callable directly. `receiver.method` binds the evaluated receiver;
`Type.method` exposes an unbound receiver as the first parameter. A bound
callable preserves receiver mutation requirements internally. Partial
application is never implicit. A class may adapt to `Fn` only through the
explicit callable contract.

## 7. Objects and data types

```text
class User implements Serializable {
    inmut id: UInt64;
    name: String;

    construct(id: UInt64, name: String) {
        this.id = id;
        this.name = name;
    }
}

mut user = User(1, "Cristian");
mut named_user = User(name: "Cristian", id: 1);
```

The constructor is called `construct`; there is no `new`; the current instance
is `this`. Visibility is `public`, `private` or `protected`. An unmodified class
attribute is `public mut`; either modifier may be written explicitly. Every
omitted attribute receives its type default before explicit initializers and
constructor execution. A constructor may finalize an `inmut` attribute before
the instance becomes available. There is no property declaration or implicit
accessor dispatch; APIs use ordinary `get_` and `set_` methods.

A class may declare multiple `construct` members when their effective
parameter signatures differ. Resolution uses arity, type and argument labels,
including reordered named arguments, and rejects duplicate or ambiguous sets.
This exception does not enable ordinary function or method overloading.

Named calls use `name: value`. When a simple visible identifier has exactly the
parameter name, `name:` abbreviates `name: name`, so
`client.get(timeout:, url:)` is explicit, reorderable named binding rather than
positional inference. After the first named argument, every remaining argument
must be named. Repeated labels, missing shorthand variables and shorthand on a
member expression are compile-time errors.

Inside a `class`, `abstract class`, `interface`, `trait` or `record` body a
method is declared without `fn`: `name(params): Return { ... }`, while a field
keeps `name: Type;`. `fn` remains only for top-level functions, function types
(`fn(Int32): Void`), `fn dec`, `unsafe fn` and `extern "C" fn`; writing `fn`
inside a type body is a targeted compile-time error. `mut` does not apply to
methods either: whether a method mutates its receiver is inferred by the
compiler — a method mutates if it writes `this.*` (directly or through a place
rooted at `this`) or calls another inferred-mutating `this` method, computed by
fixed point over the intra-class call graph. For an `inmut::strict` receiver,
calling an inferred-mutating method is an error whose diagnostic shows the
mutation chain; plain `inmut` is unaffected, and methods without an analyzable
body (`extern "C"` callables) are conservatively treated as mutating. `mut` on
fields and local bindings is unchanged.

A concrete class may `extends` one concrete class and uses ordinary `super(...)`
or `super.method()`. Public/protected instance methods dispatch virtually by
default; private/static methods do not. Replacing an inherited concrete
implementation — a base-class method or a trait default — requires the
`#override` member marker, written on its own line above the method; the method
keeps exact parameter contracts and may narrow the result. Satisfying an
`abstract class` or `interface` requirement needs no marker, and writing
`#override` where nothing is overridden is an error (a warning when it merely
satisfies a signature-only requirement). `#override` applies only to instance
methods, never to fields, constructors, nested classes or `static` members.

Classes are inheritable by default, and `final` opts out: a `final class`
cannot be extended (it may still implement contracts and be instantiated) and a
`final` method cannot be overridden. `final` and `abstract` on the same class
are contradictory and rejected; a `final` method inside a `final class` is
redundant and produces a warning. `final` on fields, constructors, interfaces,
traits or records is an error — attribute mutability is covered by
`mut`/`inmut`/`inmut::strict` and records are already non-inheritable.

A class body may also declare nested classes. `class Nested {}` is a static
nested class: pure namespacing under the canonical name `Outer.Nested`, with no
access to the enclosing instance, arbitrary depth, and ordinary member
visibility. `inner class Inner {}` captures the enclosing instance through a
hidden `outer` field and a hidden constructor parameter; inner methods may
reference `outer` and access `private`/`protected` members of the enclosing
class, and the outer class may access private members of its inners. An `inner`
class may not declare `static` members. Construction binds an enclosing
instance: `Inner()` inside `Outer` captures `this`; outside, `o.Inner()`. A
`class` declaration is also a valid statement inside a function, method or
constructor body: a local class is scoped to its block, receives a mangled
canonical name and does not capture enclosing local variables. Anonymous
classes (`Contract() { ... }`) are not part of the language — a local class or
a closure is the idiomatic replacement.

An `abstract class` is instead a nominal set of required attributes and
`abstract` method signatures with no constructor, body, allocated state or
layout contribution; a class adopts it with `implements`. Interfaces contain behavior
signatures only. Traits contain behavior requirements and reusable bodies but
no attributes or constructors. Interfaces, traits and abstract classes compose
through `implements`; cycles/incompatible requirements fail, and a class
resolves competing trait defaults with the `#override` marker and
`TraitName.super.method()`. Capability derivation is always requested
explicitly.

Generics use `<T>` and combined constraints use `from A & B`:

```text
fn serialize<T from Clone & Serializable>(value: T): String { ... }
```

Trailing generic defaults are allowed. Inference uses arguments, receiver,
expected result, callable context and constraints but never guesses. Parameters
are invariant by default; `out T` is restricted to output positions and `in T`
to input positions, while mutable storage requires invariance. Recursive
constraints are allowed when verifiable; `Box<T>` supplies managed indirection
for otherwise infinite inline values. Associated and higher-kinded types are
not initially included. Generic bodies are checked once against constraints;
portable IR retains complete instantiation identity and final builds may
monomorphize or share code only without observable or ABI change.

Additional data types:

- traditional enums, whose unmapped cases expose their exact names as default
  string values and may map with `->` to compatible string or numeric values;
- algebraic enums with zero or more associated typed values, unpacked only by
  `match`; enums are data-only and declare no user methods;
- aliases via `type`;
- unions `A | B`;
- immutable nominal records with named construction, type defaults, methods
  without mutation and structural field equality;
- immutable heterogeneous tuples typed `Tuple(A, B)`, constructed `(a, b)` and
  indexed only by compile-time integer literal `tuple.n`;
- fixed-length arrays written as `T[]`, canonically sized as `T[n]`, or
  constructed as `Array<T>(n)`; resizable `List<T>`; `Map<K,V>` and `Set<T>`.

A union is order-independent and removes duplicates, `Never`, and alternatives
subsumed by a supertype; before narrowing it exposes only common compatible
capabilities. A traditional enum exposes native `.name` and `.value`, plus
explicit name/value lookup, without implicit mapping conversion or declaration
order. Domain behavior for any enum is an external function using `match`.

A normal class has identity and state; a record represents data. `clone()`
exists only through an explicit trait and may be derived when every field is
cloneable.

## 8. Iteration and functional style

`Iterable<out T>` and `Iterator<T>` define iteration. `Iterator.next()` returns
the native data-only enum `Iteration<T>` with `Item(value)` and `Done`, avoiding
nullable ambiguity. Ordinary iteration binds independent projected copies.
Structural container mutation invalidates an existing iterator and its next
operation raises a controlled invalidation error. Explicit `view(slice)` is a
read-only bounded-lifetime shared view; ordinary access and slicing copy.

Generators use `fn gen`,
produce values in a suspendable way, preserve locals between `yield` points,
and implement both iteration contracts. `String` is iterable over its public
character units. Collections offer `map`, `filter` and `reduce` without
mutating the source. The pipe `|>` passes the left-hand result into the next
ordinary function, so pure functions need not be methods on the value's class.

`Range<T>` is lazy, iterable and independent of slicing. `start..end` excludes
the end; `start..=end` includes it. Direction follows the relative bounds; an
explicit non-zero step uses `start..end:step` (or `..=:step`), and a computed
bound or step must be braced, as in `0..{number}:2`. A zero step raises
`InvalidStepError`; a dynamic step whose sign moves away from its end raises
`InvalidRangeDirectionError` before iteration or collection allocation. Slicing uses
`[start:end:step]`, permits omitted or negative components, applies to ordered
collections and `String`, and excludes its end. With positive/omitted step,
omitted `(start,end,step)` default to `(0,length,1)`; with negative step they
default to `(last,before-first,step)`. Thus `[::]` copies all and `[::-1]`
copies in reverse. A zero step or explicitly out-of-range bound is a controlled
error. A slice is a deep independent collection; slice assignment requires the
same number of elements and never resizes.

## 8.1 Temporal values

`Date` is a calendar date; `Time` a clock time; `DateTime` combines them without
a zone; `Instant` is an absolute timeline point; `ZonedDateTime` combines an
instant with an IANA `TimeZone`. They are distinct immutable values in a sealed
`Temporal` capability family. `Date + Time` produces `DateTime`, and assigning
a zone produces `ZonedDateTime`; unrelated combinations are rejected.

Range elements may be integer scalars or `Duration`; range elements in `[...]`,
`Array(...)`, and `List(...)` expand in source order. An unannotated `[...]`
infers `Array<T>`; a `List<T>` destination selects a resizable list. `T[n]`
allocates exactly `n` zero-initialized fixed array slots.

`Duration` is a signed exact timeline quantity with nanosecond precision and a
conceptual `Int128` range. It contains fixed units through weeks and supports
arithmetic, comparison, scalar multiplication/division, ratios and remainder.
Wait/timeout APIs reject negative durations even though temporal differences
may be negative. `Period` contains calendar years, months, weeks and days; it
has no context-free total seconds or ordering. Adding one month to January 31
clamps to the final valid February day; `add_strict` rejects the missing day.

Time zones are canonical IANA zones, not fixed offsets. Ambiguous and
nonexistent local times reject by default; explicit earlier/later and
previous/next-valid policies resolve them. Temporal construction, parsing,
formatting, overflow, invalid dates/times/zones and DST resolution use
controlled typed errors.

## 9. Errors

`Result<T,E>` is `Ok(T) | Error(E)` and is the primary mechanism for expected
failure, analogous to Go's visible value/error outcome without an invalid
two-value state. It must be consumed through exhaustive `match` or a Result
method. Ignoring it is a compile error; `_ = operation()` is explicit discard.
There is no `?` and no implicit conversion to or from an exception. Result
combinators preserve a callback's declared exception effect; wrong-variant
`unwrap` invokes `fatalError`.

Explicit extraordinary exceptions are checked and must be caught or declared
with `throws`. Declared sets participate in `Fn` compatibility. Compiler-known
`RuntimeError` safety failures remain typed and catchable without mandatory
signature declaration.

```text
try {
    execute();
} catch HttpError.Timeout(duration) {
    retry_after(duration);
} catch HttpError(error) {
    stderr.println(error);
} finally {
    metrics.flush();
}
```

Catch uses guard-free patterns and exhausts explicit exceptions not propagated.
`throw;` exactly rethrows the current exception; wrapping creates a new
`Throwable` with the original as `cause`. Thrown values are deeply immutable
identities with code/message, cause, suppressed failures and lazy structured
trace. Direct control transfer in `finally` cannot replace an active outcome.

`fatalError(message)` represents an unrecoverable state and terminates the
process after the diagnostic and whatever safe shutdown is possible. An index
error, division by zero, null or invalid state never becomes undefined
behaviour.

## 10. Modules, project and packages

```text
share class User {}
import { User, Role -> DomainRole } from "./domain/user";
import { stdin, stdout, stderr } from std.io;
```

Local paths use quotes and omit `.zrk`. Standard modules use unquoted names.
`share` publishes code; `import` brings code in; `use` only enables globals.

Importing a compiler-known standard-library object also exposes its declared
convenience members directly when unambiguous. After
`import { stdout } from std.io`, `println("hello")` resolves to
`stdout.println("hello")`. A collision requires qualification. Local and
package objects do not inject methods into file scope.

`init.zrk` is a declarative DSL, not executable code. It contains `project`,
`build_targets`, `globals`, `permissions`, `requires` and dependencies according
to project type. Libraries request authority with `requires`; applications grant
it with `permissions`. Each operation uses `during: build`, `runtime` or `both`;
there is no separate `compile_permissions` block. It contains no global imports and no
arbitrary compiler/runtime configuration.

Permission needs propagate through calls and callable metadata without
appearing in `Fn` syntax. Source declaration is not consent: commands require a
signed external approval bound to project name, canonical location and exact
requester fingerprints. Moves, renames, widening and requester updates require
reapproval; unchanged fingerprints use an incremental fast path. Deployed
programs never prompt and dynamic denial returns typed `PermissionDeniedError`.

## 11. Conversion and casts

Safe conversions use constructors or typed methods that may return a `Result`.
Explicit casts admit postfix and prefix forms:

```text
mut value = source as String;
mut value = <String>source;
mut field = <CustomObject>(obj.field).field;
```

Checkable casts fail in a controlled way. Casts that reinterpret memory or
remove guarantees require `unsafe {}`.

## 12. Decorators and reflection

A non-repeatable decorator uses `fn dec`; grouped decorators use
`repeatable fn dec`. Configuration parameters describe each application and
inner blocks select the only five Zirk 1.x targets: `class`, `attribute`,
`function`, `method`, and `parameter`. Parameter includes initialization
parameters; bodyless method signatures can be inspected but not wrapped.

```text
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

Expansion follows `Inspect -> Augment -> Wrap`. `match target.wrap` selects
`Before()`, `After(result)`, `After(result, transform)`, `Catch(error)`, or
exclusive `Around(next)`. Compiler values exist only through explicit target,
payload, or local bindings. Wrappers preserve callable contracts and expose any
new error or permission effect.

Applications evaluate top-to-bottom and compose outer-to-inner. Optional
`requires decorators [...]`, `before decorators [...]`, and
`after decorators [...]` validate visible source order without reordering it.
Self-reference and dependency cycles are invalid. Contiguous applications of a
`repeatable fn dec` form one ordered expansion with explicit `applications`
payloads and typed `application.arguments`.

Member markers such as `#override` are a distinct built-in surface, not
decorators: a `#` marker is checked directly by the compiler and is never
resolved through `fn dec` lookup or decorator expansion, so the `#` and `@`
surfaces do not collide.

Decorators use an immutable validated Syntax API. Expansion is hygienic,
bounded, source-mapped, deterministic and incrementally fingerprinted. External
effects require approved `during: build` authority. Decorators are erased after
expansion: there is no `runtime fn dec`, automatic annotation retention, or
general `Reflection.decorators(...)`. Frameworks explicitly generate ordinary
typed descriptors and registries. General `comptime {}` remains excluded. The
complete normative contract is `DECORATOR_SEMANTICS.md`.

## 13. Safety and low level

Safe code guarantees the absence of use-after-free, uncontrolled null
dereference, data races and undefined behaviour. Indices are checked except
under provably safe optimization.

```text
unsafe {
    mut pointer: Pointer<Int32> = Pointer.from(value);
    pointer.write(20);
}
```

`unsafe` enables specific operations; it does not disable the type checker,
scopes, mutability or permissions. Safe references are non-null and respect
verified lifetimes. Native interoperability uses the C ABI as its stable
boundary; C++ and Rust expose `extern "C"` wrappers.

Unsafe operations form a closed set: pointer construction/dereference/
arithmetic/casts, unsafe native calls, unchecked native construction, untagged
native-union access, weak atomic ordering, and manual internal safety contracts.
An `unsafe fn` requires unsafe at the call site and still uses visible unsafe
blocks around dangerous expressions in its body.

Ordinary unsafe blocks transactionally isolate writes to managed state and
validated `NativeSliceMut<T>` ranges. Controlled `Error`, exception, checked
trap, or pre-commit cancellation closes newly acquired resources and rolls
those writes back. The compiler/runtime may optimize the journal but cannot
remove the observable atomicity.

Irreversible effects require `commit {}` inside unsafe. Entering the region
publishes pending writes; filesystem/network/process/device effects, unknown
FFI, volatile writes, manual release, concurrent publication, and unbounded raw
writes cannot be promised rollback. Reversible transactions cannot `await`,
spawn work, or expose tentative state. Permissions remain enforced. Full rules
are in `MEMORY_AND_UNSAFE_SEMANTICS.md`.

## 14. Syntax reserved for concurrency

`task`, `task scope`, `await`, `cancellation shield`, `select`, `parallel`,
`parallel for`, `thread`, `Channel<T>`, `sync` and `Atomic<T>` are defined
normatively in `STRUCTURED_CONCURRENCY_SEMANTICS.md` and the runtime
specification. `Task<T>` awaits to exactly `T`; there is no `async fn`, general
detach, or independent `worker`.

`select` chooses one ready task, channel, timer, or cancellation branch fairly,
supports `default`, and leaves losing operations alive. `Task.all`,
`Task.first`, and `Task.settled` make aggregation failure policy explicit.
Internal `Transfer`/`Share` properties are compiler-derived and never ordinary
user-implemented contracts.
