# ADR-003 — Investigación de Fase 4: evidencia real para cerrar la estrategia de memoria

- **Estado:** insumo de investigación (no es una decisión, no cambia el estado de ADR-003)
- **Fecha:** 20 de agosto de 2026
- **Fase:** 4

## Propósito

ADR-003 dejó la elección concreta de estrategia de memoria abierta hasta Fase 4, y dijo explícitamente que cerrarla requiere "un lenguaje con closures y objetos reales que medir" — evaluarla antes, sobre programas de juguete, es el error que el propio ADR-003 se propuso evitar desde la Fase 0.

Con Fase 3 (objetos, clases, closures, genéricos, contratos) y Fase 4a/4b/4c (`Result<T,E>`, `throw`/`try`/`catch`/`finally`, `Resource<E>`/`match with`) ya en `develop` y ejecutando de verdad, ese lenguaje existe. Este documento no toma la decisión de estrategia — es demasiado grande para delegarla sin supervisión directa — sino que reúne evidencia real (programas Zirk que compilan y corren hoy) para que esa decisión se tome con datos en vez de con especulación.

**Nada en `crates/*/src/*.rs` fue modificado para este documento.** Es observación pura: cinco programas de prueba, compilados y ejecutados con el compilador tal como está, más lectura del código fuente relevante.

## Método

1. Lectura completa de `docs/decisions/ADR-003-memoria.md`, `crates/zirk-runtime/src/memory.rs`, y de las tres secciones normativas que ADR-003 cita o que resultaron relevantes: `docs/MEMORY_AND_UNSAFE_SEMANTICS.md` (modelo público de memoria, semántica de `Weak<T>`, contrato de `clone()`) y `docs/ZIRK_LANGUAGE_SPEC.md` §6 (funciones y closures).
2. `grep` de `ADR-003`, `reference count`, `GC`, `arena`, `region` en todo `crates/` — para confirmar qué otras partes del compilador ya anticipan (o dejan espacio para) una estrategia concreta.
3. Cinco programas Zirk (`.zrk`) escritos para ejercitar patrones de memoria genuinamente distintos, compilados y ejecutados con `cargo run -p zirk-cli -- run archivo.zrk`. Se muestran íntegros más abajo porque los detalles de cada uno son el propio hallazgo.

## Lo que el compilador ya anticipa

`crates/zirk-runtime/src/memory.rs` es, tal como su propio comentario declara, el único punto donde la estrategia se materializa: `zirk_rt_alloc` reserva memoria puesta a cero y **nunca la libera**, deliberadamente, hasta que esta decisión se cierre. `crates/zirk-ir/src/ir.rs` e `crates/zirk-ir/src/lower.rs` confirman que la IR solo dice `alloc <tipo>` — ninguna operación de la IR nombra malloc, conteo de referencias o recolección. `crates/zirk-codegen-llvm/src/runtime.rs` cita ADR-003 en el mismo sentido.

`ADR-012-layout-de-objetos.md` (Fase 3, ya aceptado) ya reservó el espacio: la cabecera de un objeto está separada de sus campos precisamente "para lo que la Fase 4 necesite para la memoria — marcas, contadores, lo que la estrategia elegida pida", sin desplazar los índices de los campos reales. Es la única preparación estructural que existe hoy; no compromete ninguna estrategia concreta.

No se encontró ningún indicio de conteo de referencias, GC, arenas o regiones ya implementado o parcialmente modelado en ningún otro punto de `crates/`.

## Los cinco programas y qué mostraron

Los archivos completos quedaron en el scratchpad de esta sesión (no en el repositorio, para no interferir con el trabajo en paralelo de otros agentes); lo relevante de cada uno se resume aquí.

### Probe 1 — un ciclo real de referencias

Dos objetos `Node` enlazados por un campo `mut next: Node?`, construidos con `a.next = b; b.next = a;` después del `construct()` (no hay forma de anudar el ciclo dentro del propio constructor, porque no hay inicialización diferida ni referencias adelantadas). El programa recorre el ciclo dos veces y confirma que se vuelve al mismo objeto.

**Resultado:** el ciclo es perfectamente construible en Zirk tal como está implementado hoy, usando solo clases y campos nulables — nada especial ni forzado. Esto responde afirmativamente, con evidencia y no por inferencia del spec, la restricción de ADR-003 "los ciclos deben liberarse correctamente": los ciclos no son un caso raro hipotético, son el resultado natural de dos objetos que se referencian mutuamente, un patrón de diseño común (observador, padre/hijo bidireccional, listas doblemente enlazadas).

### Probe 2 — closures que capturan un objeto (y un hallazgo no buscado)

