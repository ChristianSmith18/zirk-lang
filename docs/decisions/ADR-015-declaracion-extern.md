# ADR-015 — Sintaxis y alcance de `extern`: cómo se declara una función nativa

- **Estado:** aceptada
- **Fecha:** 24 de agosto de 2026
- **Fase:** 4e

## Contexto

`MEMORY_AND_UNSAFE_SEMANTICS.md` §9 incluye, en el conjunto cerrado de operaciones unsafe, "calling an `unsafe fn` or unsafe native declaration". `ZIRK_LANGUAGE_SPEC.md` línea 581-582 dice, como única frase sobre interoperabilidad: "Native interoperability uses the C ABI as its stable boundary; C++ and Rust expose `extern "C"` wrappers." Ninguna de las specs normativas dice cómo se escribe, del lado de Zirk, la declaración de una función nativa — no hay palabra clave `extern`, ni siquiera reservada en el léxico. Es un hueco real, no una omisión de implementación: nadie decidió la sintaxis todavía.

Este ADR cierra esa decisión para lo mínimo que hace falta que `unsafe {}`/`Pointer<T>` sean útiles de verdad, sin construir el sistema completo de enlace de bibliotecas nativas (que pertenece a la Fase 6, junto con el manifiesto de permisos `requires`/`during: build`).

## Decisión

### Sintaxis: declaración de un solo ítem, sin bloque agrupador

```zirk
extern "C" fn strlen(s: Pointer<Byte>): UInt64;
extern "C" fn memcpy(dst: Pointer<Byte>, src: Pointer<Byte>, n: UInt64): Pointer<Byte>;
```

Un `extern "C" fn` es un ítem de nivel superior, sin cuerpo, terminado en `;` — la misma forma que un `fn` declarado, salvo que no tiene bloque. El literal de string en la posición de la convención de llamada (`"C"`) es la única soportada hoy; se reserva la posición para no romper la sintaxis si en el futuro hiciera falta otra (`"system"`, etc., como hace Rust), pero el compilador rechaza cualquier valor que no sea `"C"`.

**Alternativa considerada — bloque agrupador `extern "C" { fn a(...); fn b(...); }`** (estilo Rust). Rechazada por ahora: agrega una forma de agrupamiento nueva al lenguaje (Zirk no tiene bloques de ítems en ningún otro lugar de la gramática) para ahorrar repetir `extern "C"` unas pocas veces por archivo. Si el volumen de declaraciones nativas crece, se puede añadir después sin romper la forma de un solo ítem.

### Llamar una declaración `extern` exige `unsafe` y `commit`

Toda llamada a una función `extern` — sin excepción, sin importar si el programador sabe que es pura — exige estar dentro de un bloque `unsafe {}` (es la operación 2 del conjunto cerrado) y, además, dentro de un `commit {}` anidado (`MEMORY_AND_UNSAFE_SEMANTICS.md` §12 ya lista "unknown-effect native library calls" entre lo que exige `commit`). El compilador no intenta distinguir una llamada nativa "pura" de una con efectos: **toda** llamada `extern` se trata como potencialmente irreversible. Es la lectura más simple y más segura del texto normativo, y evita inventar una taxonomía de efectos nativos que ninguna spec pide todavía.

### Superficie de tipos: solo lo que tiene layout ABI-C estable

Los tipos permitidos en la firma de un `extern "C" fn` (parámetros y retorno) son: `Void` (solo retorno), `Boolean`, `Int8`/`Int16`/`Int32`/`Int64`, `UInt8`/`UInt16`/`UInt32`/`UInt64`, `Float32`/`Float64`, y `Pointer<T>` donde `T` es, recursivamente, uno de estos mismos tipos u otro `Pointer<U>`. **`String`, clases, records, enums, y cualquier tipo gestionado quedan fuera** — ninguno tiene layout binario estable del lado C sin una capa de marshaling que este ADR no construye. Un programa que necesita pasarle texto a una función nativa lo hace explícitamente, vía `Pointer<Byte>` y las operaciones de `NativeSlice<Byte>` que `unsafe` ya expone — la conversión es responsabilidad del programa, no un `String` implícitamente compatible con C.

### Resolución de símbolos: el enlazador del sistema, sin manifiesto nuevo

Una declaración `extern "C" fn` no trae consigo ninguna forma de decirle al compilador "enlaza también esta biblioteca". Se apoya enteramente en lo que el enlazador ya resuelve por default en la plataforma — típicamente libc y lo que el propio `zirk-runtime` ya enlaza transitivamente. Declarar una función que no resuelve en el enlace produce el mismo fallo que hoy ya diagnostica `zirk-native-codegen`'s "Fallo del enlace" (símbolo no encontrado, salida del linker incluida). **Enlazar una biblioteca nativa adicional queda explícitamente fuera de este ADR** — es la extensión natural del `requires`/`during: build` que `zirk-permissions` ya reserva para dependencias externas, pero construir esa integración es trabajo de Fase 6, no de esta pieza de Fase 4e.

### Sin integración con el sistema de permisos todavía

`MEMORY_AND_UNSAFE_SEMANTICS.md` §12 dice "Permissions are still checked before the effect" para lo que entra a `commit {}`. Los permisos no existen como concepto en tiempo de ejecución todavía (confirmado ya en `fase-4d-callables`: "permissions do not exist as a runtime concept yet"), así que no hay nada que comprobar hoy. Este ADR no inventa un scope de permiso nuevo para "llamar función nativa X" — cuando la Fase 6 construya el sistema de permisos real, `extern`/`commit` es donde se conectará, pero eso es trabajo de esa fase, no de esta.

## Consecuencias

- `unsafe {}`/`Pointer<T>` dejan de depender de una decisión de sintaxis sin tomar: se puede escribir, compilar y enlazar una llamada a una función de libc real (`strlen`, `memcpy`, `malloc`/`free` crudos, etc.) hoy, sin esperar a la Fase 6.
- Un programa Zirk que declara `extern "C" fn algo_que_no_existe(): Void;` y lo llama falla en el enlace, con el mismo mecanismo de diagnóstico que cualquier otro fallo de enlace hoy — comportamiento honesto, no un `unsafe` que finge funcionar.
- La superficie de tipos deliberadamente estrecha (sin `String`, sin objetos) es una limitación real y conocida: interoperar con una API de C que espera `char*`/structs necesita, hoy, escribirse a mano con `Pointer<Byte>` y aritmética explícita. Es el punto de partida correcto para no comprometerse a un marshaling automático que ninguna spec ha diseñado.
- Cuando la Fase 6 construya el manifiesto de bibliotecas nativas, este ADR es el punto de extensión: se añade *dónde* enlazar, no *cómo* se declara una función nativa — esa forma no debería necesitar cambiar.
