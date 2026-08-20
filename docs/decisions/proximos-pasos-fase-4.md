# Próximos pasos tras `fase-4a`/`fase-4b`/`fase-4c`

- **Estado:** documento de planificación (no es un ADR, no fija ninguna decisión)
- **Fecha:** 20 de agosto de 2026
- **Contexto:** investigación y planificación pura — no toca `crates/*/src/*.rs`

## 0. Punto de partida

`fase-4a-errores`, `fase-4b-excepciones` y `fase-4c-recursos` están en `develop`,
con los tres `tasks.md` completamente marcados (`[x]` en cada ítem, incluyendo
sus secciones de cierre — `cargo test --workspace`, `clippy`, `fmt`) y
`openspec validate <change> --strict` pasa limpio para los tres. `openspec
list` los reporta como `✓ Complete`. No queda trabajo de implementación
pendiente dentro de lo que estos tres cambios se propusieron hacer.

Eso no significa que la Fase 4 del roadmap (`docs/init/ZIRK_ROADMAP.md`,
"Errors and memory") esté terminada — cubrieron deliberadamente un subconjunto
de ella. Lo que sigue es el inventario de lo que quedó fuera, a propósito, en
cada uno de los tres `proposal.md`.

## 1. Inventario de lo pendiente dentro de la Fase 4

### 1.1 De `fase-4a-errores` (Result<T,E>)

- **Combinadores genéricos de `Result`** (`map`, `map_error`, `and_then`,
  `or_else`, `or_throw`): necesitan un parámetro de tipo propio de método
  (`map<U>(...)`), que `MethodInfo` no soporta hoy — es una pieza de
  inferencia de tipos nueva, no una extensión menor. `or_throw` además
  necesita excepciones (ya existen desde `fase-4b`, así que esa dependencia
  concreta ya cayó).
- **`get_or_else(factory: Fn() => T): T`**: bloqueado por la decisión D9
  (tipos función `Fn(...) => R` sin sintaxis anotable en ninguna posición del
  lenguaje). Sigue bloqueado — D9 no se ha tocado en ninguna de las tres
  fases posteriores.
- **Propagación automática (`?`)**: no es deuda, es una decisión de lenguaje
  explícita ("Zirk 1.x has no `?` propagation operator"). No aplica como
  pendiente real.

### 1.2 De `fase-4b-excepciones`

- **Trazas de pila estructuradas** (`stack_trace()` retorna un `StackTrace`
  real pero vacío hoy) y **`suppressed`** poblado desde una falla de limpieza
  en `finally`. `suppressed` además depende de `List<T>` (Fase 7).
- **Conversión de las fallas de runtime nativas** (división por cero,
  overflow, cast inválido, shift inválido, repeat inválido) **a
  `RuntimeError` catcheable**: investigado y explícitamente diferido —
  `design.md` documenta D5–D7 con el motivo concreto (el chequeo vive en
  `zirk-codegen-llvm/src/emit.rs`, una capa por debajo de donde
  `zirk-ir/src/lower.rs` construye el mecanismo de `try`/`catch`; moverlo
  requiere insertar los chequeos a nivel de `zirk-ir` en vez de codegen). D7
  ya separa esto en una porción tratable (los cuatro chequeos de
  cero/negativo/rango) y una que necesita spike propio (overflow, por la
  pregunta de instrucciones multi-resultado). Sigue bloqueado, con un plan
  de ataque ya escrito.
- **Patrones de variante en `catch`** (`catch NetworkError.Timeout(duration)`):
  necesita que una excepción de usuario declare variantes internas, mecanismo
  que no existe.
- **`Fn(...) => T throws X`**: bloqueado por D9, igual que arriba.
- **Desenrollado real de pila nativa** (landing pads/personality function):
  deliberadamente no construido — el mecanismo actual (`thread_local`
  pendiente + `try_stack`) es correcto para todo programa que el chequeador
  acepta, pero no es lo que un compilador de producción usaría. No hay
  indicación de que esto sea urgente; es una nota de arquitectura a largo
  plazo, no una tarea con fecha.

### 1.3 De `fase-4c-recursos`

- **Adquisición agrupada** (`match a with x, b with y { ... }`) con cierre
  derecha-a-izquierda cuando una adquisición posterior falla.
