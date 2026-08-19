## Context

Las dos fases anteriores evitaron alocar. La Fase 1 tuvo una sola cosa alocable —`String`— y la escondió tras la frontera del runtime (ADR-005). La Fase 2 llegó a tener closures y las resolvió **sin alocar nada**: como no pueden escapar, sus capturas viajan dentro del propio valor, en el marco de pila (D10).

Esta fase no tiene esa salida. Un objeto con identidad sobrevive al marco que lo creó, y esa es su razón de ser. Es la tensión central del diseño y la primera decisión de abajo.

Lo demás son decisiones de representación que se vuelven contratos: el layout de un objeto y su tabla de métodos viajan dentro del `.zpkg` de la Fase 8, igual que la forma de la IR (ADR-007). Elegirlas mal cuesta caro después.

ADRs vigentes que restringen esta fase:

| Decisión | ADR |
|---|---|
| La estrategia de memoria se decide en Fase 4; la IR no la nombra | [ADR-003](../../../docs/decisions/ADR-003-memoria.md) |
| El runtime es staticlib con frontera ABI C | [ADR-002](../../../docs/decisions/ADR-002-runtime-staticlib.md) |
| Forma de la IR: tres direcciones, bloques básicos, slots | [ADR-007](../../../docs/decisions/ADR-007-forma-de-la-ir.md) |
| `String` es opaco tras el runtime | [ADR-005](../../../docs/decisions/ADR-005-representacion-string.md) |
| Un `Span` nombra su archivo | [ADR-010](../../../docs/decisions/ADR-010-ubicaciones-multiarchivo.md) |

## Goals / Non-Goals

**Goals:**

- `class` con campos `public mut` por defecto, métodos, múltiples `construct`, `this`, visibilidad y herencia simple.
- Interfaces y traits, con los traits aportando implementación.
- Genéricos `<T>` con restricciones `from`.
- Records, value classes, enums algebraicos y uniones.
- Casts `as` y `<T>`, con fallo controlado en la forma comprobable.
- Retirar las tres deudas que la Fase 2 difirió a esta: `?.`, `+` sobre `String` y `for ... in` extensible.
- Cada regla nueva con un caso válido y uno inválido en tests, como en las dos fases anteriores.

**Non-Goals:**

- Elegir la estrategia de memoria. Se provee la frontera; la elección es Fase 4.
- `unsafe {}` y los casts que reinterpretan memoria.
- La biblioteca de colecciones, y con ella el parámetro variadic que la espera.
- Generadores, `|>` y el resto del estilo funcional de la sección 8.
- Varianza, tipos asociados, genéricos de orden superior: el spec no los pide.
- Sintaxis de tipo función, y con ella closures que escapan (D9).
- Decoradores, reflexión, errores, concurrencia.

## Decisions

### D1 — El runtime aloca objetos por una operación abstracta, y sigue sin elegir estrategia

`ZIRK_LANGUAGE_SPEC.md` obliga a que un objeto tenga identidad, y ADR-003 prohíbe que la IR nombre una estrategia de memoria. Las dos cosas son compatibles si el reparto es el que ADR-002 ya fijó: **la IR expresa `alloc <tipo>` y el runtime lo materializa**, siendo el runtime el punto único donde la decisión se toma.

Entonces esta fase añade al runtime una función de alocación tras la frontera ABI C, y **no decide qué hace por dentro**. Lo que hará en esta fase es lo más simple que satisface el contrato: reservar y no liberar.

Que no libere no es un descuido: es la consecuencia de que ADR-003 ponga la elección en Fase 4. Liberar exige haber decidido *cuándo*, y esa es exactamente la pregunta abierta. Un programa de esta fase termina y el sistema operativo recupera todo.

**Alternativa descartada:** adelantar un recuento de referencias porque "es lo más fácil". ADR-003 ya lo descarta como estrategia final —`RUNTIME_SPEC` §9 exige liberar ciclos y RC puro no puede—, así que construirlo aquí sería trabajo que hay que deshacer, con la trampa añadida de que un RC a medias parece funcionar hasta que aparece el primer ciclo.

Queda anotado como deuda con fecha: el requisito lo declara y la Fase 4 lo retira.

### D2 — Un objeto es una cabecera más sus campos, y la cabecera empieza siendo solo el tipo

> Registrada como decisión durable al cierre de la fase: [ADR-012](../../../docs/decisions/ADR-012-layout-de-objetos.md).

```
   objeto  =  [ descriptor de tipo | campo₁ | campo₂ | … ]
```