Al intentar escribir el escenario que ADR-003 pide medir explícitamente — "una closure que escapa de su marco y mantiene vivo un objeto capturado" — el programa no compiló. La razón es una decisión de diseño ya tomada y documentada en el propio compilador: **decisión D9** (`crates/zirk-sema/src/checker.rs:7519`, `crates/zirk-parser/src/parser.rs:1396`, confirmado en `crates/zirk-sema/tests/typing.rs:3019` con el test `invalid_closure_returned_from_a_function`) prohíbe deliberadamente anotar `Fn(...) => R` como tipo de retorno, de parámetro, de campo o de argumento genérico. El mensaje de error del propio compilador lo dice sin ambigüedad: *"function types exist in the language, but this phase's parser and checker reject the annotation on purpose"*.

La consecuencia práctica: **hoy una closure en Zirk solo puede vivir en un binding local `mut`/`inmut` con tipo inferido, en el scope donde se creó o uno anidado**. No puede devolverse de una función, no puede guardarse en un campo de objeto, no puede pasarse a un parámetro tipado ni a uno genérico. El programa de prueba se reescribió para mostrar lo que sí es posible hoy: dos closures independientes sobre dos objetos `Counter` independientes (mutación aislada, correcta), y dos closures sobre el *mismo* objeto `Counter` (mutación visible entre ambas — captura por referencia compartida, como exige `MEMORY_AND_UNSAFE_SEMANTICS.md` §1–2), todo dentro del marco de `main`.

**Resultado:** el escenario "closure que escapa de su marco creador y mantiene con vida un objeto que de otro modo sería basura" — el que ADR-003 §criterio 1 pide medir explícitamente — **no es construible en el lenguaje tal como está implementado hoy**. No es que el runtime lo maneje mal; es que la superficie del lenguaje todavía no permite construir el caso. Esto no es un defecto del probe: es información real sobre el estado de Fase 3 que ADR-003 necesita para no sobre-alcanzar su propio criterio de decisión.

### Probe 3 — una lista enlazada a mano (y varios bugs reales del compilador)

Como el roadmap deja las colecciones nativas (`Array`, `List`, `Map`, `Set`) para la Fase 7 — confirmado por grep: no existe ese tipo en `crates/zirk-runtime` ni en `crates/zirk-sema` — este probe construye una lista enlazada simple con clases y un campo `next: Node?`, la forma que va a tomar casi toda la presión de memoria "real" hasta que la Fase 7 llegue.

Escribir este probe expuso varios bugs genuinos del compilador, no relacionados con la elección de estrategia de memoria en sí, pero relevantes porque bloquean justamente el tipo de programa que hace falta escribir para evaluarla:

- No existe un operador de "unwrap forzado" sobre `T?`. `?.` no puede aparecer del lado izquierdo de una asignación (`E0301`, *"the left-hand side of an assignment must be a place"*). `match nullable { null => ..., binding => ... }` **no estrecha** el tipo de `binding` a no-nulo en la rama que no es `null`. El recorrido clásico "puntero `previous`/`cursor`" de una lista enlazada necesita escribir `previous.next = ...` donde `previous` puede o no ser null, y hoy no hay forma directa de expresarlo — el técnica del nodo centinela (una cabeza ficticia para que `previous` nunca sea null) es el rodeo que este probe terminó usando.
- `algo_nulable ?? null` — una expresión redundante pero natural ("mantener esto nulable") — **hace panicar la compilación**: `crates/zirk-ir/src/lower.rs:5415`, *"the type of this expression comes from the value it produced"*.
- El hallazgo con más peso: **`objeto.campo = <expresión que contiene `?.`>` produce IR inválida de forma reproducible**, con el error `E0508` *"uses value defined in another block; values do not cross blocks in this IR"*. Se reprodujo en su forma mínima con diez líneas (`a.next = b?.next;`, sin loop, sin match, sin recursión, sin `this`). Asignar el valor derivado de `?.` primero a una variable local, y luego esa local al campo, evita el problema — es el rodeo que usa `remove_front` en el probe final. Esto probablemente explica varios de los fallos intermedios encontrados mientras se escribía este probe (el recorrido de dos punteros, la remoción de un nodo intermedio): todos escribían un valor derivado de `?.` directamente en un campo.
- Una función recursiva sobre `Node?` que hace `match` de null/no-null y combina una llamada recursiva con una lectura `?.` del valor discriminado, en el mismo brazo, produce el mismo `E0508` — es decir, **el recorrido recursivo de una estructura enlazada, la forma natural de recorrerla sin helpers de iteración nativos, está roto hoy**, independientemente del bug de asignación de campos de arriba.

Por estos bugs, el probe final quedó limitado a construir la lista (`push_front`) y remover únicamente el nodo cabeza (`remove_front`), en vez de una remoción de nodo intermedio. Con esa limitación, el programa corrió correctamente: tras `remove_front()`, el nodo que sostenía el valor removido queda sin ninguna referencia alcanzable desde el resto del programa — exactamente el tipo de basura acíclica que cualquier estrategia de recolección real debería reclamar de inmediato.

