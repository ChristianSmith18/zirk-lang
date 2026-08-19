# ADR-014 — `Char` comparte la representación opaca de `String`

- **Estado:** aceptada
- **Fecha:** 19 de agosto de 2026
- **Fase:** 3b

## Contexto

`ZIRK_LANGUAGE_SPEC.md` sección 3 define `Char` como "exactamente un grapheme Unicode, incluso cuando está compuesto de múltiples code points y bytes". Un grapheme cluster extendido (una base más marcas combinantes, o una secuencia de emoji unida por ZWJ como `'👨‍👩‍👧‍👦'`) no tiene un tamaño máximo garantizado por el estándar Unicode — puede ocupar arbitrariamente más de 4 bytes UTF-8.

`openspec/changes/fase-3b-scalars-and-text/design.md` dejó esto como pregunta abierta con tres opciones: (a) una vista opaca respaldada por el runtime, como `String`; (b) un valor inline con optimización de buffer pequeño y un escape para el caso raro que excede el buffer; (c) restringir `Char` a un solo code point, lo que contradice la sección 3 tal como está escrita y necesitaría una corrección de spec primero.

## Decisión

`Char` se representa exactamente como `String` ([ADR-005](./ADR-005-representacion-string.md)): un handle opaco hacia el mismo tipo de contenido UTF-8 que el runtime ya sabe construir, comparar y liberar. La IR gana un `IrType::Char` propio — no reutiliza `IrType::String` — para que el chequeador y el verificador puedan seguir distinguiendo estáticamente "esto es exactamente un grapheme" de "esto es texto arbitrario", pero en LLVM ambos bajan al mismo puntero opaco y a los mismos símbolos `extern "C"` (`zirk_str_from_utf8` para construir, `zirk_str_eq` para `==`).

La comprobación de que un literal `'...'` contiene exactamente un grapheme extendido (UAX #29) ocurre en `zirk-sema`, no en el léxico ni en el runtime: el léxico ya documentaba esa frontera (`zirk-lexer::character()`'s propio comentario), y el runtime no necesita volver a segmentar un valor que el compilador ya validó en el único punto donde `Char` se construye desde texto literal.

## Motivo

- **Opción (a) sobre (b):** un buffer inline con escape duplica exactamente la máquina que `String` ya tiene — asignación cuando no cabe inline, liberación, comparación de contenido — por una ganancia de rendimiento no medida, y que además solo aplica al caso común (los graphemes de una base y pocas marcas), sin evitar el caso raro (una secuencia ZWJ larga) que de todos modos necesita la ruta de escape. La complejidad se paga dos veces: una en el runtime nuevo, otra en el codegen que ahora necesita saber cuándo un `Char` está inline y cuándo no. La opción (a) paga la complejidad una sola vez, ya pagada por `String`.
- **Opción (c) descartada** porque exige corregir el spec antes de implementar, y el spec ya es explícito y no ambiguo en la sección 3 — no hay una lectura alternativa razonable que justifique reabrirlo solo por conveniencia de implementación.
- **`IrType::Char` propio, en vez de reutilizar `IrType::String` directamente:** aunque la representación en tiempo de ejecución es idéntica, son tipos observablemente distintos para el programa Zirk — `Char` no tiene los métodos de mutación de `String`, y (a diferencia de `String`, ver la enmienda de ADR-005) `Char` no tiene identidad observable: `is` se rechaza sobre `Char` porque es un valor, no una referencia con semántica de alias compartido, aun cuando su representación de bajo nivel sea un puntero. Colapsar ambos en una sola `IrType` obligaría a codificar esa distinción en otro lado (una bandera, una lista de excepciones) en vez de dejar que el propio tipo la cargue.

## Consecuencias

- Ningún runtime nuevo: `Char` reutiliza `zirk_str_from_utf8`/`zirk_str_eq` tal cual.
- Un `Char` se aloja igual que un `String` (`IrType::needs_allocation` es verdadero para ambos) — el costo de un `Char` no es distinto del de un `String` de un grapheme, lo cual es honesto dado que la representación es la misma.
- Si en una fase posterior se mide que la indirección de un `Char` de un solo code point ASCII es un costo real en un caso concreto, se optimiza ahí con evidencia — el mismo principio de cierre que ADR-005 ya adoptó para `String`.
- La comparación (`<`, `<=`, …) y cualquier operación específica de grapheme (mayúscula/minúscula, categoría Unicode) quedan fuera de esta decisión: son superficie de biblioteca (`ZIRK_STDLIB_SPEC.md`), no de representación.
