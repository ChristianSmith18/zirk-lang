# ADR-003 — Estrategia de memoria: restricciones ahora, implementación en Fase 4

- **Estado:** aceptada (restricciones y elección de estrategia; ver "Cierre de la decisión" — la implementación del colector es trabajo de Fase 4e en curso, no bloquea el estado de este ADR)
- **Fecha:** 12 de agosto de 2026 (restricciones) — cerrada el 24 de agosto de 2026
- **Fase:** 0 (restricciones) → 4e (cierre e implementación)

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

## Cierre de la decisión (24 de agosto de 2026)

`docs/decisions/ADR-003-investigacion-fase-4.md` reunió evidencia de ejecución real sobre `develop` a lo largo de varias sesiones de Fase 4 — no programas de juguete, el propio criterio que este ADR puso como condición para cerrar. De los cuatro criterios que la sección "Criterio de decisión para Fase 4" fijó:

1. **Comportamiento con ciclos y con closures que capturan** — medido (probes 1, 8-11): los ciclos son el resultado natural de dos clases que se referencian mutuamente, no un caso de laboratorio; una closure que captura un objeto compartido y escapa de su marco creador (una vez que `fase-4d-callables` hizo el caso construible) sobrevive correcta, con identidad y aliasing intactos, incluso bajo presión de alocación sostenida.
2. **Costo de la barrera en `parallel for`** — no medible todavía: `parallel`/`thread` son Fase 5 y no existen. Este criterio no puede cerrarse antes de esa fase por construcción, no por falta de esfuerzo — se deja como validación posterior a la implementación, no como precondición para elegirla.
3. **Interacción con la frontera ABI C** — parcialmente medible desde `fase-4e-unsafe-pointer-extern` (esta sesión): `unsafe`/`Pointer<T>`/`extern` existen ahora; queda como validación de seguimiento una vez el colector exista, para confirmar que un objeto pineado y expuesto a través de la frontera FFI sigue siendo correcto durante y después de la llamada nativa.
4. **Pausas medidas contra un presupuesto** — no medible sin colector; se convierte en criterio de aceptación de la implementación, no de la elección de estrategia.

**Decisión final: trazado no movible (mark-sweep), con enumeración de raíces vía shadow stack a granularidad de función, disparo cooperativo en el propio punto de alocación.**

- **No movible.** Preserva identidad y dirección estable sin handles ni indirección adicional — coherente con que `Pointer.from`/`is` ya asumen, desde `fase-4e-unsafe-pointer-extern`, que un objeto no cambia de dirección. Cierra también, por descarte, la pregunta que la "dirección probable" original dejaba abierta (movible-con-handles vs. no movible): no movible es estrictamente más simple y nada de lo construido hasta ahora paga el costo de moverlo.
- **Trazado (mark-sweep), no conteo de referencias.** Confirmado por el probe 1: los ciclos son un patrón de diseño ordinario, no un caso extremo, así que RC puro queda descartado exactamente como la restricción original ya anticipaba. Un mark-sweep no necesita un colector de ciclos separado: los libera igual que cualquier otra basura.
- **Enumeración de raíces: shadow stack manual a granularidad de función, no LLVM statepoints ni escaneo conservador.** Investigado en esta sesión contra el binding de LLVM real (`inkwell` 0.10 expone el atributo `gc` de una función pero ningún wrapper de los intrínsecos de statepoint — habría que emitirlos a mano, con acoplamiento fino a cada punto de optimización). El shadow stack es viable barato aquí porque cada función ya `alloca` todos sus `Slot` de una vez en el bloque de entrada (`emit.rs`, confirmado en esta sesión) y las excepciones de Zirk no usan unwind de LLVM — solo hay un tipo de salida de función (`Terminator::Return`) que instrumentar, no rutas de excepción especiales.
- **Corrección crítica encontrada en esta sesión, antes de cualquier implementación:** un valor de tipo referencia gestionada que vive solo como resultado SSA (`ValueId`), nunca escrito a un `Slot`, es invisible para un shadow stack que solo mira slots con nombre — ejemplo concreto, `f(SomeClass(a), SomeClass(b))` puede recolectar `SomeClass(a)` mientras evalúa el segundo argumento, si el segundo dispara el colector. La resolución: todo valor de tipo referencia gestionada se vierte a un slot sintético propio apenas se produce, antes de participar en cualquier expresión que pueda alocar — la misma técnica que `lower_throws_check` (`fase-4b`) ya usa por una razón de validez de bloque distinta, aplicada aquí por razón de solidez del colector.
- **Cabecera de objeto crece de una palabra a tres** (`ADR-012` ya reservó esto a propósito): descriptor de despacho (sin cambio), un puntero `next` nuevo que enhebra la lista intrusiva de todo lo alocado para el barrido, y el tamaño de la alocación (para poder liberar con `dealloc` correctamente). El bit de marca se esconde en el bit bajo del puntero `next` — un campo enteramente nuevo que solo el propio colector lee, así que ningún sitio existente que ya lee el descriptor sin máscara (`zirk_rt_contract_table`, `zirk_rt_check_cast`, `zirk_rt_is_instance`, y los sitios de codegen que despachan por él) necesita tocarse.
- **Disparo cooperativo dentro de `zirk_rt_alloc`**, sin hilos: si el umbral configurado se supera, colecta antes de servir la alocación. Correcto por construcción hasta que la Fase 5 introduzca concurrencia real — en ese punto, el disparo y el "stop the world" necesitan revisarse, y queda anotado como trabajo de esa fase, no de esta decisión.