**Resultado:** más allá del hallazgo de memoria en sí (mutación de estructuras enlazadas produce basura acíclica de forma natural, sin ciclos incidentales), este probe deja evidencia de que **construir programas Zirk "reales" con grafos de objetos hoy tropieza con bugs genuinos y no triviales del compilador**, concentrados en la interacción entre tipos nulables de objeto y asignación de campos. Esto es relevante para ADR-003 de forma indirecta pero real: cualquier medición futura sobre programas más elaborados (grafos más grandes, recorridos recursivos) va a necesitar que estos bugs se corrijan primero — no son un obstáculo de la estrategia de memoria, son un obstáculo para *medirla*.

### Probe 4 — `Resource<E>` sosteniendo un grafo de objetos, con una excepción en vuelo

Una clase `Connection implements Resource<ConnectionError>` que guarda una referencia a otro objeto de heap (`lastEntry: LogEntry?`) como su propio estado. Dentro de un `match ... with`, el recurso registra dos entradas, crea además un objeto `LogEntry` puramente local (nunca almacenado en ningún sitio alcanzable), y lanza una excepción (`BoomError`). `close()` lee `this.lastEntry` — una referencia dentro del propio grafo del recurso — mientras la excepción está activamente desenrollándose.

**Resultado, con salida real:**

```
closed secondary last saw: only entry
1
never stored anywhere reachable
closed primary last saw: second entry
caught: boom while using the connection
done
```

La línea `closed primary last saw: second entry` es el dato central: se imprime *durante* el desenrollado de la excepción, y `this.lastEntry` apunta correctamente a la última entrada registrada antes del `throw`. Esto confirma con evidencia (no solo por lectura del spec) que el `Resource<E>`/`match with` de Fase 4c mantiene vivo y correcto el grafo de objetos propio del recurso — el objeto del propio recurso y todo lo que referencia directamente — durante todo el desenrollado, hasta que `close()` termina. Es exactamente lo que ADR-003 §criterio 3 ("interacción con `Resource<E>`") pide verificar, y aquí queda verificado sobre un programa que ejecuta de verdad, no sobre la lectura del spec.

El objeto local (`scratch`, la entrada "never stored anywhere reachable") no tiene ningún efecto observable una vez que el `match` termina — ni por la ruta normal ni por la ruta de excepción — consistente con ser basura recolectable de inmediato bajo cualquier estrategia real.

### Probe 5 — costo cuantificado de "nunca liberar"

Un loop de 10.000.000 de iteraciones, cada una construyendo el ciclo de dos nodos del probe 1 y descartando inmediatamente toda referencia a él. Bajo cualquier estrategia de recolección real, cada iteración generaría basura antes de que empezara la siguiente.

**Resultado medido** (`/usr/bin/time -l`, macOS arm64, build de desarrollo sin optimizar):

| Iteraciones | Objetos alocados | RSS máximo | Tiempo real |
|---|---|---|---|
| 2.000.000 | 4.000.000 | ~130 MB | 2.09 s |
| 10.000.000 | 20.000.000 | ~644 MB | 2.38 s |

El crecimiento es lineal con el número de objetos vivos-y-nunca-liberados (~32 bytes efectivos por objeto, coherente con una cabecera de descriptor más dos campos de puntero/`Int32` en un `Node` de este tamaño), y el programa termina sin fallar dentro del rango probado. Esto confirma exactamente lo que el comentario de `memory.rs` afirma sin haberlo medido: "un programa de esta fase termina y el sistema operativo reclama todo — ese es el límite, y está escrito en vez de asumido". Un programa de vida corta (CLI, script, batch) tolera hoy sin problema el "nunca libera"; un programa de vida larga (servidor, loop de eventos) no lo toleraría más allá de minutos u horas según la tasa de alocación — dato que ninguna medición anterior había puesto en números.

## Bugs del compilador encontrados (no corregidos aquí)

Para que queden asentados en un solo lugar y no se pierdan entre los cinco probes:

1. `is` entre dos operandos de tipo `T?` (objeto) hace panicar la codegen de LLVM: `crates/zirk-codegen-llvm/src/emit.rs:1740`, *"Found StructValue but expected the IntValue variant"*.
2. `is` entre un operando `T` y uno `T?` produce IR inválida: `E0508`, *"Identical between object? and object"*.
3. `nulable ?? null` hace panicar la lowering a IR: `crates/zirk-ir/src/lower.rs:5415`.
4. `objeto.campo = <expresión con `?.`>` produce IR inválida (`E0508`, *"values do not cross blocks"*) — repro mínima de 10 líneas, sin `this`, sin loop, sin match.
5. Una función recursiva que hace `match` de un `T?` y combina la llamada recursiva con una lectura `?.` del discriminante en el mismo brazo produce el mismo `E0508`.

Se dejan documentados porque bloquean directamente el tipo de programa que Fase 4 necesita seguir escribiendo para terminar de cerrar esta decisión — no porque este documento deba resolverlos.

## Qué preguntas de ADR-003 quedan respondidas

