# ADR-013 — Forma del despacho

- **Estado:** aceptada
- **Fecha:** 19 de agosto de 2026
- **Fase:** 3

## Contexto

La Fase 3 trae herencia simple, interfaces y traits, y con ellos la primera vez que una llamada a método no tiene un único cuerpo posible en tiempo de compilación: `describe_any(u: User)` puede recibir un `User` o un `Manager` que lo redefine, y `describe(s: Speaker)` no sabe qué clase concreta implementa `Speaker` hasta que el programa corre.

`ZIRK_COMPILER_SPEC.md` sección 4 pide que la IR conserve información suficiente para **devirtualizar** — lo que presupone que hay algo que devirtualizar, es decir, que no toda llamada paga el mismo costo de indirección.

Esta decisión depende directamente de ADR-012: la forma del descriptor (tabla de métodos, ancestros, tablas de contrato) es donde el despacho vive.

## Decisión

**Una llamada a método es directa por defecto; solo pasa por una tabla cuando el chequeador demuestra que hace falta, y de una tabla distinta según por dónde se llama.**

Tres formas de una llamada a método, elegidas en tiempo de compilación:

1. **Directa (`InstKind::Call`).** El destino se conoce en tiempo de compilación: ninguna subclase redefine el método. Es la mayoría de las llamadas. El receptor viaja como primer argumento explícito, igual que cualquier otro.
2. **Por la tabla propia del objeto (`InstKind::CallVirtual`).** El método es redefinible — alguna subclase lo hace — y se llama a través de un tipo que no garantiza estáticamente cuál cuerpo es. El índice en la tabla es estable entre una clase y su subclase, porque la tabla de la subclase empieza con las entradas de la base en el mismo orden (ADR-012).
3. **Por la tabla de un contrato (`InstKind::CallContract`).** El receptor se tipa a través de una interfaz o trait, no de su propia clase. Cada contrato que una clase satisface tiene su propia tabla en el descriptor, en el orden de métodos del contrato — necesario porque una clase implementa varios contratos y cada uno querría índices propios si compartieran una sola tabla.

**Qué método es "redefinible" es información que el chequeador ya tiene**, porque conoce la jerarquía completa del crate antes de que la IR se genere: un método al que ninguna subclase declarada le pone `override fn` nunca necesita indirección, sin importar a través de qué tipo se lo llame.

**Un record o value class no tiene ninguna de las tres formas indirectas.** No tiene descriptor (ADR-012), así que no hay tabla propia ni de contrato que ofrecer. Sus métodos siempre resuelven en forma directa — nunca `overridden`, porque ni `record` ni `value class` admiten `extends` — y llamar uno a través del tipo de un contrato que implementa queda bloqueado con `NOT_LOWERED` en su propia declaración.

**Un cast comprobado (`as`) no es despacho de método, pero comparte la cabecera.** Verifica en tiempo de ejecución que el objeto es de la clase destino o una de sus subclases, recorriendo la lista de ancestros aplanada del descriptor (ADR-012) — una búsqueda lineal, sin devolver ningún método.

## Motivo

**Directa por defecto, no indirecta por defecto.** Pagar una indirección por cada llamada sería pagar por una generalidad que el programa mayormente no usa: la mayoría de los métodos de un programa típico nunca se redefinen. La alternativa —despacho por búsqueda de nombre en tiempo de ejecución, al estilo de un lenguaje dinámico— es más simple de implementar y estrictamente más lenta en cada llamada, sin aportar nada que el chequeador no pueda ya decidir en tiempo de compilación.

**Tabla propia y tabla de contrato como cosas separadas, no una tabla combinada.** Una clase satisface varios contratos a la vez, cada uno con su propio conjunto de métodos y su propio orden de declaración. Combinarlos en una sola tabla obligaría a reservar el índice más ancho de todos los contratos que la clase pudiera llegar a implementar, o a recalcular índices cada vez que se agrega uno — ninguna de las dos escala. Una tabla por contrato satisfecho, indexada por su propio orden, es lo que hace que **cualquier** clase que implemente `Speaker` responda `.speak()` en el mismo índice, sin importar qué más implemente.

**Sin despacho para un record o value class, no un despacho degradado.** La alternativa —darle una cabecera solo para poder ofrecer una tabla— contradice directamente ADR-012 y la sección 7 del spec ("compacto"). La consecuencia se acepta: hoy `implements` en un record type-checkea pero no baja: alcanzar el método a través del contrato no tiene dónde apoyarse. La llamada directa sobre el tipo concreto —la mayoría de los usos reales de un record— no necesita nada de esto.

## Consecuencias

- **Agregar una subclase que redefine un método ya compilado exige recompilar quien lo llama sin saberlo.** Si `describe_any(u: User)` se compiló asumiendo `describe` directo antes de que `Manager` existiera, hace falta recompilación, no un enlace incremental — consistente con que Zirk no tiene compilación separada por archivo objeto reutilizable todavía.
- **Los genéricos especializados (ADR-012) heredan esta misma regla sin cambios**: cada instanciación de una clase genérica tiene su propia tabla de métodos y de contratos, construida exactamente igual que la de una clase no genérica.
- **`for ... in` sobre un `Iterable<T>` propio usa exactamente este mecanismo**, sin uno nuevo: `iterator()` y `next()` se llaman por la tabla del contrato nativo `Iterable`/`Iterator`, lo mismo que cualquier otro método de interfaz.
- **El límite documentado aquí —sin despacho para record/value class— es el que responde la pregunta abierta del `design.md` de esta fase sobre métodos virtuales**: la respuesta es no, y esta es la razón estructural, no una limitación temporal de la implementación.
