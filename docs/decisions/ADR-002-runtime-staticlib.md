# ADR-002 — `zirk-runtime` como staticlib de Rust con frontera ABI C

- **Estado:** aceptada
- **Fecha:** 12 de agosto de 2026
- **Fase:** 0

## Contexto

`ZIRK_ROADMAP.md` Fase 1 exige que `stdout.println("...")` llegue a una syscall real, pero no define **dónde vive ese código**. El roadmap tampoco incluye un crate de runtime en el layout propuesto del workspace.

Opciones consideradas:

| Opción | A favor | En contra |
|---|---|---|
| **(a)** codegen emite `call @puts` / `@printf` de libc | camino más corto al primer binario | deuda que hay que desarmar entera después; el ciclo de vida de la aplicación no tiene dónde vivir |
| **(b)** crate `zirk-runtime` en Rust compilado a staticlib | fija la frontera ABI C desde el inicio; es el lugar donde después viven scheduler, GC y channels | más trabajo en Fase 1 |
| **(c)** runtime en C | ABI trivial | pierde todo lo que motivó elegir Rust para el compilador |

## Decisión

Se adopta **(b)**: un crate `zirk-runtime` compilado a `staticlib` (`.a` / `.lib`), enlazado en cada binario que Zirk produce. El codegen emite llamadas a símbolos `extern "C"` estables, por ejemplo `zirk_io_println(ptr, len)`.

`zirk-runtime` es el **noveno crate** del workspace, adicional a los ocho que propone el roadmap.

## Motivo

El objetivo declarado de Fase 1 es *validar que la arquitectura completa funciona de punta a punta*. Con la opción (a) se valida el pipeline pero **no la arquitectura**: el ciclo de vida normativo de `ZIRK_RUNTIME_SPEC.md` sección 2 (validar permisos → cargar runtime → inicializar globals → `main` → cierre ordenado → flush → exit code) queda sin lugar donde existir, y habría que reintroducirlo desmontando el codegen.

Con (b), Fase 1 ya produce un `main` de LLVM que llama a `zirk_rt_init()` y `zirk_rt_shutdown()`, aunque hoy ambos no hagan nada. Esa forma vacía es exactamente el gancho donde Fase 4 (memoria) y Fase 5 (concurrencia) se cuelgan sin refactor.

Además satisface directamente `ZIRK_RUNTIME_SPEC.md` sección 1: runtime *pequeño, portable y enlazable en binarios standalone*.

La frontera ABI C es la misma que `ZIRK_LANGUAGE_SPEC.md` sección 13 exige para interoperabilidad nativa, así que no es infraestructura desechable: es la frontera definitiva, estrenada temprano.

## Consecuencias

- Cada target soportado necesita su `zirk-runtime` compilado para ese target. Se relaciona con el riesgo de sysroots de [ADR-004](./ADR-004-portabilidad.md).
- Los símbolos `zirk_rt_*` y `zirk_io_*` son una superficie de compatibilidad: cambiarlos rompe binarios ya compilados. Deben versionarse cuando el lenguaje se estabilice.
- El runtime no puede usar la stdlib de Rust libremente si más adelante se quiere reducir el tamaño del binario; se acepta usarla por ahora y revisar en Fase 11.
