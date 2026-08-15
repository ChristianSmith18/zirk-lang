## Why

La Fase 2 completó la superficie del lenguaje que **no depende de objetos**: control de flujo, funciones con closures, `match`, nulabilidad y módulos. Con eso ya se escriben programas de tamaño modesto, pero no se puede modelar un dominio: no hay forma de decir qué es un `User`, ni de que dos tipos compartan un contrato.

`ZIRK_ROADMAP.md` fija esta fase como:

> `class`, `construct`, visibilidad, herencia simple, interfaces, traits. Genéricos con `from`. Records, value classes, enums algebraicos, uniones. Casts.

Es la fase donde el lenguaje deja de ser un procedimiento y pasa a tener un sistema de tipos. Al terminar, el subset orientado a objetos del spec funciona, incluidos genéricos básicos.

### Además, retira deudas de la Fase 2 que esperaban justamente esto

Tres de los pendientes de la Fase 2 se difirieron **nombrando esta fase como su condición**, y no cerrarlos aquí los convertiría en deuda sin fecha:

- **`?.`** (D8 de Fase 2) esperaba a que existiera un tipo con miembros.
- **`+` sobre `String`** esperaba a los contratos de operador, que `ZIRK_LANGUAGE_SPEC.md` sección 4 exige como única vía de sobrecarga.
- **`for ... in` sobre tipos propios** (D3 de Fase 2) esperaba a los traits.

## What Changes

### Clases

- `class` con campos y métodos, `construct` como constructor, `this` como instancia actual.
- Visibilidad `public` / `private` / `protected`; un campo sin modificadores equivale a `public mut`.
- Múltiples declaraciones `construct`, resueltas por tipos, aridad y argumentos nombrados, incluso reordenados.
- Herencia simple: una clase extiende a lo sumo una clase. Las clases son heredables por defecto; hay `abstract` para clases y métodos, y no hay `final`.

### Contratos

- **Interfaces**: firmas sin implementación, combinables sin límite.
- **Traits**: pueden incluir implementación reutilizable.
- **Contratos de operador**: la única forma de sobrecargar `+`, `==` y compañía, mediante métodos reservados como `_add` y `_subtract`, sin alterar precedencia ni aridad (`ZIRK_LANGUAGE_SPEC.md` sección 4). Los tipos nativos no se pueden reabrir.
- **`Iterable<T>` / `Iterator<T>`**: lo que hace que `for ... in` funcione sobre tipos propios y retira el protocolo cerrado de la Fase 2.

### Tipos de datos

- **Enums algebraicos**: extienden el `enum` tradicional —cuyo valor observable por defecto es el nombre exacto del caso y que admite mappings `->`— con valores asociados; `match` gana destructuring anidado.
- **Records**: inmutables, con semántica estructural.
- **Value classes**: sin identidad observable, almacenables inline.
- **Uniones** `A | B`, y alias con `type`.

### Genéricos

- Parámetros de tipo `<T>` en funciones, clases y tipos de datos.
- Restricciones con `from`, verificadas en el sitio de uso.

### Casts

- `as` postfijo y `<T>` prefijo, con la forma comprobable fallando de manera controlada.

### Explícitamente fuera de alcance

- **Casts que reinterpretan memoria.** `ZIRK_LANGUAGE_SPEC.md` sección 11 los exige dentro de `unsafe {}`, y `unsafe` pertenece a la sección 13 junto al resto del nivel bajo. Reinterpretar memoria antes de que exista un modelo de memoria (Fase 4) no significa nada.
- **La biblioteca de colecciones.** `List<T>`, `Map<K,V>` y `Set<T>` se vuelven *expresables* al llegar los genéricos, pero implementarlas es Fase 7. Con ellas sigue esperando el parámetro variadic de la Fase 2.
- **Generadores (`fn gen`), `|>` y el estilo funcional** de la sección 8 más allá de `Iterable`/`Iterator`.
- **Decoradores y reflexión** (sección 12, Fase 10), errores y `Result` (Fase 4), concurrencia (Fase 5).
- **La estrategia de memoria.** Esta fase necesita alocar y lo hará por una operación abstracta, sin elegir estrategia: eso es Fase 4 y lo fija ADR-003.

## Capabilities

### New Capabilities

- `zirk-classes`: declaración de clases, constructores, campos, métodos, visibilidad y herencia simple.
- `zirk-contracts`: interfaces, traits, contratos de operador e iteración, y su verificación.
- `zirk-generics`: parámetros de tipo, restricciones `from` y su comprobación.
- `zirk-data-types`: records, value classes, enums algebraicos, uniones y alias.
- `zirk-object-memory`: la frontera por la que un objeto se aloca, sin nombrar estrategia (ADR-003).

### Modified Capabilities

- `zirk-grammar`: sintaxis de todo lo anterior, y retirada de los diagnósticos de fase que la cubrían.
- `zirk-type-system`: tipos nominales, subtipado por herencia e interfaces, resolución de miembros, `?.`, y unificación con genéricos.
- `zirk-ir-lowering`: representación de objetos, acceso a campos, llamadas a métodos y despacho.
- `zirk-native-codegen`: layout de objetos, tablas de métodos y la llamada indirecta que el despacho dinámico necesita.
- `zirk-runtime-io`: `String` deja de ser un intrínseco opaco y pasa a implementar los contratos que esta fase define.

## Impact

**Crates modificados:** todos los del pipeline. `zirk-runtime` gana la alocación de objetos.

**Sin dependencias externas nuevas.**

**Riesgos:**

- **Esta fase tiene que alocar, y la estrategia de memoria es de la Fase 4.** Es la tensión central: un objeto con identidad sobrevive al marco que lo creó, así que el truco de la Fase 2 —capturas dentro del valor, en la pila (D10)— no sirve. Ver design.
- **El layout de objetos y la tabla de métodos son un contrato con la Fase 8**, igual que lo fue la forma de la IR: viajan dentro del `.zpkg`. Decidirlos mal cuesta caro después.
- **Los genéricos tientan a crecer hacia un sistema completo.** El spec pide `<T>` con restricciones `from` y especialización "donde corresponda"; no pide varianza, tipos asociados ni orden superior. Lo que no está pedido se deja fuera y se anota.
- **El alcance es el mayor del roadmap hasta ahora.** Nueve construcciones nuevas del lenguaje contra las cinco de la Fase 2, y todas se tocan entre sí.