El descriptor identifica el tipo en tiempo de ejecución. Es lo que necesitan el despacho dinámico, los casts comprobables y `ZIRK_LANGUAGE_SPEC.md` sección 12 cuando dice que "la identidad básica de tipo siempre existe".

Los campos heredados van **antes** que los propios, en el orden en que la jerarquía los declara. Así el prefijo del layout de una subclase coincide con el de su superclase, y acceder a un campo heredado es el mismo desplazamiento mire quien mire. Es lo que hace que la herencia simple no cueste nada en el acceso.

La cabecera lleva **solo** el descriptor por ahora. Lo que la Fase 4 necesite para la memoria —marcas, contadores, lo que la estrategia pida— se añade ahí, y por eso existe como concepto separado desde el principio en vez de aparecer cuando haga falta.

### D3 — Los métodos virtuales se despachan por una tabla en el descriptor de tipo

> Registrada como decisión durable al cierre de la fase: [ADR-013](../../../docs/decisions/ADR-013-forma-del-despacho.md).

El descriptor apunta a una tabla de métodos, y una llamada a un método virtual es una carga indirecta a través de ella. Es la representación que la herencia simple hace barata: la tabla de la subclase empieza con las entradas de la superclase, en el mismo orden, así que el índice de un método no cambia al heredar.

**Las interfaces no caben en ese esquema** —una clase implementa varias y cada una querría sus índices—, así que cada interfaz implementada lleva su propia tabla, y el descriptor guarda la lista de las que la clase satisface. Una llamada a través de una interfaz busca su tabla y despacha por ella.

**No todo método es virtual.** Un método que ninguna subclase redefine se llama directamente: es la mayoría de las llamadas, y pagar una indirección por todas ellas sería pagar por una generalidad que el programa no usa. Determinar cuáles son redefinibles es información que el chequeador ya tiene, porque conoce la jerarquía completa del crate.

**Alternativa descartada:** despacho por búsqueda de nombre en tiempo de ejecución, al estilo de los lenguajes dinámicos. Es más simple de implementar y más lento en cada llamada, y `ZIRK_COMPILER_SPEC.md` sección 4 pide que la IR conserve información suficiente para devirtualizar — lo que presupone que hay algo que devirtualizar.

### D4 — Un trait es una interfaz que además puede traer implementación, y no un mecanismo aparte

`ZIRK_LANGUAGE_SPEC.md` sección 7 los nombra por separado y los distingue por una sola cosa: "los traits pueden incluir implementación reutilizable". No hay una segunda diferencia en el spec.

Se implementan entonces como el mismo mecanismo: un contrato con métodos, donde una interfaz es el caso en que ninguno trae cuerpo. Un método de trait con cuerpo se copia en la clase que lo adopta si no lo redefine, así que en el layout de tablas no hay nada nuevo.

Que sean el mismo mecanismo no los hace la misma palabra clave: el spec las distingue y el diagnóstico también debe hacerlo, porque escribir `interface` con un cuerpo es un error que merece decirlo así.

**Conflicto entre dos traits que aportan el mismo método:** es un error, y la clase lo resuelve redefiniéndolo. Elegir por orden de declaración sería una regla de desempate silenciosa, que es justo lo que la Fase 1 evitó al no admitir conversiones implícitas.

### D5 — Los genéricos se comprueban una vez y se especializan al bajar

El chequeo ocurre sobre el genérico: `fn f<T from Serializable>(x: T)` se verifica **una sola vez** contra la restricción, no en cada instanciación. Un uso que no cumple `from` falla en el sitio de la llamada, con el tipo concreto en el diagnóstico.

La IR **sí** recibe una copia por combinación de tipos usada. Es lo que `ZIRK_LANGUAGE_SPEC.md` sección 7 llama especializar "donde corresponda", y lo que evita que un `T` genérico tenga que existir en tiempo de ejecución.

**Alternativa descartada:** borrado de tipos con todo pasando por puntero. Haría imposible almacenar value classes inline, que `RUNTIME_SPEC` §9 exige y ADR-003 recoge como restricción no negociable.

Se dejan fuera, porque el spec no los pide: varianza, tipos asociados, genéricos de orden superior y especialización explícita por el usuario.

### D6 — Los operadores se sobrecargan por contratos reservados, y es lo que vuelve `+` sobre `String` concatenación

`ZIRK_LANGUAGE_SPEC.md` sección 4 dice que un operador solo se sobrecarga a través de contratos del lenguaje y que la sobrecarga no altera precedencia ni aridad. Esta fase fija los nombres reservados (`_add`, `_subtract`, etc.), permite implementarlos en tipos del usuario y prohíbe reabrir tipos nativos. `String` implementa internamente el contrato de concatenación y `"a" + "b"` empieza a funcionar.