- **¿Puede el lenguaje construir ciclos reales hoy?** Sí, trivialmente, con clases y campos nulables (probe 1). La restricción "los ciclos deben liberarse correctamente" no es hipotética: aplica a un patrón de diseño ordinario, no a un caso extremo.
- **¿Interactúa bien `Resource<E>`/`match with` con un grafo de objetos, incluso durante el desenrollado de una excepción?** Sí, verificado con ejecución real (probe 4): el propio recurso y lo que referencia directamente permanecen correctos hasta que `close()` termina.
- **¿Qué forma toma la basura que produce la mutación ordinaria de estructuras (no ciclos a propósito)?** Acíclica y de alcance inmediato: remover un nodo de una lista, o dejar un objeto local sin capturar, produce basura sin referencias alcanzables desde ningún lado en el instante en que el scope que lo sostenía termina (probes 3 y 4).
- **¿Cuánto cuesta "nunca liberar" en términos concretos?** Lineal, medido: ~32 bytes efectivos por objeto de este tamaño, sin fallar hasta 20 millones de objetos vivos-y-descartados en esta sesión (probe 5). Suficiente para cualquier programa de prueba de este proyecto hasta ahora; no representativo de una carga de servidor.

## Qué preguntas siguen abiertas

- **El criterio 1 de ADR-003 ("comportamiento con closures que capturan") no puede medirse aún**, porque el escenario que le da sentido — una closure que escapa de su marco creador — no es expresable en el lenguaje mientras la decisión D9 siga en pie. Esto no es una limitación de este documento: es un hueco real entre lo que ADR-003 pide medir y lo que Fase 3 implementó. Cerrar esta parte de ADR-003 con evidencia real requiere, como precondición, que `Fn(...) => R` deje de estar bloqueado como tipo anotable — o al menos que exista alguna otra vía por la que una closure escape de su marco (un campo, una colección) contra la cual medir.
- **El criterio 2 ("costo de la barrera en `parallel for`") no puede medirse en absoluto**: no se encontró ninguna palabra clave `parallel` ni `thread` implementada en el lexer/parser (`crates/zirk-lexer`, `crates/zirk-parser`). La concurrencia estructurada es, evidentemente, una fase posterior a esta.
- **El criterio 3 ("interacción con la frontera ABI C")** no se ejerció en este documento — los cinco probes son Zirk puro, sin `unsafe`, `Pointer<T>` ni llamadas nativas. `MEMORY_AND_UNSAFE_SEMANTICS.md` §5, §7 y §8 (pines automáticos, `Pointer<T>`, vistas nativas validadas) describen la superficie, pero no se pudo confirmar contra ejecución real si esa superficie ya está implementada; sería el siguiente probe natural.
- **El criterio 4 ("pausas medidas contra un presupuesto")** no aplica todavía: no hay ningún colector implementado sobre el cual medir pausas.
- **`Weak<T>` y `Clone` (`MEMORY_AND_UNSAFE_SEMANTICS.md` §4 y §6), ambos citados por el modelo público de memoria, no tienen ningún rastro en `crates/zirk-sema` ni `crates/zirk-runtime`** — no se encontró `Weak` en ningún `.rs` del compilador. Son parte del contrato que la estrategia elegida tendrá que satisfacer, pero hoy son aspiracionales, no implementados. Vale la pena que quien cierre ADR-003 sepa que diseñar contra esa API es diseñar contra algo que todavía no existe.

## Inclinación fundamentada (no vinculante)

Con la evidencia reunida — no como reemplazo del criterio de decisión, sino como lectura de lo que ya se puede observar — dos cosas parecen relativamente claras y una permanece genuinamente abierta:

**RC puro sigue descartado**, y ahora con evidencia además de por deducción del spec: el probe 1 muestra que un ciclo de dos objetos es exactamente el resultado de un patrón de diseño ordinario (dos clases que se referencian mutuamente), no un caso de laboratorio. Cualquier estrategia final necesita trazado, un cycle collector, o ambos.

**La dirección probable que ADR-003 ya proponía — trazado con escape analysis para promover al stack + value types inline — sigue siendo razonable**, y el probe 5 no la contradice: el "nunca libera" actual sostiene sin esfuerzo cargas de miles a millones de objetos en programas de vida corta, así que no hay urgencia de rendimiento que empuje hacia RC (más simple de implementar pero peor para ciclos) en vez de trazado. Dicho esto, el probe 2 significa que **esta dirección todavía no se puede validar contra el caso que más la justificaría** — closures que capturan y escapan, exactamente donde escape analysis tendría más para decidir entre stack y heap — porque ese caso no es construible en el lenguaje todavía. La inclinación hacia trazado + escape analysis sigue siendo razonable por descarte y por lo que sí se pudo medir, pero **no está confirmada por el caso que la pondría más a prueba**, y quien cierre ADR-003 debería saber que ese hueco existe antes de tratar la dirección probable como si ya estuviera validada.

