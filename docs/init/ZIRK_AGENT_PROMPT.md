# Prompt inicial — Zirk

Este documento es el prompt de arranque para cualquier sesión de trabajo (agente de código, o vos mismo retomando el proyecto después de un tiempo). Pegalo entero al inicio de cada sesión nueva, o guardalo como instrucción persistente del agente si tu herramienta lo permite.

---

## Contexto del proyecto

Estás ayudando a construir **Zirk**, un lenguaje de programación compilado, orientado a objetos, con tipado estático e inferencia, de alto nivel por defecto y con acceso opcional a bajo nivel. Compila a binarios nativos vía LLVM. Concurrencia estructurada (`task`/`await`) y paralelismo multinúcleo (`parallel`/`parallel for`/`thread`) son características de primera clase.

Filosofía: **fácil por defecto, explícito cuando necesitás control.**

## Fuente de verdad

Los siguientes documentos, en la raíz del repo, son la especificación normativa completa del lenguaje. Ante cualquier duda de sintaxis, semántica o alcance, **estos documentos mandan sobre criterio propio, memoria de otros lenguajes, o "lo que suena razonable"**:

- `ZIRK_SPEC_FINAL.md` — alcance, exclusiones explícitas, filosofía general.
- `ZIRK_LANGUAGE_SPEC.md` — sintaxis, tipos, objetos, control de flujo, errores.
- `ZIRK_COMPILER_SPEC.md` — pipeline, IR, LLVM, targets, diagnósticos, CLI.
- `ZIRK_RUNTIME_SPEC.md` — memoria, tasks, scheduler, threads, recursos.
- `ZIRK_STDLIB_SPEC.md` — módulos y contratos de la biblioteca estándar.

Si dos documentos se contradicen entre sí: `ZIRK_SPEC_FINAL.md` define alcance y exclusiones; el documento especializado define la semántica de su área.

**Regla explícita del propio spec, y regla de trabajo acá:** toda ambigüedad debe producir una pregunta o quedar documentada — nunca resolverse en silencio inventando comportamiento no especificado. Si algo hace falta para avanzar y el spec no lo cubre, decilo explícitamente antes de decidir por tu cuenta.

## La regla de alcance más importante de este prompt

El spec completo describe a Zirk en su versión madura — es el resultado de años de trabajo, no el punto de partida. **No se implementa el spec completo de una.** El trabajo avanza por fases (ver `ZIRK_ROADMAP.md`). En cada sesión, el objetivo es avanzar la fase actual — no adelantarse a features de fases futuras aunque estén documentadas, definidas, y sea tentador implementarlas ya que "está todo ahí".

Si en el camino aparece la necesidad real de algo de una fase posterior para que la fase actual funcione, decilo explícitamente en vez de implementarlo a medias o de forma silenciosa.

## Stack de implementación

- **Lenguaje del compilador: Rust.** La razón no es estética: la optimización de los binarios que produce Zirk la hace LLVM en el backend, no el lenguaje en el que está escrito el compilador — eso ya lo resuelve `ZIRK_COMPILER_SPEC.md` sección 5. Lo que sí depende del lenguaje del compilador es qué tan rápido y seguro se puede construir y mantener algo de este tamaño; Rust da seguridad de memoria, paralelismo seguro para el propio compilador (relevante para la compilación incremental que pide el spec), y bindings maduros a LLVM.
- **Backend de codegen: LLVM**, vía `inkwell` (bindings seguros de Rust sobre `llvm-sys`).
- **Estructura: Cargo workspace**, un crate por etapa del pipeline (lexer, parser, ast, sema/types, ir, codegen, cli) en vez de un binario monolítico — refleja directamente el pipeline de `ZIRK_COMPILER_SPEC.md` sección 2, y permite que cada etapa se testee de forma aislada.

## Fase actual: Zirk 0.1 — pipeline mínimo de punta a punta

Objetivo único de esta fase: que `zirk run` sobre esto compile vía LLVM y corra como binario nativo real —

```
fn main(): Void {
    stdout.println("Hola desde Zirk");
}
```

Nada de intérprete puente ni transpilación temporal. El pipeline completo (lexer → parser → chequeo mínimo de tipos → IR → LLVM IR → binario) tiene que funcionar de punta a punta, aunque el subset de lenguaje soportado sea mínimo.

**Fuera de alcance en esta fase** (no lo toques todavía, aunque esté en el spec): genéricos, clases, concurrencia, decoradores, `init.zrk`, package manager, LSP, formatter, linter, stdlib más allá de un `println` mínimo.

## Estilo de trabajo esperado

- Priorizá que algo compile y corra de punta a punta por sobre completitud de una sola capa. Un pipeline flaco pero completo vale más ahora que un parser exhaustivo sin backend conectado.
- Cada feature nueva debería venir, idealmente, con: gramática mínima, un caso válido de test, un caso inválido de test — como pide el propio spec en `ZIRK_SPEC_FINAL.md` sección 8, aunque en esta fase no hace falta que sea exhaustivo.
- Los diagnósticos, desde el día uno, deberían apuntar al formato de `ZIRK_COMPILER_SPEC.md` sección 8 (código, ubicación, causa, ayuda) aunque el contenido sea básico — es mucho más fácil mantener el formato desde el principio que migrarlo después.
- Preguntame antes de tomar decisiones de arquitectura de alto impacto (estrategia de memoria, estructura de crates, formato interno de IR) que no estén ya resueltas en `ZIRK_ROADMAP.md` fase 0.
