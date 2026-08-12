# Zirk — Roadmap de construcción

Este roadmap traduce las cinco specs normativas (que describen a Zirk maduro) en un orden de construcción real. Cada fase tiene un objetivo verificable — no se avanza a la siguiente hasta que el objetivo de la actual corre de verdad, no solo "está casi listo".

Ningún lenguaje real se construyó implementando su spec completo de una sola vez. Este documento es la disciplina para que a este tampoco le pase.

---

## Fase 0 — Decisiones antes de escribir código

No es una fase de código, es una fase de decisiones que cascadean a todo lo demás. Escribirlas como ADRs (architecture decision records) cortos, uno por decisión, en `docs/decisions/`.

- [ ] **Estrategia de memoria.** El spec promete memoria automática sin exponer ownership (`RUNTIME_SPEC.md` sección 9) y ausencia de data races en globals compartidas (`LANGUAGE_SPEC.md` sección 2). Definir explícitamente: ¿GC generacional, reference counting con detección de ciclos, regiones, o un híbrido? Esta es la decisión de mayor apalancamiento del proyecto — determina cómo se comportan closures, `parallel for`, y `Resource<E>` más adelante.
- [ ] **Layout del workspace.** Crates propuestos: `zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema` (resolución de nombres + type checker), `zirk-ir`, `zirk-codegen-llvm`, `zirk-diagnostics`, `zirk-cli`.
- [ ] **Sanity check de LLVM.** Antes de escribir una línea de Zirk, confirmar que `inkwell` genera, linkea y ejecuta un binario nativo trivial desde Rust puro. Este paso no depende de nada del lenguaje — es validar que el toolchain funciona en tu máquina/CI antes de construir sobre él.
- [ ] **Formato de diagnósticos.** Implementar el formato de `COMPILER_SPEC.md` sección 8 como su propio crate desde el día uno (`zirk-diagnostics`), aunque al principio solo lo use el lexer. Migrar el formato después de que otras 5 capas ya lo usen es mucho más caro que empezar bien.

**Salida de la fase:** ADRs escritos, workspace creado, `cargo build` compila un binario que invoca LLVM y produce un ejecutable "hola mundo" escrito directamente en Rust (sin ningún parser de Zirk todavía).

---

## Fase 1 — Zirk 0.1: pipeline mínimo de punta a punta

**Objetivo:** `zirk run` sobre un `.zrk` con `fn main(): Void { stdout.println("..."); }` compila real vía LLVM y corre como binario nativo.

Subset de lenguaje: `fn main`, `Void`, `String`, `Int32`, `Boolean`, literales, aritmética básica, `if`/`else`, variables `mut`/`inmut`, un `println` mínimo hardcodeado (no la stdlib completa todavía).

Explícitamente afuera: genéricos, clases, `Result`, concurrencia, decoradores, módulos multi-archivo, `init.zrk`.

**Salida de la fase:** un `.zrk` real, con sintaxis real del spec, compilando a un binario nativo real. Este es el hito que valida que la arquitectura completa (lexer → parser → tipos → IR → LLVM → binario) funciona de punta a punta — todo lo que sigue es extender esta columna vertebral, no construir una nueva.

---

## Fase 2 — Superficie del lenguaje core

- Control de flujo completo: `for`, `for ... in`, `while`, `loop`, `break`, `continue`, `if` como expresión.
- Funciones completas: parámetros opcionales, nombrados, variádicos, valores por defecto, closures/lambdas.
- `match` con exhaustividad básica (sobre enums simples).
- Nullability: `T?`, `?.`, `??`.
- Módulos dentro de un mismo crate: `share`/`import` básicos, sin `init.zrk` todavía.

**Salida:** programas con varias funciones, control de flujo real, y closures — todavía sin clases ni concurrencia.

---

## Fase 3 — Objetos y sistema de tipos

- `class`, `construct`, visibilidad (`public`/`private`/`protected`), herencia simple, interfaces, traits.
- Genéricos con `from` (restricciones).
- Records, value classes, enums algebraicos, unions.
- Casts (`as`, `<T>`, `unsafe` casts).

**Salida:** el subset orientado a objetos del spec funcionando, incluyendo genéricos básicos.

---

## Fase 4 — Errores y memoria