## Limitaciones de esta investigación

- No hay colecciones nativas (Fase 7): toda estructura de datos de estos probes es una clase escrita a mano. Un `Array<T>`/`List<T>` real podría exhibir patrones de alocación distintos (bloques contiguos en vez de nodos individuales) que estos probes no cubren.
- No se ejerció ninguna interacción con concurrencia (`parallel`/`thread` no implementados) ni con la frontera ABI C/`unsafe` (no evaluada aquí).
- Las mediciones de probe 5 son de un build de desarrollo sin optimizar, en una sola máquina (macOS arm64), y no deben leerse como benchmark definitivo — son evidencia de orden de magnitud, no un número de referencia.
- Los bugs de compilador documentados aquí probablemente restringen qué tan elaborados pueden ser los próximos probes hasta que se corrijan; quien continúe esta investigación debería esperar tropezar con más, dado lo temprano y estrecho que resultó el primero (diez líneas).

## Extensión: criterio 3 y una aproximación al criterio 1

Esta sección se agregó en una sesión posterior a la anterior, sobre el mismo árbol (`develop`), en paralelo con otro agente que corrige los cinco bugs de `?.`/`is`/nulables listados arriba — ese trabajo no se duplicó ni se tocó aquí, y como esos bugs seguían presentes durante esta sesión, los probes de esta sección los evitan activamente en vez de ejercitarlos. Mismo método que el resto del documento: programas `.zrk` reales, compilados y ejecutados con `cargo run -p zirk-cli -- run archivo.zrk` (con `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20`), más lectura del código fuente relevante. Los archivos completos quedaron en el scratchpad de esta sesión, no en el repositorio.

### Criterio 3 — la frontera `unsafe`/ABI C sigue completamente sin implementar, no solo sin medir

El documento original dejó este criterio como "no ejercido"; con evidencia de ejecución real la conclusión es más fuerte: **hoy no hay ninguna superficie que ejercer**. No es una laguna de este informe, es un hueco real del compilador.

- `unsafe { ... }` produce `E0302`, *"`unsafe` is not implemented yet"*, con la causa explícita *"the construct exists in the language but arrives in Phase 4"*. Confirmado en `crates/zirk-lexer/src/token.rs:252`: `Unsafe | Default => Phase::FOUR` en la función `phase()` que gatea qué palabras clave están activas. `unsafe` sigue en esa lista exactamente como lo habían anotado sesiones anteriores.
- `Pointer<T>` como anotación de tipo produce `E0402`, *"type `Pointer` is not implemented yet"*, con causa *"the type exists in the language but arrives in Phase 4"* y ayuda *"the available types are Void, Int32, Boolean, String, and declared enums and classes"*. Confirmado en `crates/zirk-sema/src/types.rs:776`: `const PHASE_4: &[&str] = &["Pointer", "Resource"]` dentro de `pending_type`.
- No existe la palabra clave `extern` en absoluto (`grep` sobre `crates/zirk-lexer/src/token.rs` no encuentra nada): no hay siquiera sintaxis reservada para declarar una función nativa, más allá de lo que `unsafe fn` ya bloquea.

Un detalle que vale la pena que quien cierre ADR-003 conozca: el comentario que acompaña `PHASE_4` en `types.rs` dice *"`Pointer`/`Resource` are still pending"*, pero `Resource<E>` sí está implementado y en uso real desde Fase 4c — el probe 4 del documento original lo ejercita con éxito. Ese nombre en la lista de `pending_type` es código muerto en la práctica (Resource se resuelve antes, por otro camino, igual que `Result<T,E>`), y el comentario quedó desactualizado. No se corrigió aquí porque es código de compilador y está fuera del alcance de esta sesión — se deja anotado para que no confunda a quien lea `types.rs` buscando el estado real de `Resource`.

**Conclusión para ADR-003:** el criterio 3 no solo carece de mediciones — carece de superficie de lenguaje sobre la cual medir. `MEMORY_AND_UNSAFE_SEMANTICS.md` §5–§13 (pines automáticos, `Pointer<T>`, vistas nativas, transacciones `unsafe` con rollback, `commit` irreversible) es, en su totalidad, especificación sin implementación por ahora. Cualquier decisión de estrategia de memoria que dependa de cómo interactúa con la frontera ABI C tendrá que tomarse sin evidencia de ejecución hasta que Fase 4 implemente al menos `unsafe {}` y `Pointer<T>` — este documento no puede acortar esa espera, solo confirmar con precisión dónde está la línea hoy.

### Una aproximación al criterio 1: captura explícita por campo, no closures

