## Context

See `proposal.md` and the delta specs. The parser already models primitive literal atoms directly, while `const(...)` routes other singleton values through the general expression parser. The change must preserve runtime temporal constructors while making singleton type syntax literal-only.

## Goals / Non-Goals

**Goals:**
- Make a type alternative either a type expression or a literal atom; never require a wrapper or function call to express a singleton.
- Give Date, Time, and local DateTime a compact, unambiguous ISO literal spelling in type position.
- Preserve exact owning-type identity, source spans, bounded normalization, diagnostics, and existing expression-constructor behavior.
- Reject `const(...)` and arbitrary call-shaped expressions in type position; only fixed built-in temporal constructors with direct integer components are permitted.

**Non-Goals:**
- Add temporal literal syntax to ordinary expression position in this change; existing `Date(...)`, `Time(...)`, and `DateTime(...)` runtime construction remains unchanged.
- Permit arithmetic, function evaluation, clock values, or arbitrary expressions as singleton type atoms.
- Introduce zoned temporal or user-defined constructor-call syntax as a literal.

## Decisions

1. **The type grammar accepts literal atoms, not constant expressions.** Primitive values, duration, null, and enum cases use direct forms in type position. Temporal singleton values must use the owning `Date(...)`, `Time(...)`, and `DateTime(Date(...), Time(...))` forms with direct integer components only; bare ISO forms are rejected in type position. Tuple and object types continue to use type syntax. `const(...)`, arbitrary function calls, and other expressions are rejected there with a focused diagnostic. The fixed temporal constructor forms are parsed into temporal literal atoms, without invoking the general expression parser or constant evaluator. The bounded evaluator may remain as an internal implementation facility for other compiler computations, but it is no longer a source-level singleton-type entry point.

2. **Temporal singleton values use their owning type constructors.** Recognize only `Date(year, month, day)`, `Time(hour, minute, second?, nanosecond?)`, and `DateTime(Date(...), Time(...))` with literal integer arguments. Canonicalize them with strict calendar/time validation. DateTime is local and has no zone. These forms are type-position atoms only in this change, avoiding changes to expression precedence or runtime code generation. Bare ISO-shaped forms such as `2026-12-25`, `09:30`, and `2026-12-25 09:30` are rejected in type position.

3. **Tuple and object values remain represented by type expressions.** Tuple type arguments and object property shapes are parsed recursively. Their properties may use direct singleton literals, as in `{ active: true; retries: 3 }`. A tuple/object constructor call is not accepted as a singleton type atom.

4. **Enum cases are direct qualified value atoms.** A qualified enum case such as `Shade.Red` remains a name-like type atom and resolves to the enum's canonical case value. No constructor call is accepted as part of that atom.

5. **Historical archives remain immutable.** Update active specs and current handbooks/examples. The archived `native-type-algebra-and-narrowing` change continues to describe the syntax that existed when it was completed; the new change records its replacement.

## Risks / Trade-offs

- [A bare date resembles subtraction] → Require `Date(...)` in type position and reject bare ISO forms there; retain exact constructor spans.
- [Malformed temporal forms could be misparsed] → Validate fixed integer components in temporal constructors and report source-located errors for invalid calendar or clock fields.
- [Object and tuple literals overlap type-shape syntax] → Keep one recursive parser path whose leaves are types or literals; test literal singleton equality separately from structural assignability.
- [Users may rely on `const(...)` type aliases] → Emit a migration diagnostic with direct scalar examples and owning temporal constructor examples; ordinary runtime temporal constructors are unaffected.

## Migration Plan

Replace all active `const(...)` singleton declarations with direct literal forms, update tests and demos, and synchronize the canonical handbook to the companion website after the repository changes are committed and both repositories meet the sync script's cleanliness requirements. No migration is needed for runtime constructor calls.
