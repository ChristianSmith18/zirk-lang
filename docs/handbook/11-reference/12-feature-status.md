# Feature Status

Status values are **specified and implemented**, **partially implemented**, **specified but not implemented**, **historical/exploratory**, or **explicitly excluded**. A repository revision and evidence are required before claiming implementation.

The type taxonomy, Float family, grapheme Char, shared mutable String, binding permissions, native operator rules, and temporal family in this handbook are the authorial language definition even where compiler delivery is pending.

| Area | Language definition | Implementation tracking |
|---|---|---|
| scalar and special types | defined | compiler roadmap and phases |
| shared references and strict aliases | defined | Phase 3 and later validation |
| user-defined types and contracts | defined | Phase 3 |
| collections and generics | defined | phased compiler work |
| temporal family | defined | later standard-library/runtime phase |

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

**Previous:** [← Standard Library Index](11-standard-library-index.md) · **Next:** [ Type Member Index](13-type-member-index.md)
