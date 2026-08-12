# ADR-003 — Estrategia de memoria: restricciones ahora, implementación en Fase 4

- **Estado:** aceptada (restricciones) / abierta (implementación)
- **Fecha:** 12 de agosto de 2026
- **Fase:** 0 (restricciones) → 4 (implementación)

## Contexto

`ZIRK_ROADMAP.md` llama a esta *la decisión de mayor apalancamiento del proyecto* y la ubica antes de escribir código. Pero el subset de Fase 1 (`main`, `println`, `Int32`, literales, `if`/`else`) no aloca prácticamente nada: elegir hoy "GC generacional" sería una decisión indefendible con datos, tomada sobre un lenguaje que todavía no existe.

El riesgo simétrico es real: si no se escribe nada, la IR de Fase 1 nace con asunciones tácitas sobre memoria que después no se pueden sacar.

## Decisión

Se **cierran las restricciones** ahora y se **ancla la elección concreta a Fase 4**.

### Restricciones derivadas del spec (no negociables)

Estas no son preferencias: se deducen de los documentos normativos y acotan el espacio de diseño mucho más de lo que sugiere la pregunta abierta "¿GC, RC, regiones o híbrido?".

| Restricción | Fuente | Implicación |
|---|---|---|
| Ownership y RC **no** son semántica pública | `SPEC_FINAL` §3 | nada al estilo de Rust en la superficie del lenguaje |
| Los ciclos deben liberarse correctamente | `RUNTIME_SPEC` §9 | **RC puro queda descartado**: hace falta trazado o un cycle collector |
| Identidad estable aunque el objeto se mueva físicamente | `RUNTIME_SPEC` §9 | un GC movible exige handles o pinning; choca con la frontera ABI C (`LANGUAGE_SPEC` §13) |
| No hay destructores de propósito general con momento observable | `RUNTIME_SPEC` §9 | **el GC no necesita finalizadores** — simplificador mayor |
| Los recursos externos se cierran vía `Resource<E>` + `match with`, no vía liberación de memoria | `RUNTIME_SPEC` §10 | la vida de archivos, sockets y locks es independiente del GC |
| Pausas y consumo deben medirse | `RUNTIME_SPEC` §9 | se admite GC, pero con presupuesto explícito y observable |
| `parallel` y `thread` son reales y multinúcleo | `RUNTIME_SPEC` §6, §7 | lo que se elija debe ser thread-safe por diseño, no adaptado después |
| Los tipos de valor pueden almacenarse inline | `RUNTIME_SPEC` §9 | value classes y records no pagan indirección |

### Dirección probable (no vinculante)

El espacio restante apunta a **trazado no movible (o movible con handles) + escape analysis para promover a stack + value types inline**. Se registra como hipótesis de trabajo, no como decisión: la elección definitiva requiere un lenguaje con closures y objetos reales que medir.

### Criterio de decisión para Fase 4

La elección se cerrará evaluando, sobre programas Zirk reales:

1. comportamiento con ciclos entre objetos y con closures que capturan;
2. costo de la barrera (si la hay) en `parallel for`;
3. interacción con `Resource<E>` y con la frontera ABI C;
4. pausas medidas contra un presupuesto declarado.

## Consecuencias

- La IR de Fase 1 **no** debe asumir un modelo de memoria concreto: toda alocación pasa por una operación de IR abstracta, resuelta por el runtime.
- `zirk-runtime` ([ADR-002](./ADR-002-runtime-staticlib.md)) es el punto único donde esta decisión se materializa.
- Este ADR se revisa y reemplaza al inicio de Fase 4. No se considera cerrado hasta entonces.
