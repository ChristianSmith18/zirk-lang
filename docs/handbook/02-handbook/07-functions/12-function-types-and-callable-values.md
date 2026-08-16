# Function Types and Callable Values

Use `Fn(P...) => R` when behavior must be stored, passed, or returned. It is the
preferred exact alias of `Function(P...) => R`.

```zirk
fn parse_user(source: String) => Result<User, ParseError> { ... }

inmut parser: Fn(String) => Result<User, ParseError> = parse_user
inmut printer: Fn(String) => Void = stdout.println
```

Named functions, lambdas, bound methods, unbound methods, and explicitly
callable objects adapt to the same type when the complete signature matches.
This is callable compatibility, not traditional overload resolution.

## Signature components

```zirk
inmut decode: Fn(source: String, radix?: Int, ...flags: String)
    => Result<Int, ParseError>
```

Labels participate in named calls. `?` records optionality and `...` records a
variadic tail. A concrete default expression belongs to a declaration, not to
the function type. Parameters are contravariant and results covariant: a value
is assignable only when it can safely accept every call promised by the target
and return a value no broader than the target promises.

```zirk
inmut render: Fn(Object) => String = object_to_string
inmut names: Fn(User) => Object = render // valid variance direction
```

Zirk does not partially apply missing arguments. Wrap the call in a lambda when
you want a smaller callable.

```zirk
inmut add: Fn(Int, Int) => Int = (left, right) => left + right
inmut add_two: Fn(Int) => Int = value => add(value, 2)
```

## Bound and unbound methods

Reading `stdout.println` as a callable binds `stdout` as the receiver. A bound
callable therefore needs only the declared method parameters. An unbound method
reference includes the receiver first. The compiler tracks receiver mutation
internally; users do not add a special marker to `Fn`.

`mut print = stdout.println` is valid and permits rebinding `print`; it does not
make the bound receiver mutable by itself.

## Identity and copying

Assigning a callable shares its closure environment and bound receiver.
`clone()` creates a deep independent environment when all captured parts are
cloneable. Use `is` to ask whether two variables hold the same callable
identity. There is no structural `==` for executable behavior.

## Failure and purity

Expected failure is expressed in the return type, for example
`Fn(String) => Result<User, ParseError>`. `Fn` does not imply purity. A callable
may mutate permitted state or perform effects unless a separate contract says
otherwise.

### Invalid forms

```zirk
inmut bad: Fn(Int) => Int = (value: String) => value.length // parameter mismatch
inmut partial = add(2)                                     // no partial application
inmut same = add == add                                    // no callable equality
```

---

**Previous:** [← Never-Returning Functions](11-never-returning-functions.md) · **Next:** [Generics →](../11-generics/README.md)
