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

## Cross-category invariants

- Values copy independently; whole reference assignment aliases. Projections
  (attribute, index, slice, destructuring, pattern binding, or projected
  capture) are independent values unless used as an assignment place.
- `mut` permits rebinding; `inmut` does not; `inmut::strict` freezes the
  reachable graph.
- Conditions are Boolean. `Result` must be consumed. `throws` is explicit.
  `match with` owns and closes a resource. There is no `?` or general `defer`.
- Ranges use `start..end`, `start..=end`, and `:step`; dynamic operands are
  braced. Do not use `.step()` or `.reverse()`.
- `unsafe` is a constrained boundary and does not bypass types, permissions, or
  ownership rules.

## Before reporting completion

Use the local `./target/debug/zirk` after an appropriate build for every claim
that an example runs. State the exact command and result, or label the example
as normative only.
