# Zirk — Language specification

This document defines the public semantics of `.zrk` source code.

## 1. Lexical structure

Zirk is case-sensitive. Blocks use `{}`. The semicolon is optional for the
parser where there is no ambiguity, but the official formatter adds it. Both
`//` and `/* ... */` are supported; documentation uses multi-line documentation
comments.

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

`mut` allows reassignment. `inmut` freezes the reference and respects the
mutability of the referenced type. `inmut::strict` prevents transitive
modification. Uppercase is a convention, not semantics encoded in the name.

Inference is allowed where it is unambiguous. Every type has a default value;
flow analysis prevents reading a variable that is not yet available.

There are block, function, file and module scopes. A file symbol does not leave
it unless published with `share`.

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

Typing is static with inference. Every value belongs to a semantic class, even
though the compiler may represent it without allocation.

Fundamental families:

- `Int8`, `Int16`, `Int32`, `Int64`, `Int128`, plus the aliases `Int`/`Integer`
  for the default signed integer.
- `UInt8`, `UInt16`, `UInt32`, `UInt64`, `UInt128`.
- `Decimal16`, `Decimal32`, `Decimal64`, `Decimal128`, plus the aliases
  `Dec`/`Decimal` for the default decimal.
- `Boolean`, exclusively `true` or `false`.
- `Char`, a Unicode code point.
- `String`, a Unicode sequence indexed semantically by graphemes and with an
  adaptive internal index/cache.
- `Void`, `Never`, `Null`, `Object` and the collection types.

There is no numeric truthiness. `Boolean?` admits `null`; `Boolean` does not.
There is no `undefined`.

Literals admit scientific notation (`1e2`) and `_` as a separator
(`1_000_000`). Durations are typed literals such as `5000ms`, `5s`, `5m` and
`5h`; the runtime may normalize them internally.

Ordinary overflow produces a controlled error; wrapping, saturating and checked
variants must be explicit operations. Conversions that may lose information are
not implicit.

## 4. Nullability, equality and operators

`T?` is equivalent to `T | Null`. Safe access uses `?.` and the fallback value
uses `??`.

- `==` and `!=`: structural equality.
- `is`: same instance, only for types with observable identity.
- Comparators: `<`, `<=`, `>`, `>=`, per the contracts of the type.
- Logical: `&&`, `||`, `!`, booleans only.
- Arithmetic and compound: `+`, `-`, `*`, `/`, `%`, `+=`, `-=`, `*=`, `/=`, `%=`.
- Increment: `count++`, `count--`, `++count`, `--count`, preserving conventional
  postfix/prefix semantics.
- Ternary: `condition ? when_true : when_false`.

Operators may only be overloaded through contracts defined by the language; an
overload cannot alter precedence or arity.

## 5. Control flow and pattern matching

`if`/`else`, `for`, `for ... in`, `while`, `loop`, `break` and `continue` are
included. `if` may be an expression when every branch produces compatible types.

`match` is exhaustive when used as an expression:

```text
mut message: String = match result {
    Ok(value) { "Value: {value}" }
    Error(error) { "Error: {error}" }
};
```

As a statement it controls flow and produces no value. It admits values, types,
associated enums, unions and destructuring. There are no special `capture` or
`yield` forms for recovering its result.

`match with` acquires a `Resource<E>` and guarantees it is closed when any
branch ends, including error, exception, `return` or cancellation:

```text
mut first_line: String = match with File.open("data.txt") {
    Ok(file) { file.read_line() }
    Error(error) { "" }
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

Local inference, optional parameters (`name?`), nullable types (`String?`),
default values, named parameters and variadics (`...values`) are supported.
There is no traditional overloading; unions, generics or different names are
used instead.

Lambdas are equivalent to function values:

```text
inmut ADD = (a: Int32, b: Int32): Int32 => a + b;
inmut ACTION = (): Void => {
    stdout.println("ok");
};
```

A closure captures immutable values safely. Shared mutable capture requires that
concurrency analysis prove it safe, or that explicit synchronization be used.

## 7. Objects and data types

```text
class User implements Serializable {
    public inmut id: UInt64;
    public mut name: String;

    construct(id: UInt64, name: String) {
        this.id = id;
        this.name = name;
    }
}

mut user = User(1, "Cristian");
```

The constructor is called `construct`; there is no `new`; the current instance
is `this`. Visibility is `public`, `private` or `protected`, with `public` as
the default.

A class may extend one class and combine multiple interfaces and traits. Classes
are inheritable by default; `abstract` classes and methods exist, but `final`
does not. Traits may include reusable implementation.

Generics use `<T>` and constraints use `from`:

```text
fn serialize<T from Serializable>(value: T): String { ... }
```

They are specialized for concrete types where appropriate.

Additional data types:

- traditional enums and algebraic enums with associated values;
- aliases via `type`;
- unions `A | B`;
- immutable records with structural semantics;
- value classes without observable identity, storable inline;
- dynamic arrays, fixed arrays, `List<T>`, `Map<K,V>` and `Set<T>`.

A normal class has identity and state; a record represents data; a value class
represents a compact value. `clone()` exists only through an explicit trait and
may be derived when every field is cloneable.

## 8. Iteration and functional style

`Iterable<T>` and `Iterator<T>` define iteration. Generators use `fn gen` and
produce values in a suspendable way. Collections offer `map`, `filter` and
`reduce` without mutating the source. The pipe `|>` passes the left-hand result
into the next operation.

`Range<T>` is iterable and independent of slicing. Inclusive and exclusive
ranges defined by their constructor are supported, as is `[start:end:step]`
slicing.

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