El hallazgo central del documento original sigue vigente: una closure no puede escapar de su marco creador mientras la decisión D9 bloquee `Fn(...) => R` como tipo anotable. Esta sección no reabre esa conclusión ni intenta sortear D9. Construye, en cambio, la aproximación más cercana que sí es expresable hoy: un objeto que guarda una referencia a otro objeto en un campo declarado explícitamente, y que sí puede escapar de su marco de creación (retornarse de una función). **Esto no es una closure.** No hay captura implícita del entorno léxico, ni una función anidada con acceso a variables externas — es un objeto con un campo típico, y el objeto que lo contiene es lo que escapa. Se documenta la diferencia sin ambigüedad para que nadie lea estos tres probes como si hubieran resuelto el criterio 1: solo se acercan al patrón de fondo que le da sentido — "¿un objeto capturado por referencia sigue vivo y correcto más allá del scope que lo creó?" — sin tocar la pregunta específica de closures.

**Probe 8 — objeto local capturado en el campo de un objeto que retorna.** `make_holder()` crea `local: Payload` en su propio marco, lo asigna al campo `inner` de un `Holder` recién construido, y retorna el `Holder`. El marco de `make_holder` termina ahí. `main` llama `h.describe()`, que internamente lee `this.inner` con un `match` (sin poder narrowear el tipo, por el bug ya documentado, de ahí el rodeo `binding?.tag ?? "no tag"` dentro del brazo).

Resultado: `created inside make_holder` — el objeto capturado por campo sigue siendo alcanzable y correcto a través del objeto contenedor, después de que el marco que lo creó terminó.

**Probe 9 — dos contenedores con alias al mismo objeto capturado (el análogo del probe 2 original con dos closures sobre el mismo `Counter`).** `make_shared_pair()` crea un `Counter` compartido y dos `Holder` (`first`, `second`) que apuntan al mismo `Counter` a través de un campo **no nulable** (`counter: Counter`, pasado por constructor) — deliberadamente, para no depender de `match`/`?.` sobre tipos nulables en absoluto. Muta el contador dos veces a través de `first.counter.bump()`, descarta `first` y retorna solo `second`.

Resultado: `2` — la mutación hecha a través de un alias antes de que su contenedor fuera descartado sigue siendo visible a través del otro alias, después de que el marco que creó el objeto compartido terminó. Esto es exactamente el comportamiento de captura-por-referencia-compartida que `MEMORY_AND_UNSAFE_SEMANTICS.md` §1–2 exige, verificado aquí sobre objetos que escapan en vez de sobre closures locales como en el probe 2 original.

Nota sobre un bug encontrado al escribir la primera versión de este probe (corregido — ver commit posterior a este ADR): la versión inicial usaba `counter: Counter?` y llamaba `c?.bump()` dentro de un brazo de `match`. Eso hacía **panicar** la compilación en `crates/zirk-ir/src/lower.rs:4660` (línea de entonces), con el mensaje *"a method reachable through `?.` has a nullable form"* — el código intentaba envolver el tipo de retorno del método en `Nullable::of(returns)` para el resultado de la llamada segura, y fallaba cuando ese tipo de retorno era `Void` (`Nullable::of` no puede envolver `Void`). La corrección: el chequeador (`Checker::check_call`, rama `?.`) tipa hoy `objeto?.algo()` como `Void` liso cuando `algo` retorna `Void`, no como `Void?` — igual que `Void?` ya se rechaza como anotación de tipo, por la misma razón (`Void` es la ausencia de un valor, así que tampoco puede estar ausente) — y `lower_safe_method_call` deja de reservar un slot de resultado para ese caso, igual que `lower_match` ya hacía para un brazo `Void`.

**Probe 10 — el mismo patrón del probe 8, leído de vuelta después de una carga sostenida de alocación no relacionada.** Igual que el probe 8 (`Holder` con campo `inner: Payload` no nulable), pero entre el momento en que el objeto escapa y el momento en que se lee de vuelta, `main` ejecuta `churn_garbage(2_000_000)` — el mismo generador de ciclos de dos objetos del probe 5 original, usado aquí solo como fuente de presión de alocación.

Resultado: `survivor` / `survivor` / `1` — el objeto capturado mantiene su identidad y contenido, y sigue siendo mutable correctamente, sin importar cuánta basura no relacionada se alocó en el medio.

**Por qué esto es evidencia débil, y hay que decirlo con esa misma claridad:** el runtime actual nunca libera y nunca mueve nada (`crates/zirk-runtime/src/memory.rs`, confirmado en el documento original). Que el probe 10 pase no prueba que un colector real — con trazado, compactación, o movimiento de objetos — preservaría la misma corrección; solo prueba que, bajo el runtime de hoy, alocar mucho en el medio no corrompe por sí solo un objeto capturado. Es la misma limitación, aplicada aquí a un caso más cercano al patrón de escape que a un colector de verdad.

