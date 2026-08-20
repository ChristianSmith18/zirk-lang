## Why

`fase-4b-excepciones` dejó `Resource<E>`/`match with` explícitamente fuera de
alcance porque dependían de que `throw`/`try`/`catch`/`finally` ya ejecutaran
de verdad, no solo chequearan — ahora lo hacen. `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md`
sección 4 describe el mecanismo completo: `Resource<E from Error>` como
contrato genérico con `close()`/`is_closed()`, adquisición agrupada
izquierda-a-derecha con cierre derecha-a-izquierda, fallas de cierre
preservadas en `ResourceFailure<BodyError,CloseError>` y en `suppressed`
durante propagación, transferencia explícita vía `TransferableResource`, y un
análisis de escape/uso-tras-transferencia que impide que un recurso
sobreviva a su scope manejado.

Ese mecanismo completo es, otra vez, demasiado grande para un solo cambio: el
análisis de escape/transferencia es un borrow-checker simplificado propio,
`ResourceFailure` necesita combinar dos resultados independientes en un
tercer tipo, y `suppressed` necesita `List<T>` (Fase 7, no existe). Este
cambio construye el núcleo verificable y ejecutable — el contrato
`Resource<E>`, la gramática de `match ... with`, y el cierre automático real
en cada camino de salida de la rama que adquiere el recurso (finalización
normal, `return`, excepción propagada, `break`, `continue`) — reutilizando
wholesale el mecanismo de `finally` de `fase-4b-excepciones` (D3 de su propio
`design.md`): un recurso adquirido es, para efectos de cuándo se cierra,
exactamente un `try { <rama> } finally { binding.close(); }` sin `catch`
propio.

## What Changes

- `interface Resource<E from Error> { fn close(): Result<Void,E>; fn is_closed(): Boolean; }`,
  registrada igual que `Iterable<T>`/`Iterator<T>` — inyectada directamente,
  no parseada de una declaración de usuario — pero con la restricción `from
  Error` expresada con la misma maquinaria de `constraints` que un `interface`
  de usuario ya tiene.
- Una clase de aplicación adopta el contrato con `implements Resource<MiError>`,
  la misma sintaxis que cualquier otro `implements`.
- `match scrutinee with binding { ... }`: el `scrutinee` debe ser
  `Result<R,Err>`; `binding` debe ser el nombre que una (y solo una) rama
  destructura en su propio patrón (típicamente `Result.Ok(binding)`), y `R`
  debe implementar `Resource<E>` para algún `E`.
- Esa rama cierra el recurso — llama a `close()`, descartando su `Result` —
  en cada camino de salida: finalización normal, `return`, `break`,
  `continue`, o una excepción que se propaga a través de ella.
- `parse_match` se reestructuró: el guard de `match with` que emitía
  `NOT_IMPLEMENTED` revisaba `with` *antes* de parsear el scrutinee, en la
  posición equivocada para la gramática real (`match <scrutinee> with
  <binding> { ... }`, `with` viene después). Ahora el scrutinee se parsea
  primero y `with binding` es opcional después de él.
- Un gap preexistente y no relacionado, descubierto al ejercitar `close():
  Result<Void,E>` por primera vez en el compilador: `zirk-codegen-llvm`
  nunca había construido el tipo LLVM de un enum con un campo `Void`
  (`Result<Void,X>` no tenía ningún uso anterior en el lenguaje). Corregido
  de forma acotada — un campo `Void` ocupa un slot de struct de tamaño cero
  en vez de hacer panic — sin tocar `value_struct`/`object_struct`/closures,
  que ningún programa ejercita con un campo `Void` todavía.

### Explícitamente fuera de alcance

- **Adquisición agrupada** (`match a with x, b with y { ... }` o equivalente)
  y su cierre derecha-a-izquierda cuando una adquisición posterior falla —
  este cambio cubre una sola adquisición por `match ... with`.
- **`ResourceFailure<BodyError,CloseError>`** — una falla de cierre no se
  combina con el resultado del cuerpo; `close()`'s propio `Result` se
  descarda tras llamarlo. Documentado como narrowing explícito, igual que
  `fase-4b-excepciones` narrowed `suppressed`.
- **`suppressed` poblado desde una falla de cierre durante propagación de
  excepción** — depende de `List<T>` (Fase 7) y de `ResourceFailure`, ambos
  fuera de alcance aquí también.
- **`TransferableResource`, `transfer()`, y el análisis de
  escape/uso-tras-transferencia** — un análisis de flujo de datos propio,
  fuera de alcance de un primer corte; un recurso puede, hoy, escapar su
  scope sin que el compilador lo detecte (queda como responsabilidad del
  programador, sin verificación).
- **Recursos dependientes que no sobreviven a su padre**, **`take` sobre un
  recurso no clonable en un contenedor** — necesitan colecciones (Fase 7).
- **Cancelación** (`docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`) — no existe
  concurrencia estructurada todavía (Fase 5).

## Impact

- Specs afectadas: `zirk-resources` (implementa la primera de sus cinco
  requerimientos, parcialmente — sin adquisición agrupada), `zirk-grammar`.
- Sin cambios de ruptura: nada de lo que compila hoy deja de hacerlo.
