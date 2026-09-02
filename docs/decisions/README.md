# Architecture decisions (ADRs)

Record of decisions that cascade to the rest of the project, required by `ZIRK_ROADMAP.md` Phase 0.

ADRs are the **durable source** of decisions. The `design.md` of each OpenSpec change references them instead of duplicating them: a change gets archived, an ADR does not.

| ADR | Decision | Status |
|---|---|---|
| [ADR-001](./ADR-001-pin-llvm.md) | Pin LLVM 20.1 + inkwell's `llvm20-1` feature | accepted |
| [ADR-002](./ADR-002-runtime-staticlib.md) | `zirk-runtime` as a staticlib with a C ABI boundary | accepted |
| [ADR-003](./ADR-003-memoria.md) | Memory: constraints now, implementation in Phase 4 | accepted / open |
| [ADR-004](./ADR-004-portabilidad.md) | Portability strategy (build vs. produce) | accepted |
| [ADR-005](./ADR-005-representacion-string.md) | Opaque `String` behind the runtime boundary | accepted |
| [ADR-006](./ADR-006-language-of-the-codebase.md) | The project is written in English | accepted |
| [ADR-007](./ADR-007-forma-de-la-ir.md) | Shape of the intermediate representation | accepted |
| [ADR-008](./ADR-008-color-en-diagnosticos.md) | Color in diagnostics | accepted |
| [ADR-009](./ADR-009-eliminacion-de-codigo-muerto-al-enlazar.md) | Dead code elimination at link time | accepted |
| [ADR-010](./ADR-010-ubicaciones-multiarchivo.md) | Locations spanning multiple files | accepted |
| [ADR-011](./ADR-011-identidad-e-igualdad-de-string.md) | Identity, equality, and normalization of `String` | accepted |
| [ADR-012](./ADR-012-layout-de-objetos.md) | Object layout: separate header, records/value classes inline without one | accepted |
| [ADR-013](./ADR-013-forma-del-despacho.md) | Shape of dispatch: direct by default, own or contract table only when needed | accepted |
| [ADR-014](./ADR-014-representacion-de-char.md) | `Char` shares `String`'s opaque representation | accepted |

## Status of Phase 0 pending items

- **Workspace layout** — ✅ resolved. Nine crates under `crates/`: the eight from the roadmap plus `zirk-runtime` (ADR-002). The single dependency direction across the pipeline is documented in each crate's `lib.rs`.
- **Diagnostics format** — ✅ resolved. `zirk-diagnostics` implements the format from `ZIRK_COMPILER_SPEC.md` section 8 with both human and structured rendering, and `zirk-codegen-llvm` already uses it.
- **LLVM sanity check** — ✅ resolved. It lives as a permanent test in `zirk-codegen-llvm`, not as a disposable spike.
- **Portability verification in CI** — ⏳ pending. The workflow exists; it still needs to be run. See ADR-004.
