# Data Types

Zirk offers several ways to model domain data. Contracts describe structural requirements, `Object` values hold open data, tuples model positional aggregates, classes model identity and lifecycle, enums define closed alternatives, aliases name an existing type, and unions admit explicitly listed types. The former `record` declaration form was removed.

Choosing among them determines construction, equality, identity, exhaustiveness, and API compatibility—not merely spelling.

Type aliases can name normalized [unions](07-union-types.md) and
[intersections](09-intersection-types.md); intersections combine compatible
requirements, while unions represent alternatives.

---

**Previous:** [← Generic Type Operators](../11-generics/09-type-operators.md) · **Next:** [Tuples →](00-tuples.md)
