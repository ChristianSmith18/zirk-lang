# Current Limitations

Zirk's final 1.x semantics are broader than the current compiler. The repository
has a precedence table, stable diagnostic-code policy, and normative detail for
ranges, slicing, traditional `for`, patterns, value classes, generators, and
callable types. Their existence in documentation does not imply every pipeline
stage implements them.

Current high-impact delivery limits include:

- `Fn(P...) => R` is parseable and type-checked in every position (Phase 4d),
  and a named function, capture-less lambda, or a single capturing closure
  literal written directly at a local's initializer or a function's `return`
  freely satisfies it — but general callable-type polymorphism across two or
  more *differently-captured* closures at one position still needs the
  captures heap-boxed behind a uniform representation, not built yet.
- Phase 4a–4c implement expected errors, explicit exceptions, and the initial
  one-resource `match with`, but final suppressed/combined cleanup failures,
  grouped acquisition, complete stack traces, throwable immutability,
  cancellation cleanup, resource transfer, and dependent lifetimes remain.
- Managed memory (ADR-003's strategy decision remains open), weak/dependent
  references, and full deep-graph cloning remain Phase 4e work.
  `inmut::strict` rejects a direct rebinding and a write through a field
  projection off a strict binding; strictness declared on a field itself
  (independent of its container's own mutability) and a mutating method call
  reached through a strict reference remain open. `unsafe fn`/`unsafe {}`/
  `commit {}` parse and are context-checked; `Pointer<T>` (an ABI-safe
  element-type subset) supports construction/read/write/offset/cast with a
  conservative escape rule; `extern "C" fn` declares and calls a native
  function under a narrow ABI-safe signature. The transactional
  journal/rollback contract itself is not implemented — `unsafe {}` does not
  yet record writes and `commit {}` does not yet durably publish anything, so
  nothing rolls back on failure yet. `NativeSlice<T>`/`NativeSliceMut<T>`,
  volatile access, untagged native-union access (no union type exists), weak
  atomic ordering (`Atomic<T>` is Phase 5), and a native-library-linking
  manifest also remain.
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
