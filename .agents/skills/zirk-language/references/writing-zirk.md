# Cross-category writing checklist

This is a compact bridge for a request that spans several language categories.
For code, load the focused references from `SKILL.md`; they contain the
decision tables and current range/collection forms.

## Before authoring

1. Identify the output value, its type, and whether it is fixed-size,
   resizable, nullable, or owned.
2. Choose the simplest matching construct: a range for a regular sequence,
   `List<T>` for a resizable collection, `Array<T>` for fixed size, a `record`
   for immutable data, or a `class` for identity and state.
3. Choose explicit failure and authority handling before adding I/O, resources,
   network, processes, or native code.
4. Read the owning handbook source for a concrete standard-library API.

## Idiomatic local code

- Prefer `inmut name = expression;` for stable bindings when the type permits
  the intended operations. Use `mut` for rebinding, definite initialization,
  or writes through a value-semantic place that require it. A mutable
  reference-backed referent alone does not require `mut`.
- Keep a local annotation when it carries intent the initializer cannot
  express: public contract, chosen numeric width, nullable/empty generic shape,
  disambiguation, or an explicitly preserved `Result<T, E>` for error handling.
- Use `as` only for the language's checked casts between related types. It is
  not a generic type assertion and does not replace a documented value
  conversion such as `T(value)`.
- Choose `match` when error identity matters. Use contextual Result projection
  to `T?` and `??` only when collapsing all `Error` cases is intended.
- Before presenting an API example, confirm its current signature in the
  owning handbook and check every operation against the current contract. Do
  not copy removed aliases or helpers from older docs, fixtures, or languages.

## Cross-category invariants

- Values copy independently; whole reference assignment aliases. Projections
  (attribute, index, slice, destructuring, pattern binding, or projected
  capture) are independent values unless used as an assignment place.
- `mut` permits rebinding; `inmut` does not; `inmut::strict` freezes the
  reachable graph.
- Conditions are Boolean. `Result` must be consumed. `throws` is explicit.
  `match with` owns and closes a resource. There is no `?` or general `defer`.
- Result projection may turn every `Error` into `null`; explicit `match`
  preserves the error. `get` is the sole safe collection lookup API where it
  returns `Result`; nullable-only operations without a Result twin remain valid.
- Ranges use `start..end`, `start..=end`, and `:step`; dynamic operands are
  braced. Do not use `.step()` or `.reverse()`.
- `unsafe` is a constrained boundary and does not bypass types, permissions, or
  ownership rules.

## Before reporting completion

Use the local `./target/debug/zirk` after an appropriate build for every claim
that an example runs. State the exact command and result, or label the example
as normative only.

When auditing examples, search for removed Result/collection helpers, Result
unwrap calls, unnecessary local annotations, casts that merely restate a type,
and `mut` bindings that have no demonstrated need. Check whether the binding
is reassigned, definitely initialized later, or needs writes through a
value-semantic place; a stable reference may still expose mutating methods.
Decide from the actual type and contract before editing: a conversion,
deliberate width, public signature,
empty generic construction, explicit Result preservation, or assertion helper
may be intentional.
