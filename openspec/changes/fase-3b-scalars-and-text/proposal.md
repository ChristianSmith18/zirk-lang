## Why

La Fase 1 implementó un único ancho entero (`Int32`), un `Boolean` y un `String` opaco — lo mínimo para que el pipeline compilara de punta a punta. Todo lo demás que la sección 3 de `ZIRK_LANGUAGE_SPEC.md` promete sobre escalares —el resto de los anchos enteros, la familia `Float`, `Char` real, la conversión contextual profunda— nunca tuvo una fase asignada. `ZIRK_ROADMAP.md` la fija como Fase 3b:

> The remaining integer widths... The binary floating family... `Char` as exactly one Unicode grapheme... Deep contextual conversion... Bitwise and shift operators... String interpolation.

Llegan juntas porque dependen entre sí: la conversión contextual no significa nada sin `Float`, los literales `Float` no significan nada sin la familia completa, y la interpolación necesita el contrato `to_string()` que esta fase también cierra. Dividirlas obligaría a implementar cada una dos veces.

### Una dependencia que el roadmap asume resuelta y no lo está

El roadmap describe la interpolación como algo que "needs the `to_string()` contract Phase 3 defines". Auditado contra el código: **Fase 3 nunca definió ese contrato.** `require_printable` en `zirk-sema/src/checker.rs` sigue aceptando únicamente `Int32`, `Boolean` y `String` en `stdout.println`, con un mensaje de ayuda que dice literalmente "`to_string()` becomes a trait in Phase 3" — una expectativa que quedó sin cerrar. Ni un enum, ni una clase, ni un record se pueden imprimir hoy sin convertir sus campos a mano.

Esta fase absorbe esa deuda: el contrato `to_string()` (o el nombre que `ZIRK_STDLIB_SPEC.md` sección 3 fije) se implementa aquí, como precondición real de la interpolación de strings, no como algo ya disponible que se asume.

## What Changes

### Anchos enteros completos

- Con signo: `Int8`, `Int16`, `Int64`, `Int128` (`Int32` ya existe). `Int`/`Integer` siguen siendo alias de `Int32`.
- Sin signo: la familia `UInt8`…`UInt128` completa, nueva en su totalidad.
- Aritmética comprobada en cada ancho: overflow es un error controlado, igual que ya lo es para `Int32` (Fase 1); wrapping/saturating/checked explícitos quedan fuera de esta fase salvo que el spec los pida en otra sección.
- Ensanchamiento seguro implícito donde no sea ambiguo; conversión con o sin signo, y toda conversión con pérdida, siempre explícita (sección 3).

### Familia `Float`

- `Float16`, `Float32`, `Float64`, `Float128`, con `Float` como alias de `Float64`.
- Infinitos positivo y negativo explícitos; **ningún** `NaN` válido — una operación que produciría uno es un error controlado, no un valor especial que se propaga.
- Literales fraccionarios (`1.5`, notación científica `1e2`), con default `Float64` cuando no hay contexto que diga otra cosa.
- Aritmética mixta entero/`Float` produce `Float` (sección 3, ya documentado, sin implementar porque `Float` no existe todavía).

### `Char`

- Exactamente un grapheme Unicode, incluso cuando abarca varios code points — no un `UInt32` con nombre distinto.
- Lo que permite, por fin, iterar un `String` grapheme a grapheme (`for c in s`, deuda anotada desde la Fase 2 y otra vez en el `for ... in` de Fase 3).

### Conversión contextual profunda

- Un constructor explícito (`Float(3 / 4)`, `String("value=" + 42)`) establece un dominio para todo el árbol de operadores compatible directamente dentro de él, convirtiendo los operandos antes de operar — no después. El contexto no muta los operandos ni cruza al cuerpo de una función llamada (sección 3).

### Operadores bitwise y de shift

- `&`, `|`, `^`, `~`, `<<`, `>>` sobre los tipos enteros, con signo y sin signo, en los niveles de precedencia que el spec fija.

