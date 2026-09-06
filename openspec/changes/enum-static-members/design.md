# Design: enum static members

## Surface

`E.count` (property, `Int32`), `E.keys()` → `List<String>`, `E.values()` → `List<E>`, `E.from_name(name: String)` → `Result<E, LookupError>`, `E.from_value(v)` → `Result<E, LookupError>` (traditional enums only), and `Enums.keys/values/count(E)` generic helpers.

## Checker

`Direction.North` today resolves through `variant_accesses` — the same `Path`-receiver dispatch point gains the static members before variant lookup: `count` is a property, the rest calls. `from_name`/`from_value` return `Result<E, LookupError>`; `LookupError` is registered in `register_native_exception_hierarchy` like `ParseError`/`RegexError` and added to `NativeExceptions`, `synthesize_native_failure_bodies`, and `NATIVE_FAILURE_CODES` (12 → 13). `from_value` is rejected for algebraic enums.

`Enums.keys(Direction)` passes a *type* where a value is expected; the checker special-cases the `Enums` callee and treats its argument as a type reference.

## Lowering

All members are compile-time known except the lookups:

- `count` → `ConstInt`.
- `keys()` → constant `List<String>` construction (`ListNew` + `ListAdd` of `ConstString`s, or the existing array/list literal path).
- `values()` → constant `List<E>` of each variant value (`EnumNew`-style construction with each discriminant).
- `from_name(name)` → expanded equality chain over the runtime `String` argument producing `Ok`/`Err` — or a dedicated `InstKind`/`zirk_rt` helper if expansion is unwieldy.
- `from_value(v)` → same shape over mapped values (numeric or string mappings).

`Enums.*` forward to the same lowering once the type argument is resolved.

## Verification

New instructions (if any) verify receiver/result types as usual; expanded lowering needs no new `InstKind`.

## Runtime

Only needed if lookups use a runtime helper; prefer compile-time expansion so no new `zirk_rt_*` symbol is required.
