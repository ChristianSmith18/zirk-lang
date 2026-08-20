## Context

`fase-4b-excepciones` construyó el mecanismo completo de propagación (D1: slot `thread_local` pendiente; D2: `try_stack` con despacho directo a un `catch` activo; D3: `finally` duplicado en cada salida) para `throw`/`try`/`catch` explícitos. Su propio `design.md`, sección D5, investigó y difirió deliberadamente extender ese mecanismo a las fallas de seguridad nativas, documentando tres brechas concretas (no solo "falta tiempo"):

1. Los chequeos viven en `zirk-codegen-llvm/src/emit.rs`, una capa por debajo de donde `zirk-ir` todavía tiene noción de `try`/`catch` (esa estructura ya se compiló a `Jump`/`Branch` planos antes de llegar a codegen).
2. Una falla nativa nunca aparece en un `throws` declarado (por diseño del lenguaje), así que `lower_throws_check` no puede seguir corriendo solo condicionalmente tras llamadas `throws`-declaradas.
3. Nada hoy sintetiza un cuerpo de método *real* (no stub) para una clase inyectada nativamente — todo lo registrado así hasta ahora es abstracto-nunca-instanciado (`Error`/`Throwable`/`RuntimeError`) o sin cuerpo (`StackTrace`, un solo `Alloc`).

Este cambio ataca las tres, acotado a los cuatro chequeos que D7 marcó como tratables (división por cero, shift inválido, repeat inválido, `NaN`), dejando overflow y cast inválido fuera.

## Goals / Non-Goals

**Goals:**
- Las cuatro fallas nativas tratables son objetos `RuntimeError` reales, capturables por tipo concreto, por `RuntimeError`, o por `Throwable`, con `message()`/`code()`/`cause()`/`stack_trace()` funcionando de verdad.
- El chequeo en sí (la comparación que decide si la falla ocurre) se mueve a `zirk-ir`; codegen dispara la excepción del mismo modo que dispara cualquier otro `InstKind::Throw`.
- Un programa no capturado sigue abortando con código de salida distinto de cero — el comportamiento visible para un programa que nunca usa `try`/`catch` no cambia.

**Non-Goals:**
- `OverflowError`, `InvalidCastError`: fuera de alcance (ver `proposal.md`).
- Ningún cambio a `checked_arith`'s propio mecanismo de overflow ni a `zirk_rt_check_cast`.
- Ninguna dependencia nueva de crate: `zirk-ir` no gana una dependencia del parser ni del lexer.

## Decisions

**D8 — los cuerpos de método de las cuatro clases nuevas se construyen a mano como IR, dentro de `zirk-ir::lower()`, sin pasar por el parser.** Se consideró (y se descarta) un enfoque "prelude": parsear un fragmento de fuente Zirk real con las cuatro clases y fusionarlo al `Program` del usuario antes de chequear, lo que les daría cuerpos completamente ordinarios sin ningún caso especial. Se descarta porque `zirk-sema` no depende hoy de `zirk-parser`/`zirk-lexer` (solo en `dev-dependencies`, para sus propios tests) — convertir esa dependencia en una dependencia real de compilación es un cambio de capas de arquitectura más grande que lo que amerita esta pieza, y further, el punto de inyección tendría que vivir en cada lugar que llama a `zirk_sema::check` (el CLI real y cada test helper de `zirk-ir`/`zirk-sema` que hoy construye su propio `Program`), duplicando el problema en vez de resolverlo una vez.

En cambio, cada una de las cuatro clases tiene exactamente un campo (`reason: String`) y cuatro métodos triviales — construirlos a mano es tratable:
- `construct(reason: String) { this.reason = reason; }` — un `Alloc` más un `StoreField`.
- `message(): String { return this.reason; }` — un `LoadField` más un `Return`.
- `code(): String { return "<CÓDIGO_FIJO>"; }` — una constante de cadena por clase (`E_DIVISION_BY_ZERO`, `E_INVALID_SHIFT`, `E_INVALID_REPEAT`, `E_FLOAT_NAN`) más un `Return`.
- `cause(): Error? { return null; }` — un `NullValue` más un `Return`.
- `stack_trace(): StackTrace { return StackTrace(); }` — reutiliza el mismo `Alloc`-sin-constructor que `fase-4b` (D4) ya usa para construir `StackTrace`.

Este patrón vive junto al `UNREACHABLE_ABSTRACT_METHOD` dummy que `lower()` ya sintetiza a mano hoy (mismo lugar, mismo nivel de "función IR construida directamente, sin AST de por medio") — no es una técnica nueva en la base de código, es la misma que ya existe, aplicada a cuatro cuerpos reales en vez de uno inalcanzable.

**D9 (reutiliza el número solo por comodidad de referencia local a este documento — no relacionado con la D9 de tipos función de `fase-4a`) — las cuatro clases se registran como *concretas*, con su propio `ObjectLayout`/tabla de métodos, no vía el camino `native_exceptions` abstracto de D4.** `Checker::register_native_exception_hierarchy` gana cuatro registros más, cada uno produciendo una clase real con `base: None`, `implements: [RuntimeError]`, campos y métodos poblados igual que si un usuario la hubiera escrito — de modo que virtual dispatch a través de un `Throwable`/`RuntimeError`-tipado funciona exactamente como para cualquier clase de usuario que implementa `RuntimeError` hoy (mecanismo ya probado por D4, sin cambios ahí).

