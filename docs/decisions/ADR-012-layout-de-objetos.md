# ADR-012 — Layout de objetos

- **Estado:** aceptada
- **Fecha:** 19 de agosto de 2026
- **Fase:** 3

## Contexto

`ZIRK_LANGUAGE_SPEC.md` sección 12 exige que "la identidad básica de tipo siempre existe" en tiempo de ejecución, y la Fase 3 es la primera que tiene algo con identidad: un objeto sobrevive al marco que lo creó, a diferencia de todo lo que las Fases 1 y 2 alocaron (D1 de `fase-3-objects-and-type-system/design.md`).

El layout de un objeto es, además, un contrato con el `.zpkg` de la Fase 8 (`ZIRK_COMPILER_SPEC.md` sección 4): lo que se fija aquí es lo que no se puede cambiar después sin rehacer paquetes ya construidos.

Esta decisión también fija dónde traza la línea entre "tiene identidad" y "no la tiene", porque el spec (sección 7) exige que un record o una value class **no** tengan una observable, y guarden solo lo estrictamente necesario ("compacto").

## Decisión

**Un objeto ordinario es una cabecera más sus campos; un record o value class es sus campos, sin cabecera, pasado por valor.**

```
   objeto (class)        =  [ descriptor de tipo | campo₁ | campo₂ | … ]
   valor (record/value class)  =  [ campo₁ | campo₂ | … ]
```

Dos representaciones en la IR (`IrType::Object(u32)` e `IrType::Value(u32)`), cada una con su propia tabla de layouts en el módulo (`ObjectLayout`/`ValueLayout`) pero compartiendo el mismo espacio de ids que `checked.classes` — `ClassType.kind` decide cuál mirar. En LLVM, `llvm_type_in` devuelve un puntero para `Object` y el struct mismo para `Value`: un valor viaja por valor en slots, parámetros, retorno y como campo de otro objeto o valor, sin convención de llamada especial — LLVM ya soporta agregados por valor.

**Campos heredados van antes que los propios**, en el orden en que la jerarquía los declara. El prefijo del layout de una subclase coincide siempre con el de su superclase, así que acceder a un campo heredado es el mismo desplazamiento mire quien mire — la herencia simple no cuesta nada en el acceso. Un record o value class no admite `extends`, así que esta regla no le aplica.

**El descriptor de un objeto lleva, en orden:**

```
[ tabla de métodos | cantidad de ancestros | ancestro₁ … | cantidad de contratos | (contrato, tabla)… ]
```

- La **tabla de métodos** es la de la clase concreta, con las entradas heredadas en el mismo índice que en la base (ver ADR-013).
- Los **ancestros** son la propia clase más cada base transitiva, en ese orden — lo que un cast comprobado (`as`) recorre para decidir si es válido en tiempo de ejecución.
- Cada **contrato satisfecho** aporta su propia tabla, en el orden de métodos del contrato — necesario porque una clase implementa varios y cada uno querría índices propios (ver ADR-013).

**Un record o value class no tiene descriptor**, ni siquiera vacío: no hay tabla de métodos que guardar (sus métodos se resuelven en tiempo de compilación, nunca por índice — ver ADR-013), no hay `extends` que dé ancestros, y `implements` está bloqueado por `NOT_LOWERED` precisamente porque no hay dónde guardar la tabla de un contrato.

**Un genérico se especializa, no se borra.** `Box<Int32>` y `Box<String>` son dos `ObjectLayout`/`ValueLayout` distintos, uno por cada combinación de argumentos de tipo que el programa usa realmente (roadmap task 11.1) — deduplicados por el propio internamiento del checker. Es la única forma compatible con que una value class genérica siga siendo inline: borrar tipos obligaría a pasar todo por puntero.

**Un enum algebraico con datos asociados** (`IrType::Enum(u32)`) es también inline, con su propia tabla (`EnumLayout`): discriminante más la carga útil de **todas** las variantes concatenada, no superpuesta como un union real — cada variante ocupa su propio tramo de campos. Un enum tradicional, ninguna de cuyas variantes carga datos, sigue siendo un `Int32` liso, sin cambio de representación.

## Motivo

**Cabecera separada de los campos, no un campo más.** Lo que la Fase 4 necesite para la memoria —marcas, contadores, lo que la estrategia elegida pida— se añade a la cabecera, que existe desde el principio como concepto propio para eso. Si el descriptor fuera "el primer campo" en vez de algo aparte, cualquier extensión futura desplazaría los índices de todos los campos reales.

**Ancestros aplanados en la cabecera, no recorrido de la jerarquía en tiempo de ejecución.** Un cast comprobado necesita responder "¿es esta clase, o alguna de sus bases?" en tiempo de ejecución sin tener el árbol de clases completo disponible fuera del compilador. Guardar la lista ya aplanada convierte esa pregunta en una búsqueda lineal sobre un arreglo, sin punteros al padre que seguir.

**Variantes concatenadas, no superpuestas, en un enum.** Un union real a nivel de bytes sería más pequeño, pero reinterpretar los bytes de la variante equivocada es exactamente el tipo de comportamiento indefinido que `ZIRK_RUNTIME_SPEC.md` promete que el lenguaje seguro no tiene. Con el número de variantes típico de esta fase el costo de espacio no aprieta; se revisa si un tipo con muchas variantes de payloads grandes lo vuelve un problema real.

**Un record/value class sin cabecera, ni siquiera vacía.** Pagar el tamaño de una cabecera en algo que el spec exige "compacto" — y que además viaja por valor, así que su tamaño se multiplica en cada copia — sería exactamente lo que la sección 7 prohíbe. La consecuencia de no tenerla es real y aceptada: no hay despacho dinámico posible sobre uno (ver ADR-013), lo que resuelve directamente la pregunta abierta del `design.md` sobre métodos virtuales en una value class.

## Consecuencias

- **`implements` en un record o value class type-checkea pero no baja.** La conformidad se verifica igual que para una clase ordinaria, pero alcanzar el método a través del tipo del contrato queda bloqueado con `NOT_LOWERED`: no hay descriptor donde guardar su tabla. Sus propios métodos, llamados directamente sobre el tipo concreto, sí despachan (siempre de forma estática, nunca por índice).
- **Un genérico especializado multiplica el código y los layouts generados.** Trade-off aceptado desde D5 del `design.md`: es la única forma de cumplir la restricción de inline para las value classes genéricas. El costo en tamaño de binario se mide cuando la Fase 8 se ocupe de eso.
- **El layout de un objeto y de un valor son contratos con el `.zpkg` de la Fase 8.** Lo que se fija aquí —orden de campos heredados, forma de la cabecera, ancestros aplanados, variantes concatenadas— no se puede cambiar después sin invalidar paquetes ya construidos. Lo que la Fase 4 necesite para memoria se añade a la cabecera existente; no debería requerir rediseñar esta forma.
- **Un value class nunca alcanza un cast comprobado ni una identidad `is`.** Ninguno de los dos tiene sentido sin cabecera: no hay descriptor que comparar ni dirección que identifique.