### El contrato `to_string()`, y con él la interpolación

- El contrato reservado que `ZIRK_STDLIB_SPEC.md` sección 3 exige, implementable por cualquier tipo del usuario — clase, record, enum — con la misma disciplina de contratos reservados que la Fase 3 fijó para los operadores (`ZIRK_LANGUAGE_SPEC.md` sección 4: sobrecarga solo por contrato del lenguaje).
- `stdout.println`/`stdout.print` pasan a rutear por él en vez de la lista cerrada de tres tipos que tienen hoy.
- Interpolación de strings (la sintaxis exacta la fija `ZIRK_LANGUAGE_SPEC.md`; auditar contra la sección 1/7 al diseñar), que la usa para convertir cada expresión interpolada.

### Explícitamente fuera de alcance

- **`Decimal`** — la sección 3 lo marca como "may be a later stdlib type", no de esta fase.
- **Los tipos temporales** (`Date`, `Time`, `Duration`, etc., sección 8.1) — familia propia, sin fase asignada todavía en el roadmap.
- **Aritmética wrapping/saturating explícita** más allá de lo que la conversión contextual ya cubre, salvo que auditar el spec durante el diseño muestre que esta fase la exige.
- **Cualquier cosa de colecciones** (`List<T>` y compañía, Fase 7) — un `Char` iterando un `String` no depende de ellas.

## Capabilities

### New Capabilities

- `zirk-scalars`: los anchos enteros completos, la familia `Float`, `Char`, y las reglas de conversión (implícita segura, explícita con pérdida, contextual profunda) entre todos ellos.

### Modified Capabilities

- `zirk-grammar`: literales `Float`/anchos con sufijo si el spec los pide, operadores bitwise/shift, sintaxis de interpolación.
- `zirk-type-system`: la familia completa de tipos numéricos y sus reglas de conversión; el contrato `to_string()`.
- `zirk-ir-lowering`: representación de cada ancho entero y de `Float`, lowering de la conversión contextual, de bitwise/shift y de la interpolación.
- `zirk-native-codegen`: codegen de aritmética por ancho, de `Float`, y del despacho a `to_string()` desde `println`/interpolación. La primitiva del runtime que escribe un `String` a la salida estándar (`zirk-runtime-io`) no cambia — lo que cambia es qué se convierte a `String` antes de llegar a ella.

## Impact

**Crates modificados:** `zirk-lexer` (literales), `zirk-parser`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-runtime`. Ninguno nuevo.

**Sin dependencias externas nuevas** — LLVM ya modela cada ancho entero y cada ancho `Float` de forma nativa; nada de esto exige software flotante propio.

**Riesgos:**

- **La deuda de `to_string()` es más grande de lo que el roadmap asume.** No es "ya existe, úsalo": es una pieza nueva completa (contrato reservado + despacho + integración con `println` + integración con interpolación). Se audita su alcance real al escribir `design.md`, antes de comprometerse a una forma.
- **`NaN` inválido es una restricción fuerte que toca cada operación de `Float`.** Cada operación que en IEEE 754 produciría `NaN` (`0.0 / 0.0`, `sqrt(-1)`, etc.) necesita convertirse en un error controlado en vez de dejar que el bit pattern de `NaN` se propague — eso es trabajo en cada operación, no una comprobación al final.
- **Los anchos enteros multiplican la superficie de la aritmética comprobada de la Fase 1.** Lo que hoy es una sola ruta de overflow para `Int32` se vuelve una por ancho con o sin signo — diez combinaciones en vez de una. Se audita si el checker/IR ya generalizan esto o si asumían `Int32` en algún punto no documentado.
- **La conversión contextual profunda interactúa con los contratos de operador de la Fase 3.** `Float(3 / 4)` reescribe el árbol de `/` antes de bajarlo; con `_add`/`_divide` de por medio para un tipo de usuario, hay que decidir si el contexto se propaga a través de un operador sobrecargado o se detiene ahí — el spec no lo dice explícitamente, así que es pregunta abierta de diseño.