**Conclusión para ADR-003:** estos tres probes no cierran el criterio 1 — ese sigue bloqueado por D9 exactamente como decía el documento original, y esta sección no propone tocar D9. Lo que sí aportan es evidencia de ejecución real, en el patrón más cercano disponible hoy, de que: (a) un objeto capturado por campo sobrevive correcto más allá del scope que lo creó (probe 8); (b) la semántica de alias/referencia-compartida se preserva a través de ese escape, igual que exige el modelo público de memoria (probe 9); y (c) esa corrección no se degrada bajo presión de alocación sostenida, dentro de los límites de lo que el runtime actual puede realmente poner a prueba (probe 10). Ninguno de los tres sustituye al caso que el criterio 1 pide medir — una closure real que escapa — y quien cierre ADR-003 debería seguir tratando ese caso como no medido, no como aproximadamente medido.

## Extensión: el criterio 1 ya es medible — D9 se resolvió parcialmente

Esta sección se agregó en una sesión posterior (24 de agosto de 2026), después de que `fase-4d-callables` (archivada) y `fase-4d-declaraciones-multiples` (archivada el mismo día) llegaran a `develop`. Mismo método que el resto del documento: un programa `.zrk` real, compilado y ejecutado con `cargo run -p zirk-cli -- run archivo.zrk` (con `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20`), más lectura del código fuente relevante. El archivo completo quedó en el scratchpad de la sesión, no en el repositorio. **Nada en `crates/*/src/*.rs` fue modificado para esta sección.**

El documento original decía, sin ambigüedad, que el criterio 1 de ADR-003 ("comportamiento con ciclos entre objetos y con closures que capturan") **no podía medirse** porque D9 bloqueaba que una closure escapara de su marco creador. Eso ya no es cierto: `fase-4d-callables` resolvió D9 para el caso de un solo literal de closure capturador escrito directamente en el inicializador de un local o en el `return` de una función (diseño D12/D14/D15), precisamente el caso que da sentido al criterio 1. Nadie había corrido ese escenario contra el compilador actual hasta este probe.

### Probe 11 — una closure real que captura un objeto compartido y escapa de su marco creador

`make_incrementer(c: Counter): Fn() => Int32` retorna `(): Int32 => c.bump();` — una closure que captura por valor una referencia a `Counter` (que, por ser tipo referencia, preserva aliasing al mismo objeto de heap, tal como exige `MEMORY_AND_UNSAFE_SEMANTICS.md` §1–2). `main` construye un `Counter` compartido, llama `make_incrementer(shared)` dos veces guardando cada resultado en un local **sin anotación de tipo explícita** (`mut inc_a = make_incrementer(shared);`), invoca ambas closures, luego ejecuta `churn_garbage(2_000_000)` — el generador de ciclos de dos nodos ya usado en los probes 5 y 10 — y vuelve a invocar ambas closures.

**Resultado, con salida real:**

```
1
2
2
3
4
4
```

`inc_a()` primero (1), `inc_b()` después sobre el mismo `Counter` compartido (2), `shared.count` confirma el alias (2) — exactamente el patrón captura-por-referencia-compartida del probe 9, pero ahora con una closure de verdad en vez de un campo de objeto. Después de 2.000.000 de iteraciones de churn (4.000.000 de objetos `Node` alocados y nunca liberados, ~130 MB RSS, consistente con el probe 5), ambas closures siguen funcionando correctamente sobre el mismo `Counter`: `inc_a()` → 3, `inc_b()` → 4, `shared.count` → 4. El marco de `make_incrementer` terminó dos veces antes de que ninguna de estas últimas tres líneas se ejecutara.

**Esto responde, con evidencia de ejecución real, la pregunta que el criterio 1 de ADR-003 pedía medir desde la Fase 0**: una closure que captura un objeto por referencia compartida sobrevive correcta, con identidad y aliasing intactos, más allá del scope que la creó — incluso bajo presión de alocación sostenida no relacionada — bajo el runtime actual (que nunca libera ni mueve nada). Sigue aplicando la misma limitación que el probe 10 ya señalaba: esto prueba que alocar mucho en el medio no corrompe por sí solo una closure escapada bajo el runtime de hoy; no prueba que un colector real con trazado o compactación preservaría la misma corrección — esa sigue siendo la pregunta que Fase 4 debe cerrar con una estrategia concreta.

### Hallazgo no buscado: el alcance exacto de D14 en la práctica

Escribir este probe con el tipo explícito `Fn() => Int32` en `inc_a`/`inc_b` (en vez de dejarlo inferido) lo rompe:

```
error[E0439]: this position cannot hold this closure
   |
34 |     mut inc_a: Fn() => Int32 = make_incrementer(shared);
   |                                ^^^^^^^^^^^^^^^^^^^^^^^^
   = cause: a capturing closure only satisfies a callable type when its own
     literal is written directly at that position; a different closure — or
     one reached through a variable — would need its captures boxed behind a
     uniform representation, which is not implemented yet
```

