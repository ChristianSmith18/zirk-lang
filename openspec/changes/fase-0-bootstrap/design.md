## Context

Zirk se especifica en cinco documentos normativos que describen el lenguaje **maduro**. El roadmap traduce eso en fases; Fase 0 es la única que no produce lenguaje, sino los cimientos sobre los que se construye todo lo demás.

Las decisiones de fondo ya están tomadas y viven en `docs/decisions/` como ADRs. Este documento **no las repite**: los ADRs son la fuente durable (sobreviven al archivado de este change) y acá solo se referencian.

| Decisión | ADR |
|---|---|
| Pin de LLVM 20.1 + feature `llvm20-1` | [ADR-001](../../../docs/decisions/ADR-001-pin-llvm.md) |
| `zirk-runtime` staticlib con frontera ABI C | [ADR-002](../../../docs/decisions/ADR-002-runtime-staticlib.md) |
| Memoria: restricciones ahora, implementación en Fase 4 | [ADR-003](../../../docs/decisions/ADR-003-memoria.md) |
| Portabilidad: construir vs producir | [ADR-004](../../../docs/decisions/ADR-004-portabilidad.md) |
| `String` opaco tras el runtime | [ADR-005](../../../docs/decisions/ADR-005-representacion-string.md) |

Estado verificado al momento de escribir esto: LLVM 20.1.8 y lld 20.1.8 instalados, `inkwell` 0.10 produciendo un binario `Mach-O arm64` ejecutable, y emisión de objetos confirmada para los nueve targets del spec desde un host `aarch64-macos`.

## Goals / Non-Goals

**Goals:**

- Un `cargo build` que compile los nueve crates en las tres plataformas.
- Un test que demuestre la cadena completa: inkwell → LLVM IR → objeto → enlace → binario que ejecuta.
- Un test que demuestre emisión de objetos para los nueve targets del spec.
- CI que ejecute ambos sobre `{windows, linux, macos} × {x86_64, aarch64}`.
- `zirk-diagnostics` emitiendo el formato de `COMPILER_SPEC` §8.

**Non-Goals:**

- **Cualquier sintaxis de Zirk.** No se parsea un `.zrk`. Los crates de lexer, parser, ast, sema e ir existen con su API mínima y sin implementación.
- Cross-**linking** con sysroots por target (Fase 6). Solo se cubre emisión de objetos.
- `zirk run`, `zirk build` y cualquier subcomando real de CLI (Fase 1 y Fase 6).
- La implementación de la estrategia de memoria (Fase 4).

## Decisions

### D1 — Nueve crates desde el inicio, aunque seis queden casi vacíos

El roadmap propone ocho crates; ADR-002 agrega `zirk-runtime`.

```
zirk-cli ──▶ zirk-codegen-llvm ──▶ zirk-ir ──▶ zirk-sema ──▶ zirk-ast
    │                                                            ▲
    └──────────────▶ zirk-diagnostics ◀────────────────── zirk-parser ──▶ zirk-lexer

zirk-runtime  (no depende de ninguno: se compila a staticlib y se enlaza
               en los binarios que Zirk produce, no en el compilador)
```

**Alternativa descartada:** empezar con un crate monolítico y separar después. El costo de un crate vacío en un workspace de Cargo es prácticamente nulo, mientras que partir un monolito una vez que las cinco etapas comparten tipos es un refactor caro. El roadmap ya resolvió esto en Fase 0 y no se relitiga.

Regla estructural: **las dependencias entre crates fluyen en un solo sentido a lo largo del pipeline.** `zirk-diagnostics` es la única dependencia transversal permitida.

### D2 — El sanity check de LLVM vive en el repo como test, no como spike

El roadmap lo plantea como una verificación previa desechable. Se decide incorporarlo como test permanente de `zirk-codegen-llvm`.

**Motivo:** su valor no se agota al pasar una vez. Es el test que detecta que alguien tiene la versión equivocada de LLVM, que un runner de CI perdió `LLVM_SYS_201_PREFIX`, o que una actualización de `inkwell` rompió la emisión. Como spike desechable ese valor se pierde el día que se borra.

