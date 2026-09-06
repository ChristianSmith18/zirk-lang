# Design: native-type-member-surface

## Context

Native members are recognized structurally in `Checker::member_type` /
`check_method_call_on` (properties) and `check_call` (methods), then
lowered by dedicated `lower_*_method_call` / `field_type_of` branches in
`zirk-ir`. Runtime helpers live in `zirk-runtime` (`zirk_str_*`,
`zirk_rt_*`, `zirk_regex_*`). The previous change established this pattern
for `String`/`Char`/`Array`/`List`/`Range`/`Regex` — this change extends
it to the full documented base-type surface.

## Goals / Non-Goals

**Goals:** every member the handbook marks deliverable for base/native
types compiles, lowers, and runs; statuses flip back to `implemented`.

**Non-Goals:** the universal `.type` member (needs a `Type` runtime
object), union values, `Pin`/`Volatile` pointer extras, eager collection
combinators (`map`/`filter`/`reduce`), `Throwable.suppressed` as
`List<Throwable>`.

## Decisions

- **Runtime helpers over new IR instructions.** Nearly all members lower
  to `InstKind::Call` on new `zirk_*` externs. This keeps `zirk-ir` and
  `zirk-codegen-llvm` untouched except for declaring the externs and
  dispatching the calls — the same way `zirk_str_trim` works today.
  Width-correct integer semantics are achieved by passing the width as a
  trailing `i32` parameter where it matters (e.g. `zirk_int_abs(value,
  width_bits, is_signed)`), or by normalizing in lowering (mask/sign-extend
  the i64 before/after the call).
- **Scalar `Int8`…`Int128`/`UInt*`/`Float*` type names resolve as static
  member namespaces.** `Int32.MAX` currently fails because `Int32` is not
  a declared path. The checker intercepts `TypeName.member` before name
  resolution for the known scalar names; lowering emits the constant
  directly.
- **`IntN.parse`/`FloatN.parse` return the native `Result`** — a runtime
  helper returns an i64 status + value pair the lowering wraps into the
  existing `Ok`/`Error` representation (same layout `Result` already uses
  for `NativeError`).
- **`checked_*` reuses the existing checked-arithmetic machinery**: the
  lowering performs the operation with overflow detection already used by
  `+`, then boxes the outcome into `Result<T, OverflowError>`. `wrapping_*`
  lowers to the same arithmetic without the check; `saturating_*` calls a
  runtime helper (width-aware clamp is verbose in IR).
- **Char/String `normalize` uses `unicode-normalization`**; `form` is a
  small native enum (`UnicodeNormalization` with `NFC`/`NFD`/`NFKC`/`NFKD`)
  the checker registers.
- **`find` vs `search`**: `find(needle)` returns `Int64?` (null when
  absent) — implemented on top of the existing `zirk_str_search` by
  mapping `-1` to `null` in lowering.
- **Collections**: literal construction lowers to `new` + per-element
  `add`/`set`; `clone`/`to_string` get `zirk_rt_list_clone` /
  `zirk_rt_*_to_string` helpers that render elements through the existing
  print conversion. Negative indices lower to `length + index` before the
  existing bounds check (also applied to `String` indexing).
- **Enum case members**: `case.name`/`case.value`/`case.to_string()`
  lower to constants known at compile time — `name`/`to_string` produce a
  `String` literal of the case name; `value` produces the declared mapping
  or the name.
- **Default `to_string` on structured types**: lowered to a runtime
  `zirk_type_name_of`-style rendering only when cheap — otherwise a
  compile-time `TypeName(...)` literal. Tuple `to_string` renders
  element-wise through the existing print conversion.
- **`Regex.parse`** calls a runtime `zirk_regex_compile` returning
  handle-or-null, wrapped into `Result<Regex, RegexError>`.

## Risks / Trade-offs

- [Int128 ops need two-i64 runtime signatures] → pass/return 128-bit
  values as `i128` in IR (`IntWidth::I128` exists); runtime uses `i128`
  FFI directly since `extern "C"` supports it on this platform.
- [Unicode normalization adds a crate] → `unicode-normalization` is a
  small, established dependency, already licensed-compatible with
  `unicode-segmentation` (same workspace family).
- [Surface is large; regressions hide in width edge cases] → corpus
  fixtures per family cover MIN/MAX boundaries for narrow widths.
- [`.type` and union members stay unimplemented] → documented as
  specified; this change does not claim them.