**D10 — los cuatro chequeos se mueven a `zirk-ir/src/lower.rs`, construyendo la excepción y despachando por `lower_pending_exception_dispatch` antes de emitir la operación real, reutilizando D1–D3 sin modificarlos.** Concretamente:
- **División por cero** (`BinaryOp::Div`/`Mod` sobre enteros): antes de emitir la división, compara el divisor con cero; si es cero, construye `DivisionByZeroError("division by zero")`, lo lanza, corre el despacho — igual que `lower_throw`.
- **Shift inválido** (`Shl`/`Shr`): antes de emitir el shift, la misma comparación de rango que hoy vive en `checked_shift` (negativo o `>= bits`), pero como IR `Branch` en vez de LLVM `trap_if`.
- **Repeat inválido** (repetición de `String`): hoy el chequeo vive dentro de `zirk-runtime/src/string.rs` (`zirk_rt_invalid_repeat`), no en codegen — se mueve el chequeo del conteo (negativo) a `zirk-ir`, antes de la llamada a la función de runtime que sigue hacienda la repetición en sí (el runtime deja de rechazar el conteo, solo repite).
- **`NaN`** (resultado de aritmética `Float`): el chequeo compara el resultado consigo mismo (`x != x`) *después* de la operación — a diferencia de los otros tres, este chequea el resultado, no un operando, así que la operación de punto flotante se emite primero, y el chequeo/throw va después, antes de que el resultado se use en cualquier instrucción siguiente.

En los cuatro casos, `codegen` dejó de tener el chequeo — `checked_division`/`checked_shift`/`check_not_nan` en `emit.rs` se simplifican a solo emitir la operación (ya no necesitan `trap_if`), y `zirk_rt_invalid_repeat` deja de llamarse desde `zirk-runtime/src/string.rs` para el caso de conteo negativo (el runtime sigue existiendo para otros invariantes de `str_repeat` si los hay, revisar al implementar).

**D11 (D6 del documento de `fase-4b`, renumerada aquí) — `lower_throws_check` corre después de toda llamada, no solo las `throws`-declaradas.** Antes de este cambio, el chequeo del slot pendiente tras una llamada se cableaba solo cuando el `throws` del callee era no vacío. Con las cuatro fallas nativas ahora lanzando `zirk_rt_throw` desde dentro de una función que puede no declarar ningún `throws` (el `throws` de una falla implícita no se declara, por diseño del lenguaje), el llamador tiene que comprobar el slot pendiente después de *cualquier* llamada para seguir siendo sólido — se acepta el costo uniforme (una comprobación más rama por sitio de llamada) que el propio D6 original ya anticipó y aceptó como trade-off correcto.

## Risks / Trade-offs

- **Costo de ejecución uniforme (D11):** cada llamada en cualquier programa compilado paga un `zirk_rt_has_pending_exception` más una rama, no solo las que declaran `throws`. Aceptado — es exactamente el trade-off que `fase-4b`'s propio D6 ya identificó y recomendó tomar si se hacía este cambio.
- **Los cuerpos IR hechos a mano (D8) no pasan por el chequeador** — si alguno tiene un error de tipos (por ejemplo, un `LoadField` con el índice equivocado), el error solo aparece como un panic en tiempo de lowering o un fallo de `zirk_ir::verify`, no como un diagnóstico de usuario. Aceptado porque el propio código de `UNREACHABLE_ABSTRACT_METHOD` ya sienta este precedente, y el volumen es mínimo (dieciséis cuerpos triviales en total, cuatro por clase).
- **Duplicación de literales de mensaje**: los textos que hoy vive en `zirk-runtime/src/failure.rs` (`"division by zero"`, etc.) se necesitan también como el `reason` que la excepción carga — se centralizan como constantes compartidas entre `zirk-ir` (que las usa como valor de `message()`) y lo que quede de `zirk-runtime`/`fatalError` (que las sigue usando si la excepción escapa sin capturar, para el mensaje final de `zirk_rt_uncaught_exception`), en vez de mantenerlas por separado.

## Migration Plan

Aditivo sobre un pipeline que ya compila y corre. Nada existente cambia de comportamiento salvo el costo por-llamada de D11 (no observable salvo en benchmarks) y la posibilidad nueva de capturar estas cuatro fallas (antes imposible, ahora posible — un programa que nunca usa `try`/`catch` para ellas sigue abortando exactamente igual). Rollback: revertir el merge; nada depende de esto todavía.

## Open Questions

- Ninguna pendiente para el alcance de este cambio — confirmado antes de empezar a implementar, siguiendo el mismo patrón de `fase-4a`/`fase-4b`.
- Abierto para una pasada futura: `OverflowError` (necesita resolver instrucciones IR multi-resultado) e `InvalidCastError` (no investigado en esta pasada) — ver `proposal.md`, "Explícitamente fuera de alcance".
