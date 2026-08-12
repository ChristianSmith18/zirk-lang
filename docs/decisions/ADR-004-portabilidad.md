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

- La verificación debe cubrir `{windows, linux, macos}` desde Fase 0, repartida entre CI y la máquina de desarrollo (ver "Reparto de la verificación").
- Ningún desarrollo puede depender de una ruta absoluta específica de una máquina. En particular, `LLVM_SYS_201_PREFIX` se resuelve por entorno y **no** se versiona en `.cargo/config.toml`.
- Windows es la plataforma de mayor fricción para la portabilidad A; ver [ADR-001](./ADR-001-pin-llvm.md) para la fuente de LLVM que sí funciona ahí.

## Reparto de la verificación

**macOS se verifica localmente; Linux y Windows en CI.**

No es una concesión de comodidad. Los runners de macOS consumen minutos a 10x en repositorios privados, y `macos-13` (Intel) además rara vez consigue runner: en las primeras ejecuciones quedó encolado indefinidamente mientras el resto de la matriz completaba. Ejecutar en un runner de macOS lo mismo que ya se ejecuta en el escritorio de desarrollo —también `aarch64-macos`— no aporta información nueva y es, con diferencia, la parte más cara de la matriz.

Linux y Windows sí aportan información que la máquina de desarrollo no puede dar, y por eso están en CI.

```
   macOS aarch64  ──▶  ./scripts/check-local.sh   (máquina de desarrollo)
   Linux x86_64   ──┐
   Linux aarch64  ──┼─▶  GitHub Actions
   Windows x86_64 ──┘
```

La contrapartida honesta: **una regresión específica de macOS no la detecta una PR**, solo la detecta quien corra el script. Es un riesgo aceptado a cambio del costo, y se revierte haciendo el repositorio público, donde Actions es gratis e ilimitado.

## Estado de la verificación

| Portabilidad | Estado | Evidencia |
|---|---|---|
| **B** — emisión para los 9 targets | ✅ verificada | Test `target_matrix`: emite y valida contenedor y arquitectura de los nueve targets. |
| **A** — macOS aarch64 | ✅ verificada | `./scripts/check-local.sh` en verde sobre `Darwin arm64`: fmt, clippy y 23 tests. |
| **A** — Linux x86_64 | ✅ verificada | CI en verde. |
| **A** — Linux aarch64 | ✅ verificada | CI en verde. |
| **A** — Windows x86_64 | ⏳ **en curso — riesgo principal** | Resueltos dos fallos (switch de 7-Zip, extracción del tarball). Pendiente: `LNK1181` por `libxml2s.lib`, que LLVM declara como dependencia del sistema en Windows pero su distribución oficial no incluye. |
| **A** — macOS x86_64 | 🚫 fuera de alcance | Intel es plataforma en retirada y sus runners son escasos. |
| **A** — Windows aarch64 | 🚫 fuera de la matriz inicial | Apila riesgo sobre la plataforma ya más frágil. Se incorpora cuando `windows-x86_64` esté estable. |

**Windows sigue siendo el único punto no verificado**, tal como este ADR anticipó desde el principio.