### Alternativas descartadas

- **LLVM statepoints (raíces precisas vía intrínsecos del propio LLVM).** Más "correcto" en el sentido de que LLVM ya sabe optimizar alrededor suyo, pero sin wrapper en el binding que este compilador usa, poco documentado fuera de compiladores JIT como el de la JVM o Julia, e interactúa de forma no trivial con inlining y otras pasadas de optimización. Se descarta para esta primera implementación real; queda como mejora futura si el shadow stack manual resulta costoso en la práctica.
- **Escaneo conservador de pila (estilo Boehm-Demers-Weiser).** Cero cambios de codegen para enumerar raíces, pero introduce falsos positivos (un entero que por casualidad parece una dirección válida retiene basura) exactamente donde este ADR ya es estricto sobre identidad y comportamiento indefinido. Descartado por ahora porque el shadow stack manual, dado que este compilador ya trackea slots con nombre, no cuesta sustancialmente más y no paga esa incertidumbre.
- **Movible con handles.** Añade una capa de indirección permanente (todo acceso a un objeto pasa por un handle, no por su dirección) que nada de lo construido hasta ahora necesita — ni `Pointer.from`, ni el despacho por descriptor, asumen indirección. Se descarta hasta que exista una razón concreta (por ejemplo, compactación real bajo presión de fragmentación) que la justifique.

## Consecuencias del cierre

- `crates/zirk-runtime/src/memory.rs`'s "no libera, deliberadamente" deja de ser el estado final: la implementación del colector (Fase 4e, en curso) reemplaza `zirk_rt_alloc`'s cuerpo actual por una versión con umbral, y añade el módulo de mark-sweep, el shadow stack, y el crecimiento de cabecera descritos arriba.
- `ADR-012` (layout de objetos) queda ejercido exactamente como anticipó: la cabecera crece sin desplazar los índices de los campos reales.
- `Weak<T>` y el contrato `Clone` (`MEMORY_AND_UNSAFE_SEMANTICS.md` §4 y §6) pasan de "sin estrategia sobre la cual construirse" a implementables — son la extensión natural una vez que el colector real distingue vivo de muerto.
- El criterio 2 (barrera en `parallel for`) y el 4 (pausas contra presupuesto) quedan como validación de seguimiento post-implementación, no como condición de cierre — este ADR documenta por qué pedirlos como precondición era circular (dependían de fases posteriores a la que este ADR gatea).
