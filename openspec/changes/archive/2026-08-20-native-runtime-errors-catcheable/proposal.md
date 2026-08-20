## Why

`fase-4b-excepciones` construyó `throw`/`try`/`catch`/`finally` con ejecución real, pero dejó fuera, a propósito, una asimetría documentada en su propio `design.md` (decisiones D5–D7): las cinco fallas de seguridad nativas (división por cero, overflow, cast inválido, shift inválido, repeat inválido, `NaN`) siguen abortando el proceso vía `fatalError`, en vez de producir un `RuntimeError` catcheable como `docs/ERROR_RESOURCE_PERMISSION_SEMANTICS.md` sección 3 exige explícitamente ("can be caught but need not be listed in every function signature"). Un programa que hoy escribe `try { 1 / 0; } catch DivisionByZeroError(e) { ... }` no compila esa expectativa: el chequeador la deja pasar (nada le dice que `DivisionByZeroError` no existe como excepción real) y en tiempo de ejecución el proceso aborta igual, sin pasar por el `catch`.

D7 ya separó el trabajo en una porción tratable y una que necesita spike propio: los cuatro chequeos de cero/negativo/rango (división por cero, shift inválido, repeat inválido, `NaN`) pueden moverse a `zirk-ir` reutilizando el mecanismo D1–D3 de `fase-4b` tal cual; el quinto, overflow, necesita resolver primero la pregunta de instrucciones IR multi-resultado (`{value, overflowed}`) que `checked_arith` usa hoy solo a nivel LLVM, y queda fuera de este cambio.

## What Changes

- Cuatro nuevas clases nativas concretas, inyectadas con el mismo mecanismo "tablas directas" que `Error`/`Throwable`/`RuntimeError` (`fase-4b`, D4) pero **concretas e instanciables**, no abstractas: `DivisionByZeroError`, `InvalidShiftError`, `InvalidRepeatError`, `FloatNanError` — las cuatro `implements RuntimeError`, cada una con `message()`/`code()`/`cause()`/`stack_trace()` reales y llamables (no el stub `UNREACHABLE_ABSTRACT_METHOD`).
- Los cuatro chequeos correspondientes se mueven de `zirk-codegen-llvm/src/emit.rs` (`checked_division`, `checked_shift`, `check_not_nan`, y el chequeo de `zirk_rt_invalid_repeat` hoy en `zirk-runtime/src/string.rs`) a `zirk-ir/src/lower.rs`, construyendo la excepción y llamando al mecanismo D1–D3 existente (`zirk_rt_throw` + `lower_pending_exception_dispatch`) antes de emitir la instrucción aritmética/de repetición real — igual que hoy se baja un `throw` explícito.
- `Lowering::lower_throws_check` pasa a correr después de **toda** llamada, no solo las que declaran `throws` (D6) — una falla nativa implícita nunca aparece en un `throws` por diseño (`ERROR_RESOURCE_PERMISSION_SEMANTICS.md` lo dice explícitamente), así que ningún análisis estático del `throws` declarado del callee basta para decidir si comprobar el slot pendiente tras la llamada.
- `catch RuntimeError(e)` y `catch Throwable(e)` capturan las cuatro nuevas excepciones igual que cualquier otra; un programa que no las captura y no las declara (no hace falta declararlas — son implícitas) sigue abortando si escapan de `main`, vía el mismo `zirk_rt_uncaught_exception` de `fase-4b`.

### Explícitamente fuera de alcance

- **`OverflowError`** — necesita resolver la pregunta de instrucciones IR multi-resultado primero (D5/D7). Sigue abortando vía `fatalError`, sin cambios.
- **Cast inválido** (`InvalidCastError`) — no está en la lista tratable que D7 identificó; el chequeo vive en un camino de codegen distinto (`zirk_rt_check_cast`) que no se investigó en esta pasada. Sigue abortando.
- **`suppressed` poblado desde una falla de limpieza en `finally`** — depende de `List<T>` (Fase 7), sin cambios respecto a `fase-4b`.
- **Trazas de pila reales** (`stack_trace()` sigue devolviendo un `StackTrace` vacío, sin marcos) — sin cambios respecto a `fase-4b`.

## Impact

- Specs afectadas: `zirk-errors` (cierra la asimetría "pueden ser capturadas" que `fase-4b-excepciones` dejó documentada pero no implementada para estas cuatro).
- Costo de ejecución: un `zirk_rt_has_pending_exception` más una rama, en cada sitio de llamada del programa compilado (D6) — antes, solo en las llamadas cuyo objetivo declaraba `throws`.
- Sin cambios de ruptura: ningún programa que hoy compila deja de compilar; los cuatro chequeos siguen abortando el proceso si nadie los captura, exactamente como antes — solo que ahora un `catch` explícito puede interceptarlos.
