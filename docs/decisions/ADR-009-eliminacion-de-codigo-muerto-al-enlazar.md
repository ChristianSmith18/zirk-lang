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

## El ahorro real depende de la plataforma, y no es parejo

La primera versión de este ADR asumía que la misma bandera lograría un ahorro comparable en las tres plataformas. No es así, y vale explicarlo en vez de ocultarlo: quien lea este ADR dentro de un año y compare tamaños entre plataformas no debería tener que redescubrirlo.

| Plataforma | Mecanismo | Resultado |
|---|---|---|
| macOS | `ld64` elimina por **símbolo**, incluso dentro de una sola sección — no depende de que el código esté dividido en secciones separadas | ~1.4 MB → ~446 KB |
| Windows | LLVM emite las funciones en secciones **COMDAT** por defecto para este target — `/OPT:REF` obtiene esa granularidad fina sin nada adicional | mejora comparable a macOS |
| Linux | Ninguno de los dos mecanismos aplica: el `std` precompilado que distribuye `rustup` **no** tiene una sección por función | mejora marginal, muy por debajo de las otras dos |

Esto último se verificó, no se asumió: se inspeccionaron los archivos objeto del `libstd` precompilado (`ar x` sobre el `.rlib` del sysroot) y se confirmó que un módulo de compilación completo —1332 símbolos en el caso probado— vive en un único `__text`. Sin secciones separadas, `--gc-sections` en ELF solo puede descartar archivos objeto enteros, que es la misma granularidad que el enlace de un `.a` ya tenía **antes** de esta bandera.

Lograr en Linux lo mismo que en las otras dos plataformas requeriría recompilar `std` con `-ffunction-sections`, vía `-Z build-std` — una característica de nightly, fuera del toolchain estable que este proyecto fija (`rust-toolchain.toml`, [ADR-001](./ADR-001-pin-llvm.md) por analogía). Queda fuera de alcance de esta fase.

## Verificación

Medido en `aarch64-macos`, con el mismo perfil de build que usa CI:

```
sin la bandera:  ~1.4 MB
con la bandera:  ~446 KB    (en línea con Rust puro)
```

En CI, Linux (x86_64 y aarch64) enlazó a ~3.9 MB con la bandera —mejora real pero acotada por la razón de arriba—, y Windows quedó por debajo de 1 MB.

Hay un test de punta a punta con un techo de 5 MB para el ejecutable del programa de referencia. Es generoso a propósito: pensado para atrapar una regresión real —la bandera desaparece o deja de aplicarse— y no para fingir un tamaño mínimo parejo que la arquitectura actual, con el toolchain estable, no puede ofrecer en Linux.

## Consecuencias

- Ningún cambio de comportamiento: la bandera solo elimina código inalcanzable.
- El ahorro crece con cada símbolo nuevo que `zirk-runtime` exponga en macOS y Windows; en Linux ese crecimiento no se mitiga con esta bandera.
- Esto no compite con la optimización de Fase 8 (LTO, tamaño de binario en `COMPILER_SPEC` sección 5): es una eliminación de código muerto en el enlace, ortogonal a la optimización del código que sí se ejecuta.
- **Queda anotado como mejora futura**: si el tamaño en Linux se vuelve un problema real —no solo estético—, las vías son `-Z build-std` en nightly (cambia el toolchain pineado) o reducir la cantidad de símbolos `extern "C"` que expone el runtime, agrupando funciones relacionadas en menos archivos objeto para que la granularidad de archivo importe menos.
