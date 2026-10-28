# Zirk for Java and C# Programmers

Zirk's classes, structural contracts, static types, exceptions, generics, and brace-delimited syntax will feel familiar. The former `interface` and `record` declaration forms were removed; the largest differences concern native distribution, algebraic data, explicit capability contracts, and concurrency.

## Values share an object model without uniform allocation

Every value belongs semantically to a class, but the compiler may represent integers and other simple values inline. Do not infer heap allocation merely because operations are methods or contracts belong to types.

Zirk adds tuples, traits, unions, and enums with associated values. These often model closed data more precisely than a class hierarchy:

```zirk
mut message: String = match result {
    Ok(value) => "Value: {value}";
    Error(error) => "Error: {error}";
};
```

## Failure has three channels

Expected failure uses `Result<T, E>`. Exceptions represent exceptional but recoverable situations. `fatalError` is reserved for irreparable states. Learn the decision rule before translating exception-heavy APIs.

Zirk explicit exceptions are checked with `throws`; built-in safe-runtime
exceptions remain catchable but need not pollute every signature. Catch uses
pattern form `catch Type(binding)`. Resource close errors and suppressed
failures are typed instead of relying on finalizers.

## Concurrency is structured

There is no public global event loop and no `async fn` in Zirk 1.x. Structured
concurrency uses `concurrent { ... }`, `spawn`, and `Job<T>`; cancellation is
cooperative. `parallel` requests CPU work that may use the worker pool. The old
`task` syntax was removed. Managed `Thread.run`, scalar atomics, and scoped
`Mutex<T>.with` are delivered; general thread handles and other synchronizers
remain specified only.

## Applications decide capabilities

Applications and packages declare the same finite `requires` capabilities. A declaration is not consent: the user approves an exact resource when protected code reaches it. The current compiler does not expose protected host operations to decorators, and there is no separate compile-time permission phase.

## Recommended route

Focus on immutability levels, algebraic enums and matching, `Result`, resource-safe control flow, structured concurrency, capability requirements, and portable packages. Then use the class, contract, and trait chapters to map the familiar parts precisely.

---

**Previous:** [← Zirk for Rust, C, and C++ Programmers](04-zirk-for-rust-c-cpp-programmers.md) · **Next:** [ Tooling in Five Minutes](06-tooling-in-five-minutes.md)
