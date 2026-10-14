## ADDED Requirements

### Requirement: Lowering preserves checked named-call bindings

Lowering SHALL use the checker-resolved parameter and constructor binding for
every call form. It SHALL evaluate explicit argument expressions in source
order, then pass their values in the selected declaration's parameter order;
default, optional, variadic, and branch-spilling behavior SHALL remain
equivalent to an otherwise identical positional call.

#### Scenario: Reordered constructor arguments reach their parameters

- **WHEN** a constructor `(id: Int32, name: String, age: Int32)` is called as
  `Employee(age: 2, name: name, id: 45)`
- **THEN** the emitted call passes `45`, `name`, and `2` to `id`, `name`, and
  `age` respectively

#### Scenario: Reordered super method arguments reach their parameters

- **WHEN** `super.rename(suffix: "SUF", prefix: "PRE")` targets a method
  declared `(prefix: String, suffix: String)`
- **THEN** the emitted call passes `"PRE"` as `prefix` and `"SUF"` as `suffix`
