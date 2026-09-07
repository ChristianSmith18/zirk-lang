# Roadmap

The handbook targets normative Zirk 1.x; the detailed construction and status
source is [`docs/init/ZIRK_ROADMAP.md`](../../init/ZIRK_ROADMAP.md). Phase 0,
1, 2, 3, 3b, and 4a–4e have completed scoped delivery changes, and the
`array-list-tuple-duration-regex` change delivers the everyday data surface
(`Tuple`, `Array<T>`, `List<T>`, `Duration`, `Regex`, the `String`/`Char`
method surface, and `type` alias lowering) while removing `value class`.
“Scoped” matters: `Range<T>` and `Regex.split`/`String.split` remain
explicitly assigned to the same or later changes. The Phase 3 OOP closing
slice delivered `static` members on `class`, field initializers, the `as?`
nullable cast, `TraitName.super.method()`, contract composition via
`implements` on contracts, type-parameter defaults on generic types,
comparison operators as contracts, generic contract dispatch for classes
and records, and positional variance verification. Still assigned to later
changes: trait defaults under generic substitution, nested contract
arguments (`Container<Box<T>>`), type-parameter defaults and bodies for
user-defined generic functions, and `static` on `record`/`enum`.

The remaining sequence moves through structured concurrency,
projects/permissions, the rest of the standard library, functional style,
packaging, developer tooling, decorators, hardening, and eventual
self-hosting. Every feature requires one owning phase, end-to-end tests,
diagnostics, and a documentation/status update.

Future features require an explicit specification change and compatibility
analysis. Excluded 1.x ideas are not automatically roadmap commitments, and a
completed documentation design is not an implementation-complete claim.

---

**Previous:** [← Current Limitations](07-current-limitations.md) · **Next:** [ Normative Sources](09-normative-sources.md)
