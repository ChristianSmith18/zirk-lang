## Why

Zirk currently makes callers unpack even routine expected failures before they can use a successful value, so simple environment reads and other nullable operations require ceremony. This change gives `Result<T, E>` two explicit usage modes: preserve and inspect the complete result when the program asks for it, or project it to `T?` wherever an ordinary value is expected.

## What Changes

- **BREAKING** Define contextual `Result<T, E>` projection: a direct `match` and contexts explicitly typed as `Result<...>` preserve both variants; other value-demanding contexts convert `Ok(value)` to `value` and `Error(_)` to `null`.
- **BREAKING** Make the projected nullable type participate in ordinary inference, arguments, returns, assignments, member operations, interpolation, and `??`; specify the order in which contextual typing and generic inference select the preserved or projected type.
- **BREAKING** Remove `Result` status and extraction helpers that become redundant with direct `match` or projection, and remove environment fallback/null helpers that duplicate the same policies. Keep `Result` itself and its genuine composition operations.
- Update `std.environment` examples so ordinary reads are concise while explicit `Result` annotations and direct `match` retain permission, parsing, and host-error details.
- Specify expression-statement handling, diagnostics, nullable rendering, and the intentional loss of `E` after projection so the conversion cannot silently change in different contexts.
- Update the language and standard-library specifications, the `Result` and environment handbook, core/error semantics, current feature-status and limitation documents, and add an ADR for the two-context `Result` model.
- Update the companion `../zirk-lang-site` after the authoritative repository changes are committed.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `zirk-errors`: define explicit-result preservation, implicit nullable projection, and the revised `Result` surface and must-use behavior.
- `zirk-type-system`: define contextual typing, generic inference order, nullable result projection, and its interaction with direct `match` and `??`.
- `zirk-standard-library`: remove overlapping `Result`/`Env` convenience APIs and specify environment usage through explicit inspection or projection.
- `universal-string-rendering`: define how a `Result` used as printable/interpolated value projects before nullable rendering.

## Impact

- Compiler: semantic analysis and type inference; call, assignment, return, member, interpolation, and print contexts; direct `match` typing; IR lowering for `Ok`/`Error` projection; diagnostics and compiler conformance tests.
- Standard library: `Result` structural methods and `std.environment` public declarations and call sites.
- Normative docs and ADRs: `docs/ZIRK_LANGUAGE_SPEC.md`, `docs/CORE_LANGUAGE_SEMANTICS.md`, `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`, `docs/ZIRK_STDLIB_SPEC.md`, handbook error/environment pages, current feature status/limitations, and a new ADR-024.
- Companion repository: `../zirk-lang-site` must receive the reviewed public docs/status update after the Zirk commit.
- Compatibility: source using removed extraction, inspection, or fallback helpers must migrate to direct `match`, explicit `Result<...>`, projection, or `??` according to whether it needs error detail.
