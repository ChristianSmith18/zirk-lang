## Context

La Fase 1 dejó un pipeline de punta a punta pero un lenguaje sin nada para *estructurar* un programa: sin bucles, sin funciones con forma real, sin manera de decir "esto puede no tener valor" y sin manera de repartir código en más de un archivo. Esta fase llena esa superficie, deliberadamente sin tocar objetos (Fase 3) ni concurrencia (Fase 5).

A diferencia de Fase 1, acá no se diseña una IR desde cero: se extiende la de ADR-007. El criterio es el mismo que entonces — lo que no se puede añadir después sin rehacer se decide con cuidado ahora; lo demás se pospone.

ADRs vigentes que restringen esta fase:

| Decisión | ADR |
|---|---|
| La IR no asume modelo de memoria | [ADR-003](../../../docs/decisions/ADR-003-memoria.md) |
| Forma de la IR: tres direcciones, bloques básicos, slots | [ADR-007](../../../docs/decisions/ADR-007-forma-de-la-ir.md) |
| `String` es opaco tras el runtime | [ADR-005](../../../docs/decisions/ADR-005-representacion-string.md) |

## Goals / Non-Goals

**Goals:**

- Bucles (`for`, `for ... in`, `while`, `loop`) con `break`/`continue`, y `if` utilizable como expresión.
- Funciones con parámetros opcionales, nombrados, variadic y valores por defecto; closures con captura inmutable.
- `match` exhaustivo como expresión y como sentencia, sobre un subconjunto acotado de constructores.
- `T?`, `?.`, `??` funcionando de punta a punta, incluyendo su interacción con el chequeo de tipos y el codegen.
- `share`/`import`/`use` resolviendo nombres entre archivos de un mismo crate.
- Cada regla nueva con un caso válido y uno inválido en tests, como en Fase 1.

**Non-Goals:**

- Clases, herencia, interfaces, traits, genéricos, records, uniones, enums con datos asociados (Fase 3).
- `Result`, manejo de errores, estrategia de memoria real (Fase 4).
- Captura mutable compartida en closures: requiere el análisis de concurrencia de Fase 5, que no existe todavía.
- `init.zrk`, paquetes externos, múltiples crates (Fase 6, Fase 8).
- Destructuring de patrones, `match with` sobre `Resource<E>`.
- Cross-compilation, rendimiento del compilador, LSP.

## Decisions

### D1 — `match` exhaustivo llega con un enum mínimo, no con los enums algebraicos de Fase 3

`ZIRK_ROADMAP.md` pone `match` con exhaustividad "sobre enums simples" en Fase 2, pero "enums algebraicos" en Fase 3 junto con records, value classes y uniones — la misma frase del roadmap parece pedir algo que su propio roadmap todavía no permite construir.

La resolución: esta fase introduce una forma reducida de `enum` — un conjunto cerrado de constructores **sin datos asociados** (`enum Direction { North, South, East, West }`), suficiente para que `match` tenga algo real que verificar por exhaustividad. Fase 3 extiende esa misma declaración con datos asociados, genéricos y su integración con el resto del sistema de objetos; no la reemplaza.

**Alternativa descartada:** implementar `match` solo sobre literales y el comodín `_`, sin ningún tipo `enum`. Se descarta porque el roadmap es explícito en pedir exhaustividad "sobre enums", y un `match` que nunca es exhaustivo de verdad no ejercita la regla central del feature.

Patrones admitidos esta fase: literal, constructor de enum mínimo, variable de binding, `_`. Sin destructuring, sin patrones sobre uniones (no existen todavía).

### D2 — Las closures capturan por copia inmutable a través de un entorno opaco, alocado igual que `String`

```
   closure literal  ──▶  IR: alloc <env>, captura por valor  ──▶  puntero opaco a función + entorno
```

Igual que D3 de Fase 1 con `String`: la IR no dice "el entorno vive en el heap" ni "vive en el stack". Emite una operación abstracta de alocación (ya existente, ADR-003) para el entorno de captura, y el runtime decide.

