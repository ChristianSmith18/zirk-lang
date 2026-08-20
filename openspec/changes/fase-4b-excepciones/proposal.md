## Why

`fase-4a-errores` cubrió la mitad *esperada* de fallas (`Result<T,E>`) de `ZIRK_LANGUAGE_SPEC.md` sección 9. Esta fase cubre la mitad *extraordinaria*: `throw`/`try`/`catch`/`finally`, la jerarquía `Error`/`Throwable`/`RuntimeError`, y `throws` en firmas de función, verificado por el chequeador (capturar o declarar).

El mecanismo completo de excepciones (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` sección 3) es grande: unwinding real a través de marcos de pila arbitrarios, trazas de pila estructuradas perezosas, `suppressed` poblado desde una falla de limpieza en `finally`, conversión de las fallas de runtime nativas existentes (división por cero, overflow, cast inválido, índice fuera de rango) a `RuntimeError` catcheable, y `Resource<E>`/`match with` (que depende de que todo lo anterior exista). Intentarlo entero en un solo cambio arriesgaría las mismas dos cosas que la separación de `fase-4a` ya evitó: tamaño y acoplamiento.

Este cambio construye el núcleo verificable *y ejecutable* — sintaxis completa, la jerarquía de clases, el análisis de efectos "capturar o declarar", y un mecanismo de propagación real — sin construir el desenrollado de pila basado en tablas (landing pads / `personality function`) que un compilador de producción usaría: el análisis "capturar o declarar" del chequeador ya prueba, en tiempo de compilación, que toda excepción no capturada localmente está declarada en cada función de la cadena de llamada hasta donde se captura. Eso hace posible un mecanismo de propagación más simple y total del mismo modo — cada función cuyo cuerpo puede lanzar (transitivamente) devuelve, además de su valor ordinario, una salida implícita "lancé/no lancé"; cada sitio de llamada a una función así comprueba esa salida y, si hubo lanzamiento, salta al `catch` local que lo cubre o repropaga inmediatamente desde la función actual (decisión D1 en `design.md`). No es desenrollado real de la pila nativa — es un `Result` invisible que el programador nunca escribe — pero es correcto para cada programa que el chequeador aceptó, y es la pieza que faltaría para declarar esto "no ejecuta" en vez de "ejecuta".

## What Changes

- Palabras clave `throw`/`throws` (nuevas); `try`/`catch`/`finally` se gradúan de "reservadas para Fase 4" a implementadas.
- `throw expr;` (lanza) y `throw;` (relanza, solo válido dentro de un `catch`).
- `try { } catch Tipo(nombre) { } ... finally { }` — al menos un `catch` o un `finally`. Cada `catch` prueba por tipo exacto o ancestro (sin patrones de variante todavía — ver "fuera de alcance").
- `throws Tipo (| Tipo)*` en la firma de una función o método (no en un tipo `Fn(...)`: la sintaxis de tipos función no existe todavía — decisión D9, `fase-4a-errores`).
- La jerarquía compilador-conocida `abstract class Error { ... }`, `abstract class Throwable implements Error { ... }`, `abstract class RuntimeError implements Throwable {}` (`docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` sección 3), registradas igual que `Result` — inyectadas directamente, no parseadas de una declaración de usuario.
- Análisis de efectos "capturar o declarar": una excepción lanzada o propagada desde una llamada a una función `throws` debe estar cubierta por un `catch` que la alcance, o la función contenedora debe declarar `throws` de un tipo que la cubra. Un catch más general antes que uno más específico es un error (inalcanzable).
- Ejecución real de `throw`/`try`/`catch`/`finally` y de `throws` propagado entre funciones, sobre el mecanismo de propagación de D1 — un compilado real corre el programa, con salida y código de salida correctos.
- `stack_trace()` de `Throwable` retorna un `StackTrace` real pero vacío (sin marcos) — el tipo existe y es llamable, la captura de contexto real queda fuera de alcance.

### Explícitamente fuera de alcance

- **Trazas de pila estructuradas, `suppressed` poblado desde una falla de limpieza en `finally`.**
- **Conversión de las fallas de runtime nativas existentes** (división por cero, overflow, cast inválido, índice fuera de rango) **a `RuntimeError` catcheable** — siguen abortando el proceso como hoy; es un cambio propio, separado, que toca cada sitio de `zirk-runtime` que hoy llama `fatal()`.
- **Patrones de variante en `catch`** (`catch NetworkError.Timeout(duration)`) — necesita que una excepción de usuario declare variantes internas, un mecanismo que no existe; solo `catch Tipo(nombre)` por tipo de clase.
- **`Fn(...) => T throws X`** — la sintaxis de tipos función no existe (D9, `fase-4a-errores`).
- **`Resource<E>`/`match with`** — depende de que las excepciones ya ejecuten de verdad, no solo chequeen.
- **Bajado a IR y codegen** (el desenrollado real) — gateado tras `NOT_LOWERED`, cambio separado después de este.

## Impact

- Specs afectadas: `zirk-errors` (implementa los otros tres de los cuatro requisitos que ya documenta, salvo trazas/suppressed), `zirk-type-system`, `zirk-grammar`, `zirk-lexical-syntax`.
- Sin cambios de ruptura: nada de lo que compila hoy deja de hacerlo.
