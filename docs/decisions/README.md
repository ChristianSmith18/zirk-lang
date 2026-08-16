# Decisiones de arquitectura (ADRs)

Registro de decisiones que cascadean al resto del proyecto, exigido por `ZIRK_ROADMAP.md` Fase 0.

Los ADRs son la **fuente durable** de las decisiones. Los `design.md` de cada change de OpenSpec los referencian en vez de duplicarlos: un change se archiva, un ADR no.

| ADR | Decisión | Estado |
|---|---|---|
| [ADR-001](./ADR-001-pin-llvm.md) | Pin de LLVM 20.1 + feature `llvm20-1` de inkwell | aceptada |
| [ADR-002](./ADR-002-runtime-staticlib.md) | `zirk-runtime` como staticlib con frontera ABI C | aceptada |
| [ADR-003](./ADR-003-memoria.md) | Memoria: restricciones ahora, implementación en Fase 4 | aceptada / abierta |
| [ADR-004](./ADR-004-portabilidad.md) | Estrategia de portabilidad (construir vs producir) | aceptada |
| [ADR-005](./ADR-005-representacion-string.md) | `String` opaco tras la frontera del runtime | aceptada |
| [ADR-006](./ADR-006-language-of-the-codebase.md) | El proyecto se escribe en inglés | aceptada |
| [ADR-007](./ADR-007-forma-de-la-ir.md) | Forma de la representación intermedia | aceptada |
| [ADR-008](./ADR-008-color-en-diagnosticos.md) | Color en los diagnósticos | aceptada |
| [ADR-009](./ADR-009-eliminacion-de-codigo-muerto-al-enlazar.md) | Eliminación de código muerto al enlazar | aceptada |
| [ADR-010](./ADR-010-ubicaciones-multiarchivo.md) | Ubicaciones a través de varios archivos | aceptada |
| [ADR-011](./ADR-011-identidad-e-igualdad-de-string.md) | Identidad, igualdad y normalización de `String` | aceptada |

## Estado de los pendientes de Fase 0

- **Layout del workspace** — ✅ resuelto. Nueve crates en `crates/`: los ocho del roadmap más `zirk-runtime` (ADR-002). La dirección única de dependencias a lo largo del pipeline está documentada en el `lib.rs` de cada crate.
- **Formato de diagnósticos** — ✅ resuelto. `zirk-diagnostics` implementa el formato de `ZIRK_COMPILER_SPEC.md` sección 8 con renderizado humano y estructurado, y ya lo usa `zirk-codegen-llvm`.
- **Sanity check de LLVM** — ✅ resuelto. Vive como test permanente en `zirk-codegen-llvm`, no como spike desechable.
- **Verificación de portabilidad en CI** — ⏳ pendiente. El workflow existe; falta ejecutarlo. Ver ADR-004.