La captura es **por valor, inmutable**: al crear la closure se copian los valores capturados a su entorno. Esto es consistente con `ZIRK_LANGUAGE_SPEC.md` sección 6 ("una closure captura valores inmutables de forma segura") y evita que esta fase tenga que resolver aliasing entre una closure y su scope de origen, que es exactamente lo que Fase 5 (concurrencia) todavía no tiene con qué analizar.

**Alternativa descartada:** captura por referencia con el entorno apuntando al stack frame original. Se descarta de plano: si la closure escapa la función que la creó — el caso de uso que la hace útil — el stack frame ya no existe. Requeriría exactamente el análisis de escape que Fase 4/5 todavía no construyen.

### D3 — `for ... in` itera sobre un protocolo mínimo cerrado, no sobre traits de iteración

`ZIRK_LANGUAGE_SPEC.md` no define todavía un trait `Iterable`/`Iterator` — eso depende de traits, que son Fase 3. Sin embargo `for ... in` está pedido esta fase.

Se resuelve acotando `for ... in` a los tipos que el compilador conoce de forma intrínseca esta fase: rangos (`0..N`, azúcar sintáctica reconocida por el parser) y `String` iterada por carácter. No hay protocolo extensible por el usuario todavía — un `for x in mi_tipo` sobre un tipo no reconocido falla con un diagnóstico que indica que la iteración de tipos propios llega con los traits de Fase 3.

Es la misma estrategia que D4 de Fase 1 usó para `println`: un intrínseco acotado y con fecha de retiro documentada, preferido sobre construir medio sistema de traits antes de tiempo.

### D4 — Los parámetros opcionales, nombrados y variadic se resuelven en `zirk-sema`, no en el ABI

La firma siguen siendo posiciones fijas en la IR y en el codegen: `zirk-sema` reordena argumentos nombrados a posición, sustituye ausentes por su valor por defecto (evaluado en el sitio de llamada) y empaqueta los variadic en un valor de secuencia antes de bajar a IR. La IR y LLVM nunca ven "argumento nombrado" ni "argumento faltante" — ven una llamada normal, de aridad fija, ya resuelta.

**Alternativa descartada:** ABI variádica real (estilo `printf`). Se descarta porque el spec no pide interoperar con C variádico, y una ABI variádica complica el codegen de todas las llamadas para un beneficio que ningún requisito exige todavía.

### D5 — `T?` es un tipo, no una anotación cosmética; `??` baja a chequeo explícito de nulidad en la IR

`T | Null` se representa en `zirk-sema` como su propio tipo, distinto de `T`, que participa en el chequeo de asignación y de argumentos como cualquier otro. `zirk-ir` baja `a ?? b` a una comprobación explícita de nulidad con dos ramas — igual que un `if` —, con `b` como rama alternativa. No hay una instrucción mágica de coalescencia: es azúcar que el lowering expande, consistente con D6 de Fase 1 (nada se inventa por debajo del nivel de la IR que el spec no pida).

`?.` usaría el mismo mecanismo, pero se difiere por la razón de D8.

### D6 — Los módulos resuelven nombres entre archivos del mismo crate en una pasada de resolución previa al chequeo de tipos

`share` marca una declaración como visible fuera de su archivo; sin `share`, una declaración solo es visible dentro del archivo que la define. `import { X } from "./ruta"` trae ese nombre al scope del archivo que importa. No hay todavía `public`/`private`/`protected` de tres niveles — eso es Fase 3, sobre clases — así que la visibilidad de esta fase es binaria: compartida o privada al archivo.

Se resuelve como una pasada nueva antes del chequeo de tipos existente: dado el conjunto de archivos de un crate, se construye un grafo de `import`, se detectan ciclos (error, con el ciclo mostrado) y se anota qué declaración `import` resuelve a cuál. El chequeador de tipos de Fase 1 no cambia su forma: solo su tabla de scopes ahora puede contener entradas resueltas desde otro archivo.

**Alternativa descartada:** resolución perezosa símbolo por símbolo durante el chequeo de tipos. Se descarta porque no permite detectar ciclos de import de forma clara ni antes de que el chequeo de tipos ya esté a medio camino con errores en cascada.

