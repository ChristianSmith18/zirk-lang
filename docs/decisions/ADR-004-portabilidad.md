# ADR-004 — Estrategia de portabilidad

- **Estado:** aceptada
- **Fecha:** 12 de agosto de 2026
- **Fase:** 0

## Contexto

El requisito es que Zirk sirva en cualquier máquina, no solo en la del autor: Windows, Linux y macOS sobre las arquitecturas base del spec (`ZIRK_COMPILER_SPEC.md` sección 6).

Ese requisito esconde **dos portabilidades distintas** que no cuestan lo mismo y no se resuelven igual. Confundirlas es la trampa principal.

```
PORTABILIDAD A — el compilador se CONSTRUYE en las 3 plataformas
    ¿quién puede compilar zirkc desde fuente?
    ← acá duele llvm-sys en Windows

PORTABILIDAD B — el compilador PRODUCE binarios para las 3 plataformas
    ¿para qué targets emite Zirk? (COMPILER_SPEC seccion 6)
    ← NO requiere que el compilador corra en Windows
```

**B es la que le importa al usuario de Zirk. A solo le importa a quien desarrolla el compilador.**

## Hallazgo que motiva la decisión

Se verificó empíricamente que **una sola instalación de LLVM 20.1 sobre `aarch64-macos` emite objetos nativos válidos para los nueve targets del spec**:

| Target Zirk | Objeto producido |
|---|---|
| `aarch64-macos` | Mach-O 64-bit object arm64 |
| `x86_64-macos` | Mach-O 64-bit object x86_64 |
| `x86_64-linux` | ELF 64-bit LSB, x86-64 |
| `aarch64-linux` | ELF 64-bit LSB, ARM aarch64 |
| `armv7-linux` | ELF 32-bit LSB, ARM EABI5 |
| `x86-linux` | ELF 32-bit LSB, Intel 80386 |
| `x86_64-windows` | Intel amd64 COFF object file |
| `x86-windows` | Intel 80386 COFF object file |
| `aarch64-windows` | Aarch64 COFF object file |

Es decir: **la portabilidad B está estructuralmente resuelta desde el día uno** por LLVM + lld, sin trabajo adicional del compilador.

## Decisión

1. **La portabilidad B es un requisito verificado continuamente.** La emisión de objetos para los nueve targets se cubre con tests desde Fase 1, no se pospone a Fase 6.

2. **La portabilidad A se resuelve vía CI en las tres plataformas + binarios preconstruidos.** El usuario de Zirk nunca compila el compilador: descarga un `zirkc` ya construido. Solo quien contribuye al compilador necesita el toolchain completo de [TOOLCHAIN.md](../TOOLCHAIN.md).

3. **`lld` es el linker por defecto**, no el `cc` que haya en el host. Provee `ld.lld` (ELF), `ld64.lld` (Mach-O) y `lld-link` (COFF) desde un mismo binario y una misma versión, lo que hace el enlace reproducible entre plataformas. Depender del `cc` de cada host reintroduce por la puerta de atrás la variabilidad que este ADR busca eliminar.

## Riesgo abierto: sysroots para cross-linking

Emitir el objeto está resuelto; **enlazar** un ejecutable para otra plataforma requiere además el sysroot de destino (libc y bibliotecas del sistema). Esto no está resuelto y es trabajo real de Fase 6.

No bloquea Fase 1, que solo compila para el host. Se registra acá para que no se descubra tarde.

## Consecuencias

- CI debe cubrir la matriz `{windows, linux, macos} × {x86_64, aarch64}` desde Fase 0.
- Ningún desarrollo puede depender de una ruta absoluta específica de una máquina. En particular, `LLVM_SYS_201_PREFIX` se resuelve por entorno y **no** se versiona en `.cargo/config.toml`.
- Windows es la plataforma de mayor fricción para la portabilidad A; ver [ADR-001](./ADR-001-pin-llvm.md) para la fuente de LLVM que sí funciona ahí.
