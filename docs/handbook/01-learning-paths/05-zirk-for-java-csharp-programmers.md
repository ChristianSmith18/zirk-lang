# Zirk for Java and C# Programmers

Zirk's classes, interfaces, static types, exceptions, generics, and brace-delimited syntax will feel familiar. The largest differences concern native distribution, algebraic data, explicit capability contracts, and concurrency.

## Values share an object model without uniform allocation

Every value belongs semantically to a class, but the compiler may represent integers and other simple values inline. Do not infer heap allocation merely because operations are methods or contracts belong to types.

Zirk adds records, traits, unions, and enums with associated values. These often model closed data more precisely than a class hierarchy:

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

There is no public global event loop and no `async fn` in Zirk 1.x. A `task` belongs to a scope, cancellation propagates through the structure, `parallel` requests multicore CPU work, and `thread` represents a real system thread.

## Applications decide capabilities

Libraries cannot grant themselves filesystem or network access. They declare requirements; applications grant the final finite permission set in `.zkinit`. Compile-time decorators have a separate permission boundary.

## Recommended route

Focus on immutability levels, algebraic enums and matching, `Result`, resource-safe control flow, structured concurrency, permissions, and portable packages. Then use the class and interface chapters to map the familiar parts precisely.

---

**Previous:** [← Zirk for Rust, C, and C++ Programmers](04-zirk-for-rust-c-cpp-programmers.md) · **Next:** [ Tooling in Five Minutes](06-tooling-in-five-minutes.md)