### D7 — `if` como expresión exige compatibilidad de tipos entre ramas; sin ello, sigue siendo sentencia

`if`/`else` con ambas ramas presentes y de tipo compatible es una expresión que produce un valor, según `ZIRK_LANGUAGE_SPEC.md` sección 5. `if` sin `else`, o con ramas de tipos incompatibles, sigue siendo válido pero solo como sentencia: usarlo donde se espera un valor es un error de tipos con un diagnóstico que explica cuál de las dos condiciones falta (rama faltante o tipos distintos).
### D8 — `?.` se difiere a Fase 3: no hay miembros que acceder todavía

Descubierto al implementar, no al planificar, y vale registrarlo en vez de forzarlo.

`ZIRK_ROADMAP.md` pone `T?`, `?.` y `??` juntos en Fase 2, como si fueran una sola característica. No lo son: `T?` y `??` operan sobre el valor completo y funcionan sin nada más, pero **`?.` es acceso a un miembro**, y en Fase 2 no existe ningún tipo con miembros — las clases, los records y los traits son todos Fase 3. Un `usuario?.nombre` no tiene qué nombrar.

Se implementan entonces `T?`, `null` y `??` de punta a punta, y `?.` emite el diagnóstico de fase que ya usa el resto de construcciones no implementadas, indicando Fase 3. El token ya lo reconoce el lexer desde Fase 1, así que el diagnóstico es preciso y no un "token inesperado".

**Alternativa descartada:** inventar un acceso a miembros acotado solo para que `?.` tenga algo que hacer. Contradice la regla del proyecto de no inventar comportamiento no especificado, y adelantaría a Fase 2 una decisión —qué es un miembro, cómo se resuelve— que pertenece al diseño de objetos de Fase 3.

La nulabilidad que sí se entrega en esta fase no es cosmética: `T?` participa en el chequeo de asignación, de argumentos y de retorno, y `??` baja a comprobación real de nulidad. Lo que espera es el operador de navegación, no el sistema de tipos que lo sostiene.

### D9 — Los operadores compuestos y de incremento entran en esta fase

`+=`, `-=`, `*=`, `/=`, `%=`, `++` y `--` no aparecen en la lista de Fase 2 del roadmap, pero el lexer de Fase 1 ya los reconocía y ya los declaraba como pendientes de **Fase 2** — es decir, la Fase 1 los ubicó aquí y la lista del roadmap simplemente no los enumera.

Además esta fase los necesita: el `for` con tres cláusulas de `ZIRK_LANGUAGE_SPEC.md` sección 5 se escribe `for (mut i = 0; i < 10; i++)`. Dejar `++` fuera obligaría a escribir `i = i + 1` en la construcción más común del lenguaje.

Se implementan como azúcar sintáctica que el parser expande a la asignación equivalente: `i += 1` produce el mismo árbol que `i = i + 1`, y `i++` lo mismo. No llegan a la IR como instrucciones propias.

**Limitación deliberada:** la distinción prefijo/postfijo de `ZIRK_LANGUAGE_SPEC.md` sección 4 —que `x++` valga el valor previo y `++x` el nuevo— solo es observable cuando el incremento se usa *como expresión*. Esta fase los admite únicamente como **sentencia**, donde ambas formas son equivalentes, y rechaza su uso en posición de expresión con un diagnóstico explícito. Admitirlos como expresión exige fijar el orden de evaluación de los efectos secundarios dentro de una expresión, que ningún documento normativo define todavía.


### D10 — Una closure no puede escapar en esta fase, así que sus capturas viajan dentro del propio valor

Descubierto al implementar D2, y cambia su costo por completo.

D2 decidió capturar por valor a través de un entorno alocado con la operación abstracta de ADR-003. Al ir a implementarlo apareció el problema: el runtime solo sabe alocar `String`, y una alocación general sería la primera del lenguaje — exactamente la decisión que ADR-003 fija en Fase 4.