- `Result<T, E>` con `match` exhaustivo.
- `try`/`catch`/`finally`, `fatalError`.
- Implementación completa de la estrategia de memoria decidida en la Fase 0.
- `unsafe {}`, `Pointer<T>`, garantías de seguridad de memoria enforced por el compilador.
- `Resource<E>` y `match with`.

**Salida:** manejo de errores completo y la promesa central del spec — código seguro sin use-after-free, null deref no controlado ni UB — verificable con tests.

---

## Fase 5 — Concurrencia y paralelismo (la parte más difícil y menos trillada)

Esta es la fase de mayor riesgo técnico del proyecto — construirla en sub-pasos, no de una:

1. `task`/`await` sobre un executor propio single-threaded primero (concurrencia estructurada sin paralelismo real todavía).
2. `Channel<T>`, `sync`, `Atomic<T>`.
3. `thread` (threads reales del OS).
4. `parallel`/`parallel for` sobre un pool multinúcleo.
5. Análisis estático de capturas mutables inseguras en `parallel`/`thread` (la garantía de data-race freedom del spec, acotada a globals — no es data-race freedom general).

**Salida:** los cinco primitivos de concurrencia del `RUNTIME_SPEC.md` funcionando con las garantías mínimas que promete el spec.

---

## Fase 6 — Sistema de proyecto y CLI

- `init.zrk` como DSL declarativa (parser propio, no reutiliza el parser de Zirk).
- `project`, `build_targets`, `globals`, `permissions`, `compile_permissions`.
- CLI: `new`, `init`, `run`, `build`, `check`, `test`.
- Cross-compilation real a los targets del spec (`COMPILER_SPEC.md` sección 6).

**Salida:** proyectos multi-archivo reales, con manifiesto y compilación cruzada.

---

## Fase 7 — Stdlib

Orden sugerido por dependencia real, no por el orden en que aparecen en el spec:

`std.io` (ya parcialmente cubierto) → `std.collections` → `std.fs`/`std.path` → `std.time` → `std.process` → `std.task`/`std.thread`/`std.sync` (envolviendo la Fase 5) → `std.json` → `std.net`/`std.http` (la más grande de todas) → `std.crypto` → `std.testing` (`@test`/`@e2e`/`@bench`) → `std.reflect` → `std.system`.

**Salida:** aplicaciones reales no triviales (CLI, backend simple) escribibles en Zirk usando solo stdlib.

---

## Fase 8 — Empaquetado y distribución

- `.zpkg`, `zirk.lock`, `zirk add/remove/install/update`, `zirk package`, `zirk publish`.
- Builds reproducibles (`COMPILER_SPEC.md` sección 7).

---

## Fase 9 — Developer experience

- Formatter canónico e idempotente.
- Linter compartiendo parser/tipos con el compilador.
- LSP con snapshots incrementales.
- Debugger con símbolos y mapeo a `.zrk`.

Esta fase es grande en volumen de trabajo pero baja en riesgo conceptual — nada acá es territorio nuevo, es ingeniería de mucho volumen.

---

## Fase 10 — Metaprogramación

- Decoradores (`fn dec`), Syntax API versionada e inmutable.
- `std.reflect` avanzado.

Se deja para el final a propósito: depende de que el resto del compiler esté estable, porque los decoradores tocan casi todas las capas (parser, tipos, IR).

---

## Fase 11 — Endurecimiento de producción

- Fuzzing, differential testing debug/release, suite de benchmarks públicos (`COMPILER_SPEC.md` sección 1).
- Cobertura completa de la matriz de targets.

---

## Fase 12 — Self-hosting (horizonte largo, años)

Reescribir el compilador en Zirk una vez que el lenguaje sea suficientemente maduro y estable. Es el mismo camino que siguieron Rust (rustc empezó en OCaml), Go (el compilador empezó en C) y Zig (empezó en C++). Ningún de estos proyectos self-hosteó desde el día uno, y Zirk tampoco debería intentarlo antes de tiempo.

---

## Cómo usar este roadmap

- Una fase se da por completa cuando su "Salida" corre con tests reales, no cuando el código "está casi".
- Si en medio de una fase aparece la tentación de adelantar algo de una fase posterior, se anota como nota pendiente y se sigue con la fase actual — no se implementa a mitad de camino.
- Este documento es vivo: si una fase resulta mal dimensionada (muy grande o con dependencias no previstas), se ajusta acá mismo, no se improvisa en el código.
