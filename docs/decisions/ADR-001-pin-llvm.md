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
- Windows — tarball oficial `clang+llvm-20.1.8-*-pc-windows-msvc.tar.xz` (x86_64 y aarch64)

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
- En Windows, el instalador `.exe` oficial **no sirve**: no incluye las bibliotecas estáticas. Debe usarse el tarball de desarrollo.
