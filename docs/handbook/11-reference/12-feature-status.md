# Feature Status

Status values are **specified and implemented**, **partially implemented**, **specified but not implemented**, **historical/exploratory**, or **explicitly excluded**. A repository revision and evidence are required before claiming implementation.

The type taxonomy, Float family, grapheme Char, shared mutable String, binding
permissions, native operators, temporal family, callable types, escaping
closures, projection-copy semantics, object/contract model, complete generics,
algebraic data, collections, and matching rules are authorial definitions even
where compiler delivery is pending.

The same applies to mandatory `Result`, checked explicit exceptions, implicit
typed runtime exceptions, patterned catch, resource responsibility, combined
cleanup failure, the two-block permission model, signed location-bound consent,
and incremental requester-aware validation.

| Area | Language definition | Implementation tracking |
|---|---|---|
| scalar and special types | defined | compiler roadmap and phases |
| shared references and strict aliases | defined | Phase 3 and later validation |
| user-defined types and contracts | defined | Phase 3 |
| callables and escaping closures | defined | later than Phase 3 |
| collections and generics | defined | phased compiler work |
| tuples, records, enums, unions and match | defined | phased compiler work |
| temporal family | defined | later standard-library/runtime phase |
| Result and exception model | defined | later compiler/runtime phase |
| deterministic resources | defined | later compiler/runtime phase |
| permissions and secure approval | defined | later compiler/package tooling phase |

Explicit Zirk 1.x exclusions include browser/WebAssembly, public runtime directives, standalone `worker`, `async fn`, public event loop, textual inline assembly, general `comptime`, general `defer`, multiple class inheritance, traditional function overloading, `Result` propagation `?`, and public ownership/RC semantics.

The intended surface now includes exponentiation, descending and stepped ranges,
Python-style slicing, classic and iterable `for`, `do ... while`, regex literals,
optional-`fn` lambdas, constructor signatures, mapped traditional enums,
generators, comma-grouped patterns, tuple/record destructuring, `Fn` callable
values, and bound methods. Compiler support may trail this target; consult milestone diagnostics
rather than treating absence in the current parser as a language exclusion.

Remaining implementation artifacts include the complete machine-readable
grammar and stable diagnostic-code assignments. They do not leave the language
semantics in this checkpoint open.

---

**Previous:** [← Standard Library Index](11-standard-library-index.md) · **Next:** [ Type Member Index](13-type-member-index.md)