### D3 — El pin de LLVM se verifica, no se confía

`llvm-sys` falla de formas confusas ante una versión mayor equivocada. El build script debe comprobar la versión de LLVM y fallar con un mensaje que apunte a `docs/TOOLCHAIN.md`, en vez de dejar que el error emerja como un fallo de enlace ilegible.

Es coherente con la filosofía del propio proyecto: `COMPILER_SPEC` §8 exige diagnósticos con causa y ayuda. Aplicárselo al propio build del compilador es consistente, no exceso de celo.

### D4 — `LLVM_SYS_201_PREFIX` por entorno, nunca versionado

No va en `.cargo/config.toml` porque esa ruta es específica de cada máquina y plataforma. Versionarla rompe exactamente el requisito de portabilidad de ADR-004. Se documenta en `docs/TOOLCHAIN.md` y CI la define por plataforma.

### D5 — La matriz de CI es la definición operativa de "portable"

Portabilidad afirmada en un documento no es portabilidad. La matriz mínima:

| OS | Arquitecturas | Fuente de LLVM 20.1 |
|---|---|---|
| Linux | x86_64, aarch64 | `apt.llvm.org` → `llvm-20-dev` |
| macOS | x86_64, aarch64 | `brew install llvm@20` |
| Windows | x86_64 | tarball `clang+llvm-20.1.8-*-pc-windows-msvc.tar.xz` |

**Windows aarch64 queda como objetivo, no como bloqueante** de esta change: el tarball oficial existe, pero la disponibilidad de runners es menos estable y no debe frenar el resto.

La descarga de LLVM en CI debe cachearse; sin caché, cada job paga cientos de MB y la matriz se vuelve impracticable.

## Risks / Trade-offs

- **Windows no logra construir `llvm-sys`** → Es el riesgo principal y por eso es parte de esta change, no de una posterior. Mitigación: usar el tarball oficial de desarrollo (no el `.exe`), que sí incluye las bibliotecas estáticas, y VS 2022 Build Tools. Si aun así falla, el hallazgo obliga a revisar ADR-001 antes de seguir a Fase 1.

- **Seis crates vacíos parecen burocracia** y tientan a fusionarlos → Mitigación: cada crate arranca con su responsabilidad documentada en su `lib.rs`, de modo que el límite sea explícito antes de tener código que lo respete.

- **El sanity check pasa en macOS y da falsa confianza** → Mitigación: hasta que CI corra en verde en las tres plataformas, la portabilidad se considera no verificada. Es el criterio de salida de esta change.

- **La matriz de CI hace el ciclo lento** → Mitigación: cachear la instalación de LLVM y las dependencias de Cargo. Si aun así molesta, la matriz completa puede quedar en `main` y PRs correr solo el host, pero nunca al revés.

- **El pin de LLVM 20.1 envejece** → Trade-off aceptado conscientemente en ADR-001: disponibilidad sobre novedad. Subir de versión mayor es un cambio deliberado con su propio ADR.

## Migration Plan

No aplica: no hay estado previo que migrar. El repo no tiene código.

Rollback: la change es puramente aditiva; revertir es borrar el workspace y los archivos de CI.

## Open Questions

- **¿Runners de Windows aarch64?** Su disponibilidad determina si `aarch64-windows` entra en la matriz ahora o se pospone. Se resuelve al montar CI.
- **¿`rust-toolchain.toml` pinea versión exacta o mínima?** Pinear exacto da reproducibilidad y coincide con el espíritu de `COMPILER_SPEC` §7; pinear mínimo reduce fricción a quien contribuye. Inclinación: exacto, coherente con la disciplina del pin de LLVM.
- **¿Qué expone `zirk-runtime` en esta fase?** Con Fase 1 fuera de alcance, podría no exponer nada. Alternativa: definir ya `zirk_rt_init` y `zirk_rt_shutdown` vacíos para fijar la forma del ciclo de vida de `RUNTIME_SPEC` §2. Inclinación: definirlos, es barato y es justo el gancho que ADR-002 justifica.
