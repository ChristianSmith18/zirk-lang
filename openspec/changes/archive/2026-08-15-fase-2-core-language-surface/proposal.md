## Why

La Fase 1 cerró un pipeline flaco pero completo: un subset mínimo compila hasta un binario nativo. Ese subset no tiene bucles, no tiene closures, no tiene `match`, no tiene nulabilidad y no tiene forma de dividir un programa en más de un archivo. Ningún programa real de tamaño modesto se puede escribir todavía.

Esta fase completa la **superficie del lenguaje que no depende de objetos**. `ZIRK_ROADMAP.md` la fija como:

> Control de flujo completo, funciones completas, `match` con exhaustividad básica, nulabilidad y módulos dentro de un mismo crate.

Al terminar, un programa con varias funciones, control de flujo real y closures se escribe y ejecuta — todavía sin clases ni concurrencia, que son Fase 3 y Fase 5.

## What Changes

### Control de flujo

- `for`, `for ... in` sobre tipos iterables del subset, `while`, `loop`, `break`, `continue`.
- `if` deja de ser solo sentencia: es expresión cuando ambas ramas son type-compatible, según `ZIRK_LANGUAGE_SPEC.md` sección 5.

### Funciones completas

- Parámetros opcionales (`name?`), valores por defecto, parámetros nombrados y variadic (`...values`).
- Closures/lambdas como valores de función, con captura inmutable segura (`ZIRK_LANGUAGE_SPEC.md` sección 6). La captura mutable compartida queda fuera: requiere el análisis de concurrencia de Fase 5.

### `match`

- `match` como expresión, exhaustivo, sobre un conjunto simple de constructores nombrados que este cambio introduce con el mínimo necesario para tener algo exhaustivo que chequear (ver design.md, D1, sobre por qué esto no son los enums algebraicos completos de Fase 3).
- `match` como sentencia, que controla flujo sin producir valor.
- Patrones sobre literales, variables de binding y el comodín `_`. Destructuring de records, `match with` sobre `Resource<E>` y patrones sobre uniones quedan fuera: dependen de records/uniones/`Resource`, que son Fase 3 y Fase 4.

### Nulabilidad

- `T?` como azúcar de `T | Null`, acceso seguro `?.`, fallback `??`, según `ZIRK_LANGUAGE_SPEC.md` sección 4.

### Módulos dentro de un crate

- `share` publica una declaración, `import` la trae a otro archivo del mismo crate, `use` habilita globals sin traer un nombre calificado.
- Todo dentro de un único crate: sin `init.zrk`, sin dependencias externas, sin paquetes (`ZIRK_ROADMAP.md` Fase 6 y Fase 8).
- El diagnóstico de Fase 1 ante `import` — "los módulos llegan en una fase posterior" — se retira: ahora sí llegan.

### Explícitamente fuera de alcance

Clases, `construct`, herencia, interfaces, traits, genéricos, records, value classes, enums algebraicos con datos asociados, uniones, `Result`, manejo de errores, memoria real (sigue como placeholder de ADR-003), concurrencia, decoradores, `init.zrk`, paquetes externos, destructuring, `match with` / `Resource<E>`, y la stdlib más allá de `println` y lo estrictamente necesario para iterar en `for ... in`.

## Capabilities

### New Capabilities

- `zirk-modules`: `share`/`import`/`use` dentro de un crate — resolución de nombres entre archivos, visibilidad, sin `init.zrk`.

### Modified Capabilities

- `zirk-grammar`: gramática de bucles, `if` como expresión, parámetros opcionales/nombrados/variadic, closures, `match`, `T?`/`?.`/`??`, `share`/`import`/`use`.
- `zirk-type-system`: tipos y chequeos de todo lo anterior — exhaustividad de `match`, tipado de closures, tipos opcionales, resolución de nombres entre archivos.
- `zirk-ir-lowering`: lowering de bucles con `break`/`continue`, `if` como expresión, closures, `match`, acceso seguro y coalescencia nula.
- `zirk-native-codegen`: codegen de las construcciones anteriores — saltos de bucle, entornos de closure, dispatch de `match`, chequeo de nulidad.

## Impact

**Crates modificados:** `zirk-ast`, `zirk-parser`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`. Ninguno nuevo: a diferencia de Fase 1, esta fase extiende un pipeline que ya funciona de punta a punta.

**Sin dependencias externas nuevas.**

**Riesgos:**

- **Los enums mínimos de esta fase no pueden convertirse en deuda que Fase 3 tenga que deshacer.** Se acotan a constructores sin datos asociados (ver design.md D1) precisamente para que Fase 3 los extienda en vez de reescribirlos.
- **Las closures son la primera vez que la IR necesita capturar estado fuera del stack frame de la función.** Es una decisión de representación con las mismas apuestas a largo plazo que tuvo la forma de la IR en Fase 1 (ADR-007).
- **Módulos dentro de un crate tientan a resolver ya la visibilidad `public`/`private`/`protected` de Fase 3.** Esta fase solo necesita `share` (visible fuera del archivo) vs. privado por defecto (visible solo en el archivo); los tres niveles completos son Fase 3, sobre clases.