La consecuencia que importa es de dirección: el chequeador deja de tener una lista fija de tipos por operador y pasa a buscar el contrato. Los enteros y los booleanos siguen resolviéndose de forma directa —son del lenguaje, no de una biblioteca— pero lo hacen por el mismo camino.

### D7 — `?.` llega ahora porque ahora hay miembros

D8 de la Fase 2 difirió `?.` con una razón concreta: accede a un miembro y ningún tipo tenía miembros. La razón desaparece con las clases.

Baja igual que `??`: una comprobación explícita de nulidad con dos bloques, donde la rama presente accede al miembro y la ausente produce `null`. El tipo del resultado es el del miembro, en su forma nulable. El mecanismo ya existe desde la Fase 2 (D5), así que lo que llega es el operador, no la maquinaria.

### D8 — `for ... in` pasa a pedir `Iterable<T>`, y el protocolo cerrado de la Fase 2 se retira

D3 de la Fase 2 acotó `for ... in` a rangos y `String` porque no había traits, y anotó que la iteración de tipos propios llegaría con ellos. Llegaron.

`Iterable<T>` e `Iterator<T>` se definen como contratos del lenguaje, y `for x in e` pasa a exigir que el tipo de `e` implemente `Iterable<T>`. Los rangos y `String` dejan de ser casos especiales del compilador y pasan a implementarlo, que es lo que hace que un tipo del usuario sea indistinguible de uno del lenguaje en un `for`.

### D9 — La Fase 3 no entrega todavía la sintaxis final de tipos función

Era la pregunta abierta con la que arrancó el design, y se resuelve antes de escribir código porque condiciona qué se puede hacer con una closure.

**Límite de implementación de esta fase.** La sintaxis final aceptada es
`Function(P...) => R`, con alias preferido `Fn(P...) => R`, compatibilidad por
firma y closures escapables con almacenamiento automático. Fase 3 todavía no
la implementa: su parser y checker rechazan la anotación con un diagnóstico de
disponibilidad de fase. El rechazo no constituye semántica final ni hace
nominal por expresión al tipo futuro.

En consecuencia, durante toda la Fase 3 una closure:

- **puede** guardarse en una variable local cuyo tipo se infiere;
- **puede** invocarse dentro del alcance donde su tipo concreto se conoce;
- **no puede** anotarse como tipo de parámetro, de retorno ni de campo;
- **no puede todavía** escapar de la función que la crea en el compilador de
  Fase 3;
- conserva temporalmente sus capturas inline, sin prejuzgar la representación
  automática final.

**Por qué no ahora.** Introducir la sintaxis convertiría las closures en valores intercambiables por firma: se podrían guardar en objetos, retornar y recibir como argumento. Eso obliga a decidir **dónde vive el entorno y cuánto dura**, que es una decisión de memoria, y las decisiones de memoria son de la Fase 4 (ADR-003). Sería adelantar exactamente lo que D1 de esta fase se cuida de no adelantar.

**Semántica ya decidida fuera del alcance de entrega.** Las funciones, lambdas,
métodos compatibles y objetos invocables se adaptan al tipo `Fn`; las closures
pueden escapar; los parámetros son contravariantes y los retornos covariantes;
la identidad usa `is`; y la asignación comparte el entorno mientras `clone()`
lo copia profundamente. La fase posterior que implemente esta superficie debe
seguir el checkpoint canónico, no inferir reglas desde la limitación temporal
de Fase 3.

Usar un lambda donde se exige una anotación de tipo produce un diagnóstico que dice que los tipos función pertenecen a una fase posterior, y no "tipo desconocido".

### D10 — Las decisiones autorales posteriores prevalecen sobre los borradores heredados

Las acotaciones autorales de agosto de 2026 fijan la superficie que esta fase comparte con el handbook: no hay shadowing ordinario; `this.nombre` desambigua una captura que colisiona con un parámetro de lambda; los campos son `public mut` por defecto; puede haber varios `construct`; los argumentos nombrados seleccionan y reordenan parámetros; y los enums tradicionales exponen el nombre del caso salvo mapping `->` explícito. Estas reglas se verifican en parser y checker antes de fijar lowering o layout.

### D11 — Las referencias compartidas obedecen la misma matriz de mutabilidad

