# Zirk — Language specification

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
- binary `Float16`, `Float32`, `Float64`, `Float128`, with `Float` aliasing
  `Float64`; an exact base-ten `Decimal` may be a later stdlib type;
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
(`1_000_000`). A fractional literal defaults to `Float64`. `Float` has explicit
positive and negative infinity but no valid `NaN`; division by zero, overflow
and indeterminate results are controlled errors. Duration suffixes are `ns`,
`us`, `ms`, `s`, `m`, `h`, `d` and `w`; calendar months and years use `Period`.

Ordinary overflow produces a controlled error; wrapping, saturating and checked
variants must be explicit operations. Safe widening may be implicit where
unambiguous; signed/unsigned and lossy conversions are explicit. Mixed integer
and Float arithmetic produces Float.

An explicit constructor may establish a deep contextual domain for the
compatible operator tree directly inside it. `Float(3 / 4)` converts operands
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
`break` and `continue` are included. `if` may be an expression when every branch
produces compatible types. The ternary is preferred for a short value choice.
An effect-only `if` may govern one immediate statement without braces:

```text
if closed return;
```

The traditional loop is `for mut i = 0; i < 10; i++ { ... }`. A post-condition
loop is `do { ... } while condition;` and always executes its body once.

`match` is exhaustive when used as an expression:

```text
mut message: String = match result {
    Ok(value) => "Value: {value}";
    Error(error) => "Error: {error}";
};
```

As a statement it controls flow and produces no value. It admits values, types,
associated enums, unions and destructuring. There are no special `capture` or
`yield` forms for recovering its result.

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
mut first_line: String = match with File.open("data.txt") {
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

A closure captures immutable values safely. Shared mutable capture requires that
concurrency analysis prove it safe, or that explicit synchronization be used.
The leading `fn` is optional on a lambda. When a lambda-local name collides with
a capture, `this.name` selects the capture. `clone(receiver.method)` creates a
local callable bound to the same receiver and preserves its complete callable
contract without cloning that receiver or an external handle.

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
field is `public mut`; either modifier may be written to state a non-default
contract explicitly.

A class may declare multiple `construct` members when their effective
parameter signatures differ. Resolution uses arity, type and argument labels,
including reordered named arguments, and rejects duplicate or ambiguous sets.
This exception does not enable ordinary function or method overloading.

A class may extend one class and combine multiple interfaces and traits. Classes
are inheritable by default; `abstract` classes and methods exist, but `final`
does not. Traits may include reusable implementation.

Generics use `<T>` and constraints use `from`:

```text
fn serialize<T from Serializable>(value: T): String { ... }
```

They are specialized for concrete types where appropriate.

Additional data types:

- traditional enums, whose unmapped cases expose their exact names as default
  string values and may map with `->` to compatible string or numeric values;
- algebraic enums with zero or more associated typed values;
- aliases via `type`;
- unions `A | B`;
- immutable records with structural semantics;
- value classes without observable identity, storable inline;
- fixed-length arrays written as `T[]`, canonically sized as `T[n]`, or
  constructed as `Array<T>(n)`; resizable `List<T>`; `Map<K,V>` and `Set<T>`.

A normal class has identity and state; a record represents data; a value class
represents a compact value. `clone()` exists only through an explicit trait and
may be derived when every field is cloneable.

## 8. Iteration and functional style

`Iterable<T>` and `Iterator<T>` define iteration. Generators use `fn gen`,
produce values in a suspendable way, preserve locals between `yield` points,
and implement both iteration contracts. `String` is iterable over its public
character units. Collections offer `map`, `filter` and `reduce` without
mutating the source. The pipe `|>` passes the left-hand result into the next
ordinary function, so pure functions need not be methods on the value's class.

`Range<T>` is lazy, iterable and independent of slicing. `start..end` excludes
the end; `start..=end` includes it. Direction follows the relative bounds,
`.step(distance)` uses a positive non-zero distance, `.reverse()` inverts a
range, and an inline computed bound may be `0..{number}`. Slicing uses
`[start:end:step]`, permits omitted or negative components, applies to ordered
collections and `String`, and excludes its end.

## 8.1 Temporal values

`Date` is a calendar date; `Time` a clock time; `DateTime` combines them without
a zone; `Instant` is an absolute timeline point; `ZonedDateTime` combines an
instant with an IANA `TimeZone`. They are distinct immutable values in a sealed
`Temporal` capability family. `Date + Time` produces `DateTime`, and assigning
a zone produces `ZonedDateTime`; unrelated combinations are rejected.

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

`Result<T,E>` is the primary mechanism for expected failures. It is handled
explicitly with `match`; there is no `?`.

Exceptions are exceptional but recoverable:

```text
try {
    execute();
} catch<HttpError> error {
    stderr.println(error);
} default error {
    stderr.println(error);
} finally {
}
```

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
`build_targets`, `globals`, `permissions`, `compile_permissions`, `requires` and
dependencies according to the project type. It contains no global imports and no
arbitrary compiler/runtime configuration.

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

A native decorator is declared with `fn dec`. Its outer parameters configure the
decorator and its inner blocks determine the admissible targets:

```text
fn dec route(path: String) {
    class(target) {
        // logic for classes
    }

    method(target) {
        // logic for methods
    }
}
```

One decorator may implement several targets. Each block receives a fixed, typed
contextual API. Decorators run at compile time, may read metadata and perform
transformations through a controlled Syntax API; they receive no arbitrary
access to the internal AST or to the system.

Basic type identity always exists. Advanced structural reflection is preserved
only when a type or decorator requests it. General compile-time reflection
happens inside decorators; there is no general `comptime {}` in 1.x.

## 13. Safety and low level

Safe code guarantees the absence of use-after-free, uncontrolled null
dereference, data races and undefined behaviour. Indices are checked except
under provably safe optimization.

```text
unsafe {
    mut pointer: Pointer<Int32> = &value;
    *pointer = 20;
}
```

`unsafe` enables specific operations; it does not disable the type checker,
scopes, mutability or permissions. Safe references are non-null and respect
verified lifetimes. Native interoperability uses the C ABI as its stable
boundary; C++ and Rust expose `extern "C"` wrappers.

## 14. Syntax reserved for concurrency

`task`, `await`, `parallel`, `parallel for`, `thread`, `Channel<T>`, `sync` and
`Atomic<T>` are defined normatively in the runtime specification. There is no
`async fn` and no independent `worker`.