Pero esa alocación no hace falta, porque **en esta fase una closure no puede escapar de la función que la crea**. No hay sintaxis para escribir un tipo función en una anotación: `(Int32) => Int32` no existe como tipo, así que una closure solo puede vivir en un local inferido. No se puede retornar, ni pasar como parámetro, ni guardar en un campo — los campos son Fase 3.

Entonces el valor de closure lleva **las capturas dentro de sí**, no un puntero a un entorno:

```
   closure  =  { puntero a función, captura₁, captura₂, … }
```

y el cuerpo del lambda se eleva a una función de módulo cuyos primeros parámetros son las capturas. Llamarla es extraer el puntero, extraer las capturas y llamar con `(capturas…, argumentos…)`.

Consecuencias:

- **Cero alocación.** El valor vive donde viva su slot, en el marco de pila, y no se toca ADR-003.
- **Cada lambda tiene su propio tipo de IR**, identificado por el lambda y no por su firma. Es correcto justamente porque no hay forma de escribir un tipo función: en cada sitio de uso el tipo es estáticamente conocido. Dos lambdas de la misma firma no son intercambiables, y como no hay dónde declararlas intercambiables, no se nota.
- **La captura sigue siendo por valor e inmutable**, tal como D2 lo fijó. Nada de D2 se deshace: cambia solo dónde vive el entorno.

Cuando Fase 3 le dé sintaxis a los tipos función, una closure podrá escapar y ahí sí hará falta decidir dónde vive su entorno. Esa decisión llega junto con la de memoria, que es donde corresponde, y no antes de tiempo.

## Risks / Trade-offs

- **El enum mínimo de D1 puede quedar como deuda si Fase 3 no lo extiende con cuidado** → Mitigación: se declara sin datos asociados desde el nombre del requisito ("enum simple"), y el requisito de Fase 3 en el roadmap ("algebraic enums") se entiende explícitamente como extensión de esta misma declaración, documentado acá para que Fase 3 lo herede en vez de descubrirlo.

- **La captura por valor de D2 puede sorprender a quien espera capturar por referencia** → Mitigación: es literalmente lo que pide `ZIRK_LANGUAGE_SPEC.md` sección 6; el diagnóstico ante un intento de mutar una variable capturada debe explicar que la captura es inmutable, no solo rechazar la mutación.

- **D3 acota `for ... n` a rangos y `String` — puede tentar a extenderlo símbolo por símbolo antes de que existan traits** → Mitigación: mismo mecanismo que D6 de Fase 1, diagnóstico explícito con la fase donde se resuelve, en vez de ir agregando tipos ad hoc al intrínseco.

- **D6 introduce una pasada nueva antes del chequeo de tipos, que toca el driver de `zirk-cli`** → Trade-off aceptado: es la única forma de detectar ciclos de import de forma limpia, y el driver ya orquesta etapas (Fase 1, `compile()`), así que agregar una no cambia su forma.

## Migration Plan

Aditivo sobre un pipeline que ya funciona. Cada construcción nueva que hoy falla con el diagnóstico D6 de Fase 1 ("esto existe pero no está implementado todavía") pasa a compilar.

Rollback: revertir el merge. Fase 1 no depende de nada de esta fase.

## Open Questions

- **¿Qué pasa si dos archivos del mismo crate declaran el mismo nombre compartido?** `ZIRK_LANGUAGE_SPEC.md` sección 10 no lo dice.

  Propuesta: error de colisión en la pasada de resolución de D6, sin resolución implícita por orden de archivo — mismo espíritu que Fase 1 tuvo con la ausencia de conversiones implícitas: preferir un error claro a una regla de desempate sorprendente. Se confirma durante la implementación.

- **¿`for x in 0..N` incluye `N`?** El azúcar de rangos no está definido en los documentos normativos leídos para esta fase.

  Propuesta: `0..N` exclusivo, `0..=N` inclusivo, siguiendo la convención más común y evitando que el caso exclusivo (el que se usa para "N veces") necesite escribir `N - 1`. Se confirma durante la implementación y se documenta donde corresponda si difiere de esta propuesta.
