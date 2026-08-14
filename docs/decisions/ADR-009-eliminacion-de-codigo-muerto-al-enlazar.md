# ADR-009 — Eliminación de código muerto al enlazar

- **Estado:** aceptada
- **Fecha:** 14 de agosto de 2026
- **Fase:** 1

## Contexto

Un programa Zirk mínimo —`fn main(): Void { stdout.println("..."); }`— enlazaba a **~1.4 MB**, frente a 336 KB de un `println!` equivalente en Rust puro y 33 KB en C. Una diferencia de 4x sobre Rust puro no es el costo conocido de `std`: es un defecto del enlace.

La causa: `zirk-runtime` ([ADR-002](./ADR-002-runtime-staticlib.md)) expone varios símbolos `extern "C"` independientes —`zirk_rt_init`, `zirk_str_from_i32`, `zirk_str_eq`, y el resto—. Un archivo estático (`.a`) se enlaza a granularidad de **archivo objeto completo**: si algún símbolo de un `.o` se referencia, el linker conserva ese `.o` entero. Con varios puntos de entrada distintos, eso retiene mucho más `std` del que un programa concreto necesita, y sin pedir eliminación de código muerto, ese exceso llega intacto al binario final.

## Decisión

**Se pide eliminación de código muerto al linker**, con la bandera que corresponde a cada formato de contenedor:

| Plataforma | Contenedor | Bandera |
|---|---|---|
| macOS | Mach-O | `-Wl,-dead_strip` |
| Linux | ELF | `-Wl,--gc-sections` |
| Windows | COFF (`lld-link`) | `-Wl,/OPT:REF` |

`lld-link` no implementa `--gc-sections`; su equivalente es `/OPT:REF`. Las tres se pasan a través del mismo driver de enlace (`clang`) que ya exige [ADR-004](./ADR-004-portabilidad.md).

## Verificación

Medido en `aarch64-macos`, con el mismo perfil de build que usa CI:

```
sin la bandera:  ~1.4 MB
con la bandera:  ~446 KB    (en línea con Rust puro)
```

Hay un test de punta a punta que fija un techo generoso (1 MB) para el ejecutable del programa de referencia, pensado para atrapar una regresión a "código muerto sin eliminar", no para fijar un tamaño exacto que variaría entre plataformas y versiones del toolchain.

## Consecuencias

- Ningún cambio de comportamiento: la bandera solo elimina código inalcanzable.
- El ahorro crece con cada símbolo nuevo que `zirk-runtime` exponga: sin esta bandera, cada función nueva del runtime arrastra su archivo objeto completo al binario final aunque el programa no la use.
- Esto no compite con la optimización de Fase 8 (LTO, tamaño de binario en `COMPILER_SPEC` sección 5): es una eliminación de código muerto en el enlace, ortogonal a la optimización del código que sí se ejecuta.
