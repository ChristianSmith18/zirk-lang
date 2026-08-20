# Current Limitations

Zirk's final 1.x semantics are broader than the current compiler. The repository
has a precedence table, stable diagnostic-code policy, and normative detail for
ranges, slicing, traditional `for`, patterns, value classes, generators, and
callable types. Their existence in documentation does not imply every pipeline
stage implements them.

Current high-impact delivery limits include:

- `Fn(P...) => R` is final syntax but remains rejected in general type
  positions; closures therefore cannot yet escape their creating function.
- Phase 4a–4c implement expected errors, explicit exceptions, and the initial
  one-resource `match with`, but final suppressed/combined cleanup failures,
  grouped acquisition, complete stack traces, throwable immutability,
  cancellation cleanup, resource transfer, and dependent lifetimes remain.
- Managed memory, weak/dependent references, full deep-graph cloning,
  `inmut::strict` alias enforcement, transactional unsafe rollback, and
  irreversible `commit` effects remain Phase 4e work.
- Several Phase 3 constructs parse and type-check more broadly than they lower:
  user generic contracts/enums, abstract-class dynamic dispatch, value-type
  contract dispatch, and derived structural equality require remaining stages.
- `Float128` arithmetic lacks complete Windows verification and `Float128`
  currently lacks `to_string()` support.
- Standard-library, structured-concurrency, packaging, developer-tooling,
  decorator, and public documentation surfaces are specified ahead of full
  compiler/runtime delivery.

Zirk 1.x explicitly excludes browser/WebAssembly, public runtime directives or
event loop, standalone `worker`, `async fn`, textual inline assembly, general
`comptime`/`defer`, multiple class inheritance, traditional overloads, Result
`?`, and public ownership/reference-counting semantics.

---

**Previous:** [← Differences from Rust](06-differences-from-rust.md) · **Next:** [ Roadmap](08-roadmap.md)