`String`, arrays, colecciones y clases comparten la regla pública: `mut` permite
reasignar y mutar, `inmut` solo impide reasignar, y `inmut::strict` congela el
grafo alcanzable. Una referencia strict no produce aliases mutables ni puede
adquirirse desde un alias mutable todavía accesible; `clone()` crea una copia
lógica independiente cuando el contrato existe. Fase 3 aplica esta regla a los
objetos y contratos que introduce; la estrategia concreta de análisis y memoria
sigue perteneciendo a las fases correspondientes. Solo mover la variable
completa comparte referencia. Leer un atributo, índice, slice, componente
desestructurado o binding de patrón produce una copia lógica profunda e
independiente; la misma ruta usada como place sí muta el almacenamiento
original. Una proyección de referencia exige `Clone`.

## Risks / Trade-offs

- **Alocar sin liberar es correcto para esta fase y una fuga en cuanto un programa sea largo** → Mitigación: el requisito lo declara como deuda con fecha en Fase 4, y no se construye ninguna liberación parcial que después haya que deshacer (D1).

- **El layout de objetos es un contrato con el `.zpkg` de la Fase 8** → Mitigación: lo que se fija ahora es lo que no se puede cambiar después sin rehacer —el orden de los campos heredados y la existencia de la cabecera—; lo que la Fase 4 pida se añade a la cabecera, que existe desde el principio para eso (D2).

- **El alcance es el mayor del roadmap y todo se toca entre sí** → Las clases necesitan genéricos para ser útiles, los genéricos necesitan contratos para restringir, los contratos necesitan clases para ser implementados. No hay un orden en que una parte se termine antes que las otras, así que `tasks.md` avanza por capas del pipeline y no por construcción del lenguaje.

- **Los genéricos especializados multiplican el código generado** → Trade-off aceptado: es la única forma de cumplir la restricción de value classes inline de ADR-003, y el costo en tamaño se mide cuando la Fase 8 se ocupe del tamaño del binario.

- **Retirar el protocolo cerrado de `for ... in` cambia código que ya funciona** → Los rangos pasan a implementar `Iterable<T>`, así que los programas existentes deben seguir compilando sin cambios. El corpus de la Fase 2 es la prueba, y no se toca.

## Migration Plan

Aditivo sobre un pipeline que funciona de punta a punta. Las construcciones que hoy fallan con el diagnóstico de fase pasan a compilar.

Tres cosas que hoy se rechazan pasan a aceptarse y necesitan que el corpus existente siga verde: `?.`, `+` sobre `String` y `for ... in` sobre tipos propios.

Rollback: revertir el merge. Ninguna fase anterior depende de esta.

## Open Questions

La pregunta sobre los tipos función se resolvió antes de empezar y pasó a ser D9.


- **¿Una value class puede tener métodos virtuales?** Sección 7 dice que no tienen identidad observable y que se almacenan inline. Un método virtual necesita un descriptor de tipo en tiempo de ejecución, y guardarlo en algo que se almacena inline contradice "compacto".

  **Confirmado al cierre (14.3):** no. Un método propio de un record o value class se despacha siempre de forma estática — `IrType::Value` se resuelve por el mismo `method_of` que un objeto, pero como ninguno de los dos admite `extends`, `method.overridden` nunca es verdadero y jamás pasa por `CallVirtual`. `implements` sí se acepta y su conformidad se verifica igual que para una clase, pero alcanzar el método a través del tipo del contrato (la única razón por la que el despacho dinámico importaría para un tipo sin identidad) queda bloqueado con `NOT_LOWERED` en la propia declaración: no hay descriptor donde guardar la tabla del contrato. Encontrado y corregido en el cierre: llamar un método directo de un record crasheaba (nunca se había probado, `Self::method_of`/`lower_method_call` solo reconocían `IrType::Object`); corpus en `value_class_methods.zrk` (llamada directa, baja) y `record_implements_not_lowered.zrk` (a través del contrato, gateado).

- **¿Qué pasa al comparar dos records con `==`?** Sección 7 dice que un record tiene semántica estructural, lo que sugiere igualdad campo a campo derivada. La sección 4 dice que `==` ya es igualdad estructural para todos.

  **Decidido, pendiente de bajar (14.3/14.4):** para un record o value class se deriva automáticamente, sin `_equals`; para una clase con identidad, `==` compara identidad salvo que la clase implemente el contrato de operador — esta segunda mitad ya baja (D6). La primera type-checks (`check_equality` acepta `==`/`!=` sobre un record sin exigir `_equals`) pero su lowering —recorrer campo a campo, incluyendo un record anidado dentro de otro— queda gateado con `NOT_LOWERED` propio (`reject_unstructured_comparison`, roadmap task 11.5): construir el valor inline (11.5) fue una pieza; comparar dos de esos valores estructuralmente es otra que ninguna tarea de esta fase reclamó como propia. Queda como deuda viva sin fase asignada todavía — ver 14.4.