- **`ResourceFailure<BodyError,CloseError>`**: hoy `close()` se llama y su
  `Result` se descarta; no se combina con el resultado del cuerpo.
- **`suppressed` poblado desde una falla de cierre durante propagación de
  excepción**: depende de `List<T>` (Fase 7) y de `ResourceFailure`.
- **`TransferableResource`/`transfer()`** y el análisis de
  escape/uso-tras-transferencia: un recurso puede hoy escapar su scope sin
  que el compilador lo detecte — queda como responsabilidad del programador,
  sin verificación. Es la brecha más grande de seguridad que deja `fase-4c`
  abierta.
- **Recursos dependientes que no sobreviven a su padre**, **`take` sobre un
  recurso no clonable en un contenedor**: necesitan colecciones (Fase 7).
- **Cancelación** (`STRUCTURED_CONCURRENCY_SEMANTICS.md`): depende de Fase 5.

### 1.4 El resto de la Fase 4 del roadmap (no cubierto por ninguno de los tres cambios)

- **La estrategia de memoria completa** (`unsafe {}`, `Pointer<T>`,
  referencias safe/weak/dependent, deep clone graph semantics, pinning nativo
  acotado automático): sigue en `ADR-003-memoria.md`, estado "abierta
  (implementación)". Ver sección 2.
- **Transactional write journals y rollback** para rangos
  managed/validated, con `commit` irreversible: no se ha tocado en ninguna
  sesión reciente. Depende de `unsafe`/`Pointer<T>` existiendo primero.
- **`inmut::strict`**: deep immutability con alias analysis. El roadmap es
  explícito en que necesita el mismo análisis que la estrategia de memoria,
  así que no tiene sentido antes de que esa estrategia esté cerrada.

