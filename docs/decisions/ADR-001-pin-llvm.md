# ADR-001 — Pin de LLVM 20.1

- **Estado:** aceptada
- **Fecha:** 12 de agosto de 2026
- **Fase:** 0

## Contexto

`ZIRK_COMPILER_SPEC.md` sección 5 fija LLVM como único backend inicial. El stack de implementación usa `inkwell` sobre `llvm-sys`, que enlaza contra la ABI de C++ de **una versión mayor concreta** de LLVM: una versión distinta no compila, no es una preferencia de estilo.

`inkwell` 0.10 soporta de `llvm15-0` a `llvm22-1`. Homebrew ofrece de `llvm@14` a `llvm@22`.

## Decisión

Se fija **LLVM 20.1.x**, con la feature `llvm20-1` de `inkwell` 0.10.

El criterio de selección es **disponibilidad en las tres plataformas por sobre novedad**. Con el requisito de portabilidad de [ADR-004](./ADR-004-portabilidad.md), una versión que sea difícil de obtener en cualquiera de las tres plataformas bloquea el proyecto entero; una versión con APIs un poco más viejas solo cuesta comodidad.

LLVM 20.1 está disponible como paquete de desarrollo con bibliotecas estáticas en:

- macOS — `brew install llvm@20`
- Linux — `apt.llvm.org`, paquete `llvm-20-dev`
- Windows — **no** la distribución oficial. Ver abajo.

## Verificación

El sanity check exigido por el roadmap (Fase 0) se ejecutó y pasó sobre `aarch64-macos`:

```
inkwell 0.10 → LLVM IR → objeto Mach-O arm64 → enlace → binario nativo → ejecuta (exit 0)
```

El binario resultante es un `Mach-O 64-bit executable arm64` que solo depende de `libSystem`.

Instalación local verificada: LLVM 20.1.8 con 203 bibliotecas estáticas presentes, lld 20.1.8 con los cuatro drivers (`ld.lld`, `ld64.lld`, `lld-link`, `wasm-ld`).

## Consecuencias

- Toda máquina de desarrollo y todo runner de CI debe proveer LLVM 20.1 con bibliotecas estáticas y exponer `LLVM_SYS_201_PREFIX`. Ver [TOOLCHAIN.md](../TOOLCHAIN.md).
- Subir de versión mayor de LLVM es un cambio deliberado con su propio ADR, no una actualización de rutina.
- En Windows, **ninguna** distribución oficial de LLVM funciona con `llvm-sys`. El instalador `.exe` no incluye bibliotecas estáticas, y el tarball de desarrollo, que sí las incluye, está compilado contra la CRT estática mientras que Rust usa la dinámica: el proceso aborta con `STATUS_ACCESS_VIOLATION` en la primera llamada a LLVM que devuelva una cadena.

  Se usa `TyrsDev/llvm-package-windows`, un build mantenido específicamente para que `inkwell` funcione en Windows, en la misma versión del pin. Es una dependencia de un solo mantenedor y constituye un riesgo de cadena de suministro asumido conscientemente; la alternativa es compilar LLVM desde fuente con `LLVM_USE_CRT_RELEASE=MD`, que son horas por build.

  Las cuatro combinaciones descartadas y su razón están en el issue #2.
