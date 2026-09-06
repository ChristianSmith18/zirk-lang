## Why

The handbook and the `zirk-data-types` spec already promise built-in static members on enum types — `EnumType.keys()`, `EnumType.values()`, `EnumType.count`, `EnumType.from_name()`, and `EnumType.from_value()` — but the compiler rejects every one of them as an unknown member. The documented reflection/conversion surface of enums is therefore unusable, and the previous native-type audit left this as a known gap.

## What Changes

- `EnumType.count` — `Int32` (or `UInt64`, following the `.length` convention) constant equal to the number of variants.
- `EnumType.keys()` — `List<String>` (or `Array<String>`, per the handbook's `[...]` display) with the variant names in declaration order.
- `EnumType.values()` — `List<EnumType>` (or `Array<EnumType>`) with the enum values in declaration order.
- `EnumType.from_name(name)` — `Result<EnumType, EnumLookupError>` (or `EnumType?`, per the handbook's `Ok/Err` shape) resolving a case name at run time.
- `EnumType.from_value(value)` — `Result<EnumType, EnumLookupError>` resolving a mapped case value at run time (traditional enums only).
- `Enums.keys(E)` / `Enums.values(E)` — generic helpers mirroring the handbook when the enum type is not known statically.
- All of the above are evaluated at compile time where possible: `count`/`keys`/`values` lower to constant construction; `from_name`/`from_value` lower to the equivalent of an exhaustive `match`.
- Semantics follow the handbook exactly: declaration order, no ordering implication, `Ok`/`Err` results, no implicit conversions.

No existing valid program becomes invalid; the members did not resolve before.

## Capabilities

### New Capabilities

- `enum-static-members`: static `count`, `keys()`, `values()`, `from_name()`, `from_value()` on declared enum types, plus the `Enums` generic helpers.

### Modified Capabilities

- `zirk-data-types`: the "Closed data-only enums" requirement already names `from_name()`/`from_value()`; the delta narrows the exact surface (return types, algebraic-enum applicability) without relaxing it.

## Impact

- `crates/zirk-sema/src/checker.rs`: static-member dispatch on `Base::Enum` receivers (the path `EnumType.member`), argument checking for `from_name`/`from_value`, generic-enum handling.
- `crates/zirk-ir/src/ir.rs` / `lower.rs` / `verify.rs`: new instructions or compile-time expansion for the members.
- `crates/zirk-codegen-llvm` / `crates/zirk-runtime`: runtime lookup helpers for `from_name`/`from_value` if not fully expanded at compile time.
- `crates/zirk-cli/tests/corpus/`: end-to-end fixtures.
- `docs/init/ZIRK_FEATURE_STATUS.md`, handbook enum pages, and `../zirk-lang-site` after commit.
