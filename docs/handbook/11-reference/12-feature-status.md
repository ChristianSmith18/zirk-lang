# Feature Status

Status values are **specified and implemented**, **partially implemented**, **specified but not implemented**, **historical/exploratory**, or **explicitly excluded**. A repository revision and evidence are required before claiming implementation.

Explicit Zirk 1.x exclusions include browser/WebAssembly, public runtime directives, standalone `worker`, `async fn`, public event loop, textual inline assembly, general `comptime`, general `defer`, multiple class inheritance, traditional function overloading, `Result` propagation `?`, and public ownership/RC semantics.

The intended surface now includes exponentiation, descending and stepped ranges,
Python-style slicing, classic and iterable `for`, `do ... while`, regex literals,
optional-`fn` lambdas, constructor signatures, mapped traditional enums,
generators, comma-grouped patterns, nested destructuring, and callable method
cloning. Compiler support may trail this target; consult milestone diagnostics
rather than treating absence in the current parser as a language exclusion.

Remaining documentation gaps are the complete machine-readable grammar,
numeric precedence levels, stable diagnostic-code assignments, and exhaustive
runtime/API details for features whose implementations are still scheduled.

---

**Previous:** [← Standard Library Index](./11-standard-library-index.md) · **Next:** Explanations *(next section)*
