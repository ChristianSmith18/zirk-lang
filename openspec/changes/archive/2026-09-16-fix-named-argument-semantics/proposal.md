## Why

The documented named-argument contract is only partially implemented. In particular, calls other than free functions can compile with labels accepted by the checker yet receive values in source order at runtime, causing silent miscompilation; shorthand, ordering validation, and constructor overload selection also diverge from the language contract.

## What Changes

- Make every supported call form bind named arguments by declared parameter name at parse, check, and lowering time, including methods, constructors, `super.method(...)`, and base-constructor `super(...)` calls.
- Parse `name:` as the equivalent named expression `name: name` for a simple identifier, preserving existing name-resolution diagnostics when that identifier is not visible.
- Reject a positional argument after the first named argument, including after a shorthand argument.
- Reject constructor declarations whose parameter types admit the same positional call, then resolve the remaining overloads using arity, labels, optional parameters, and argument types; retain the checker-selected constructor identity for lowering and diagnose genuinely ambiguous calls.
- Add regression coverage for successful reordered calls and for each rejected form, with an executable test guarding against wrong runtime parameter binding.
- Update the normative language reference and handbook examples to match the delivered behavior and synchronize the affected public documentation with `../zirk-lang-site`.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `zirk-grammar`: calls accept identifier-only named-argument shorthand and preserve labels while rejecting positional arguments after named arguments.
- `zirk-type-system`: callable matching and constructor overload resolution select a unique parameter binding by labels, types, arity, and optionality.
- `zirk-ir-lowering`: lowering consumes checker-resolved parameter and constructor bindings so every call form passes values in declaration order.

## Impact

- **Code**: `zirk-parser` call-argument parsing; `zirk-sema` argument matching, constructor selection, diagnostics, and resolved-construction metadata; `zirk-ir` shared argument lowering and method, constructor, and `super` call paths.
- **Tests**: parser, semantic, lowering, and CLI corpus/runtime regressions covering free functions, methods, constructors, and both `super` forms.
- **Public surface**: yes. This corrects normative named-call and constructor-resolution semantics already documented in `docs/ZIRK_LANGUAGE_SPEC.md` and the named-arguments handbook page; examples and delivered-status wording must be reviewed.
- **Companion repository**: after the relevant Zirk documentation is committed, update `../zirk-lang-site` with `./scripts/sync-website-content.sh`, review its diff and any status catalog changes, and validate the separate site revision.
