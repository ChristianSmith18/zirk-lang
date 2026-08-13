# Zirk

Lenguaje de programación compilado, orientado a objetos, con tipado estático e inferencia. De alto nivel por defecto y con acceso opcional a bajo nivel. Compila a binarios nativos vía LLVM.

> **Fácil por defecto, explícito cuando necesitas control.**

Concurrencia estructurada (`task` / `await`) y paralelismo multinúcleo (`parallel` / `thread`) son características de primera clase. Los binarios son standalone: no requieren Node.js, Python, Java ni ninguna otra instalación.

```zirk
import { stdout } from std.io;

fn main(): Void {
    stdout.println("Hola desde Zirk");
}
```

---

## ⚠️ Estado: Fase 0 — cimientos

**Zirk todavía no compila código Zirk.** El ejemplo de arriba es la meta de la Fase 1, no algo que hoy funcione.

Lo que sí funciona y está verificado con tests:

| Capacidad | Estado |
|---|---|
| Generar código máquina y producir un binario nativo que ejecuta | ✅ |
| Emitir objetos para los 9 targets del spec (Windows/Linux/macOS × x86, x86_64, armv7, aarch64) | ✅ |
| Diagnósticos con severidad, código estable, ubicación, causa y ayuda | ✅ |
| Runtime enlazable con frontera ABI C | esqueleto |
| Léxico, sintaxis, tipos, IR de Zirk | ❌ Fase 1 |

```
   .zrk ──▶ [lexer] ──▶ [parser] ──▶ [sema] ──▶ [ir] ──▶ [codegen] ──▶ binario
               ↑           ↑           ↑          ↑          ↑
             Fase 1      Fase 1     Fase 1     Fase 1    ✅ funciona
```

La Fase 0 construye primero la parte riesgosa —que el backend de LLVM funcione en las tres plataformas— antes que la predecible. Descubrir tarde que el toolchain no compila en Windows costaría meses de trabajo tirado.

## Arquitectura

Workspace de Cargo con un crate por etapa del pipeline de `ZIRK_COMPILER_SPEC.md` sección 2:

| Crate | Responsabilidad |
|---|---|
| `zirk-lexer` | texto fuente → tokens |
| `zirk-parser` | tokens → árbol de sintaxis |
| `zirk-ast` | forma del árbol de sintaxis |
| `zirk-sema` | resolución de nombres, tipos, análisis de flujo |
| `zirk-ir` | representación intermedia tipada y portable |
| `zirk-codegen-llvm` | IR → LLVM → archivo objeto |
| `zirk-diagnostics` | formato de errores y warnings |
| `zirk-cli` | el ejecutable `zirk` |
| `zirk-runtime` | runtime enlazado en los binarios producidos |

Las dependencias fluyen en un solo sentido a lo largo del pipeline. `zirk-diagnostics` es la única transversal.

## Empezar

Requiere Rust 1.94+ y **LLVM 20.1** con bibliotecas estáticas. La instalación por plataforma está en **[docs/TOOLCHAIN.md](docs/TOOLCHAIN.md)** — leelo antes de compilar, especialmente en Windows, donde el instalador `.exe` oficial de LLVM **no sirve**.

```sh
export LLVM_SYS_201_PREFIX=$(brew --prefix llvm@20)   # macOS
cargo test --workspace
```

## Documentación

**Especificación normativa** — describe Zirk maduro, no lo que hoy existe:

- [ZIRK_SPEC_FINAL.md](docs/ZIRK_SPEC_FINAL.md) — alcance, exclusiones, filosofía
- [ZIRK_LANGUAGE_SPEC.md](docs/ZIRK_LANGUAGE_SPEC.md) — sintaxis, tipos, objetos, errores
- [ZIRK_COMPILER_SPEC.md](docs/ZIRK_COMPILER_SPEC.md) — pipeline, IR, LLVM, targets, CLI
- [ZIRK_RUNTIME_SPEC.md](docs/ZIRK_RUNTIME_SPEC.md) — memoria, tasks, scheduler, recursos
- [ZIRK_STDLIB_SPEC.md](docs/ZIRK_STDLIB_SPEC.md) — biblioteca estándar

**Construcción:**

- [ZIRK_ROADMAP.md](docs/init/ZIRK_ROADMAP.md) — las 13 fases, de acá al self-hosting
- [docs/decisions/](docs/decisions/) — ADRs: decisiones que cascadean al resto del proyecto
- [docs/TOOLCHAIN.md](docs/TOOLCHAIN.md) — instalación del toolchain

## Idioma

Las specs, el roadmap y el código están **en inglés**. Los ADRs, esta guía y el resto de la documentación de trabajo están en español. Ver [ADR-006](docs/decisions/ADR-006-language-of-the-codebase.md).

## Contribuir

Ver [CONTRIBUTING.md](CONTRIBUTING.md). El proyecto usa git flow y avanza por fases: no se implementan features de fases futuras aunque estén especificadas.

## Licencia

Apache-2.0. Ver [LICENSE](LICENSE).