Confirmado con `grep` en esta sesión: `unsafe` produce `E0302` ("not
implemented yet, arrives in Phase 4"), `Pointer<T>` produce `E0402` por la
misma razón, y no existe la palabra clave `extern` en el lexer. Esto no
cambió desde la investigación de `ADR-003-investigacion-fase-4.md`.

## 2. ¿Se puede cerrar ya la estrategia de memoria (ADR-003)?

Mi lectura, después de revisar la sección "Qué preguntas siguen abiertas" del
documento de investigación completo:

**No todavía, pero no por falta total de evidencia — por una brecha concreta
y nombrable.** El documento de investigación ya reunió evidencia de ejecución
real sobre tres de los cuatro criterios de decisión que el propio ADR-003
fija:

- **Criterio 3 (interacción con `Resource<E>`/frontera ABI C):** la mitad de
  `Resource<E>` está respondida con evidencia fuerte (probe 4 — el grafo de
  un recurso permanece correcto durante el desenrollado de una excepción).
  La mitad de la frontera ABI C está confirmada como *inexistente*, no como
  "no medida": `unsafe`/`Pointer<T>`/`extern` no tienen ninguna superficie
  de lenguaje todavía. Esta parte del criterio 3 es genuinamente bloqueante
  — no hay nada contra qué medir hasta que al menos `unsafe {}` y
  `Pointer<T>` tengan un parser/checker mínimo.
- **Criterio 4 (pausas contra presupuesto):** no aplica — no existe ningún
  colector implementado. Bloqueante por definición: no se puede medir la
  pausa de algo que no existe. Pero noto que esto es circular con la propia
  decisión que ADR-003 debe tomar — no es una precondición externa, es la
  implementación misma. No debería tratarse como un bloqueo previo a decidir,
  sino como lo que viene *después* de decidir.
- **Criterio 1 (closures que capturan y escapan):** bloqueado por D9, con una
  aproximación parcial (probes 8–10, captura por campo en vez de closure) que
  el propio documento es cuidadoso en no sobrevender. Este es, a mi juicio,
  el más importante de los tres huecos reales, porque es exactamente el
  patrón que más presiona la elección entre "trazado con escape analysis" y
  cualquier alternativa — es donde escape analysis tiene algo genuino que
  decidir (¿promueve a stack o no?), y hoy no se puede ejercitar.
- **Criterio 2 (costo de la barrera en `parallel for`):** no existe
  `parallel`/`thread` en el lexer. Bloqueante en el mismo sentido que el
  criterio 4 — es post-Fase-5, no una precondición razonable para decidir
  ahora la estrategia de memoria (el roadmap mismo pone la memoria en Fase 4
  y la concurrencia en Fase 5, en ese orden, precisamente porque la memoria
  debe cerrarse primero).

**Mi propia lectura, no solo la del documento:** de los cuatro criterios, dos
(2 y 4) no son bloqueantes reales para *decidir* — son criterios de
*validación posterior a la implementación*, no de *elección previa*. Pedirles
evidencia hoy invierte el orden: no se puede medir la pausa de un GC que aún
no se escribió, ni el costo de una barrera en un `parallel for` que llega en
la fase siguiente. Tratar esos dos como bloqueantes sería, en la práctica,
nunca decidir.

Los dos que sí importan — criterio 1 (closures que escapan) y la mitad ABI C
del criterio 3 — comparten la misma causa raíz: ambos dependen de superficie
de lenguaje que todavía no existe (D9 para el primero, `unsafe`/`Pointer<T>`
para el segundo), no de que falte trabajo de investigación. Ningún volumen
adicional de probes va a destrabarlos; hace falta implementación primero.

**Conclusión:** ADR-003 sigue abierta, y con razón — no por prudencia
genérica sino porque dos de sus cuatro criterios tienen una brecha nombrada y
concreta (D9 y `unsafe`/`Pointer<T>` mínimo) que ningún documento adicional
de investigación puede cerrar sin que el compilador avance primero. La
"inclinación fundamentada" que ya deja escrita `ADR-003-investigacion-fase-4.md`
(trazado con escape analysis + value types inline, RC puro descartado) me
parece razonable y no la contradeciría con nada de lo que revisé aquí — pero
coincido con el documento en que tratarla como si ya estuviera validada
contra el caso que más la pondría a prueba sería prematuro.

## 3. ¿Tiene sentido preparar terreno para la Fase 5 (concurrencia) ahora?

No, y creo que la respuesta es clara, no ambigua. La razón no es solo que el
roadmap la ordene después — es una dependencia real de contenido:

- El propio roadmap describe el paso 3 de la Fase 5 como "compiler-derived
  transfer/share and capture analysis sufficient to enforce the safe-code
  data-race guarantee" — eso es, literalmente, un análisis que necesita saber
  qué es una referencia compartida, qué es propiedad exclusiva, y cómo se
  mueve entre tareas. Ninguna de esas nociones existe todavía en el
  compilador porque son exactamente lo que la estrategia de memoria de la
  Fase 4 tiene que definir primero.
- `Mutex<T>`/`RwLock<T>`/`Atomic<T>` (paso 4 de la Fase 5) necesitan un
  modelo de memoria compartida con garantías de visibilidad — no se puede
  especificar sin saber si el runtime usa trazado, RC, o regiones.
- A diferencia de ADR-003, donde ya existe evidencia real parcial que vale la
  pena documentar (closures, objetos, `Resource<E>`), la Fase 5 no tiene hoy
  ningún lenguaje construible que la ejercite — ni `task`, ni `Channel<T>`,
  ni `parallel`. Escribir hoy un documento de "precondiciones de Fase 5" del
  mismo tipo que se hizo para ADR-003 produciría, en el mejor caso, una
  relectura del roadmap sin evidencia nueva que aportar — y en el peor,
  fijaría expectativas sobre una superficie de lenguaje que la propia
  decisión de memoria todavía puede cambiar.

Mi recomendación honesta: no es momento de tocar la Fase 5 en absoluto, ni
siquiera en modo de lectura/anotación. Esperen a que la estrategia de memoria
esté al menos decidida (no necesariamente implementada por completo) antes de
invertir tiempo ahí — cualquier nota tomada ahora sobre Fase 5 arriesga
quedar obsoleta en cuanto ADR-003 se cierre, porque la forma que tome la
memoria cambia directamente qué análisis de transferencia es siquiera posible.

## 4. Trabajo de cierre pendiente en lo ya implementado

- **`tasks.md` de los tres cambios**: sin ítems sin marcar. Los tres están
  100% `[x]`, incluidas sus secciones de cierre (tests/clippy/fmt en verde,
  `design.md` con su sección `## Decisions` completada).
- **`openspec validate --strict`**: pasa limpio para `fase-4a-errores`,
  `fase-4b-excepciones` y `fase-4c-recursos`.
- **`openspec list`**: reporta los tres como `✓ Complete`.
- **Candidatos a `openspec archive <change> --yes`** (no ejecutado en esta
  sesión, por instrucción explícita — solo se señala): los tres califican.
  Cada uno tiene su `tasks.md` completo, su `design.md` cerrado, y (donde
  aplica) su delta de spec ya escrito — `fase-4c-recursos/tasks.md` ítem 7.3
  documenta explícitamente el delta a `openspec/specs/zirk-resources/spec.md`.
  Sugiero archivar los tres, en el orden en que se completaron
  (`fase-4a-errores` → `fase-4b-excepciones` → `fase-4c-recursos`), antes de
  abrir cualquier cambio nuevo — mantiene `openspec list` limpio y evita que
  un cambio nuevo se confunda por accidente con uno de estos tres si comparte
  archivos tocados.
- **Detalle menor detectado durante la investigación, no una tarea de
  cierre de estos tres cambios**: `crates/zirk-sema/src/types.rs` tiene un
  comentario desactualizado junto a `PHASE_4` (`pending_type`) que sigue
  listando `Resource` como pendiente, cuando `Resource<E>` ya está
  implementado desde `fase-4c` y se resuelve por otro camino (el nombre en
  esa lista es código muerto en la práctica). Es una corrección de una línea
  de comentario, sin efecto funcional — vale la pena que quien toque
  `types.rs` a continuación la arregle de paso, no amerita un cambio openspec
  propio.

## 5. Priorización — qué atacar primero y por qué

Siguiendo el patrón que ya demostró funcionar en esta racha de sesiones
(cambios chicos, verificables, uno a la vez, en vez de fases completas de un
salto), mi recomendación de orden:

1. **Archivar los tres cambios completos** (`fase-4a-errores`,
   `fase-4b-excepciones`, `fase-4c-recursos`) con `openspec archive --yes`.
   Costo mínimo, cero riesgo, y limpia el estado antes de que se abra
   cualquier cambio nuevo — es higiene de proceso, no trabajo de diseño, pero
   bloquea trivialmente la trazabilidad de lo que viene si se deja sin hacer.

2. **D5–D7 de `fase-4b-excepciones`: convertir los cuatro chequeos nativos
   tratables (división por cero, shift inválido, repeat inválido, `NaN`) en
   excepciones catcheables reales**, dejando overflow fuera (como el propio
   D7 recomienda) hasta que la pregunta de instrucciones multi-resultado
   tenga su propio spike. Lo priorizaría antes que cualquier otro ítem de la
   lista de la sección 1 porque: (a) ya tiene diseño escrito y una razón
   concreta de por qué se difirió — no es exploración desde cero; (b) es
   coherente con el patrón "capturar o declarar" que el usuario del lenguaje
   ya espera de cualquier otra excepción, así que cerrar esta asimetría tiene
   valor real de lenguaje, no solo de cobertura; y (c) es un cambio acotado
   (cuatro chequeos, no cinco, con el quinto explícitamente pospuesto) del
   tamaño que esta racha de sesiones ya maneja bien.

3. **Corregir los bugs de compilador que bloquean seguir midiendo ADR-003**
   (los cinco del documento original más el de `?.` sobre un método
   `Void` documentado en la extensión) — con la salvedad de que esto ya está
   siendo trabajado en paralelo por otro agente en esta misma sesión, según
   el contexto de esta tarea, así que no lo recomendaría como "próximo paso"
   propio sino como algo que ya está en curso. Una vez que ese trabajo
   aterrice, valdría la pena un probe adicional dirigido específicamente a
   grafos de objetos más grandes y recorridos recursivos, para terminar de
   ejercitar lo que el probe 3 original dejó incompleto por los bugs que
   encontró.

No incluyo D9 (tipos función) ni `unsafe`/`Pointer<T>` mínimo en esta lista
de "primero" a pesar de que son las dos brechas reales que bloquean cerrar
ADR-003 (sección 2) — son decisiones de diseño de lenguaje más grandes, con
superficie propia (sintaxis, chequeo, inferencia), y no until ADR-003 en sí
se aborde como su propio proceso de decisión tiene sentido dimensionarlas.
Adelantar una implementación parcial de `Pointer<T>` solo para "tener algo
que medir" sin haber decidido primero qué garantías de memoria debe cumplir
sería construir sobre una decisión que aún no se tomó — exactamente el riesgo
que ADR-003 (fase 0) se propuso evitar desde el principio.
