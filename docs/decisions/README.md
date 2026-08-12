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

## Pendientes de Fase 0

- **Layout del workspace** — los ocho crates que propone el roadmap más `zirk-runtime` (ADR-002). Se resuelve al crear el workspace.
- **Formato de diagnósticos** — `zirk-diagnostics` como crate propio desde el día uno, según `ZIRK_COMPILER_SPEC.md` sección 8.