El diseño D14 de `fase-4d-callables` exige que el *literal* de la closure esté escrito directamente en la posición anotada (el inicializador del local, o el `return` de la función) — el valor que produce **llamar** a una función que retorna `Fn(...) => R`, incluso si esa función solo contiene un único literal en su propio `return`, no cuenta como "escrito directamente" en el local que recibe el resultado de la llamada. Sin anotación explícita, la inferencia de tipos simplemente conserva el `Base::Function` id exacto que ya trae el valor devuelto — sin pasar por ninguna posición anotada — y por eso sí compila. Es un límite real y ya documentado en el propio mensaje de error (D13, la polimorfía general con boxing, sigue sin implementar), no un bug, pero vale la pena que quien use `Fn(...) => R` en programas reales sepa que **anotar explícitamente el tipo de retorno de una función que a su vez retorna otra función capturadora, y luego anotar también el local que recibe esa llamada, es hoy una combinación que se rechaza** — mientras que omitir la segunda anotación no.

### Conclusión para ADR-003

El criterio 1 deja de estar en la lista de "preguntas que siguen abiertas" del documento original. Sigue habiendo una limitación honesta que declarar (el runtime actual nunca libera ni mueve nada, así que esto no sustituye medir contra un colector real), pero la pregunta específica — ¿sobrevive correcta una closure que escapa y mantiene vivo un objeto capturado? — ya tiene evidencia de ejecución real, no solo del spec. Combinado con lo que ya estaba confirmado (probes 1, 3, 4, 5, 8, 9, 10), de los cuatro criterios de cierre de ADR-003 solo el 2 (barrera en `parallel for`, bloqueado por Fase 5) y el 3 (frontera ABI C, bloqueado por `unsafe`/`Pointer<T>` sin implementar) siguen sin ninguna superficie de lenguaje sobre la cual medir; el 4 (pausas contra presupuesto) sigue sin aplicar porque no hay colector implementado. El criterio 1 y buena parte de la evidencia de comportamiento con ciclos y grafos ya están cubiertos.

## Extensión: el gap de escritura por proyección a través de `inmut::strict` queda cerrado

Esta sección se agregó en la misma sesión que la extensión anterior (24 de agosto de 2026), después de implementar `fase-4d-declaraciones-multiples` y de mapear, en modo de exploración, el estado real de cada pieza de la Fase 4e contra el código (no contra la especificación).

Ese mapeo confirmó, leyendo `Checker::check_writable_field` en `crates/zirk-sema/src/checker.rs`, que el gap que `fase-4d-declaraciones-multiples` había documentado (`p.x = 5;` con `p: inmut::strict` compilaba sin rechazo) no era un descuido nuevo: el propio comentario de esa función ya lo admitía por escrito ("where the object came from does not enter into it here"), y el change que introdujo `inmut::strict` (`fase-3-objects-and-type-system`, tarea 5.15) había documentado explícitamente que solo cubría el caso más estrecho — una nueva ligadura inicializada directamente desde el nombre de otra variable — dejando fuera, a propósito, "mutar una proyección... a través de un referente `inmut::strict`".

De las piezas de la Fase 4e, esta era la única completamente autocontenida: a diferencia de `Weak<T>`, `Clone` profundo o un recolector real, no depende de que ADR-003 cierre su elección de estrategia de memoria — es análisis estático de alias en el checker, igual que el resto de la matriz D11.

`fase-4e-inmut-strict-proyeccion` (archivada el mismo día) lo cerró: el checker ahora camina la cadena de proyección (`p.a.b.c = x;`, no solo `p.x = x;`) hasta su ligadura raíz y rechaza la escritura si esa raíz es `inmut::strict`.

**Lo que sigue abierto, para que nadie lea esto como "reachable-alias analysis está completo":**
- Mutabilidad estricta declarada en un campo (`FieldDecl.mutability`), propagándose de forma independiente de la mutabilidad de su contenedor — no implementado.
- Llamar a un método mutador a través de una referencia `inmut::strict` (`strictObj.mutate();`) — un camino de escape distinto (por `this` dentro del método llamado), no cubierto por este mecanismo en absoluto.
- Un hallazgo lateral, no un gap de esta pieza: hoy no existe sintaxis de mutabilidad para parámetros de función (`Param` en `zirk-ast` no tiene campo `mutability`; todo parámetro se liga como `Mutability::Immutable` en el checker) — así que un parámetro `inmut::strict` no puede escribirse en un `.zrk` todavía, y por lo tanto tampoco puede ejercitarse contra este mecanismo.

## Consecuencias

Este documento no cierra ni reabre ADR-003, ni cambia su estado. Es un insumo: la persona que decida la estrategia final de memoria en Fase 4 tiene ahora evidencia de ejecución real donde antes solo había restricciones deducidas del spec, más una lista concreta de qué preguntas de ADR-003 siguen sin poder medirse y por qué — y, desde la primera extensión, una de esas preguntas (el criterio 1) ya tiene esa evidencia. La segunda extensión no es sobre ADR-003 en sí (es un gap de `inmut::strict`, no de la estrategia de memoria), pero queda registrada aquí porque este documento es donde se mapeó el estado real de toda la Fase 4e.
