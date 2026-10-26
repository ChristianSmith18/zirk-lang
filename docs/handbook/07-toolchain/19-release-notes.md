# Release Notes

## Unreleased

### Type algebra and narrowing

- Added recursive n-ary unions and intersections, canonical singleton values,
  compile-time constant evaluation, generic aliases, structural shapes, flow
  narrowing, and erased generic type operators.
- Added semantic IDE queries for canonical types, singleton values, structural
  members, generic call substitutions, and flow-refined types.
- Added inline native tagged-union execution for tested scalar/string,
  temporal-singleton, and enum-constant cases. CLI parity fixtures cover
  `Int32 | String`, mixed String/numeric/Boolean singleton alternatives,
  nullable alternatives, enum singleton alternatives, and Duration/Date/
  Float/Decimal/Char singleton alternatives in both native and interpreter
  engines.
- Made type-relation and constant-evaluation limits deterministic and
  source-located. `E0482` reports bounded type computation failures.

The type system has broader semantic support than current runtime lowering.
Structural shape values, some nominal/scalar union combinations, cross-target
runtime ABI execution, complete interpreter parity, and GC stress at additional
specialized managed-container boundaries remain incomplete. The currently
covered GC boundaries include suspended tasks, channels, and caught exceptions.
See the [feature status](../../init/ZIRK_FEATURE_STATUS.md) and [current
limitations](../13-appendices/07-current-limitations.md) for the tested
boundaries.

---

**Previous:** [← Performance Goals](18-performance-goals.md) · **Next:** [Testing →](../08-testing/README.md)
