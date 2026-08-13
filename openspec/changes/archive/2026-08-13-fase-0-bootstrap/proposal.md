## Why

El proyecto tiene cinco specs normativas completas y cero líneas de código. `ZIRK_ROADMAP.md` Fase 0 exige resolver, antes de escribir nada del lenguaje, las decisiones que cascadean a todo lo demás y montar el esqueleto que las materializa.

Las decisiones ya están tomadas y registradas como ADRs, y el sanity check de LLVM ya pasó sobre `aarch64-macos`. Lo que falta es el esqueleto verificable: un workspace que compile y un CI que demuestre que compila en las tres plataformas, no solo en la máquina del autor.

Sin esto, cualquier trabajo de Fase 1 se construye sobre un toolchain cuya portabilidad es una suposición.

## What Changes

- **Workspace de Cargo con nueve crates**, uno por etapa del pipeline de `ZIRK_COMPILER_SPEC.md` sección 2: `zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-diagnostics`, `zirk-cli` y `zirk-runtime`.
- **`zirk-runtime` como staticlib con frontera ABI C** (ADR-002). No está en la lista original del roadmap; se agrega como noveno crate.
- **`zirk-diagnostics` operativo desde el día uno** con el formato de `ZIRK_COMPILER_SPEC.md` sección 8 (severidad, código estable, ubicación, causa, ayuda), aunque al principio no tenga consumidores reales.
- **Sanity check de LLVM incorporado al repo como test**, no como spike desechable: verifica que la cadena inkwell → LLVM IR → objeto → enlace → ejecución funciona, y que se emiten objetos válidos para los nueve targets del spec.
- **CI sobre la matriz `{windows, linux, macos} × {x86_64, aarch64}`**, que es lo que convierte la portabilidad de intención en hecho verificado.
- **Pin de LLVM 20.1 documentado y aplicado** en el workspace y en CI (ADR-001, `docs/TOOLCHAIN.md`).

Explícitamente **fuera de alcance**: cualquier sintaxis de Zirk. Al terminar esta change no se parsea un solo `.zrk`. El lexer, parser y demás crates existen pero están vacíos.

## Capabilities

### New Capabilities

- `toolchain-bootstrap`: el toolchain reproducible de construcción del compilador — pin de LLVM, variable de entorno, dependencias por plataforma, y la verificación de que la cadena completa produce y ejecuta un binario nativo.
- `compiler-workspace`: la estructura de crates del compilador, sus límites de responsabilidad y la regla de dependencias entre etapas del pipeline.
- `diagnostics-format`: el contrato de diagnósticos de `ZIRK_COMPILER_SPEC.md` sección 8 — severidad, código estable, ubicación, causa y ayuda.
- `target-matrix`: los targets que Zirk debe poder emitir y las plataformas donde el compilador debe poder construirse, con sus criterios de verificación.

### Modified Capabilities

Ninguna: no existen specs previas en `openspec/specs/`.

## Impact

**Nuevo:**
- `Cargo.toml` raíz (workspace) y nueve `crates/*/`
- `.github/workflows/ci.yml`
- `rust-toolchain.toml` para fijar la versión de Rust

**Ya existente, referenciado:**
- `docs/decisions/ADR-001..005` — fuente durable de las decisiones; este change las referencia, no las duplica
- `docs/TOOLCHAIN.md` — instalación por plataforma

**Dependencias externas introducidas:**
- `inkwell` 0.10 (feature `llvm20-1`) → `llvm-sys` 201 → LLVM 20.1 del sistema
- Requiere `LLVM_SYS_201_PREFIX` en toda máquina de desarrollo y todo runner de CI

**Riesgos:**
- Windows es la plataforma de mayor fricción: exige el tarball oficial de desarrollo de LLVM, no el instalador `.exe`. Si el runner de Windows no logra construir, es un hallazgo bloqueante de esta change, no un detalle a posponer.
- El cross-**linking** (sysroots por target) queda fuera; solo se cubre la emisión de objetos. Trabajo de Fase 6, registrado en ADR-004.
