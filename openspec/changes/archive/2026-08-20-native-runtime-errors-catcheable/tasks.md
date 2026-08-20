## 1. Chequeador — registro de las cuatro clases concretas

- [x] 1.1 `Checker::register_native_exception_hierarchy` (o un helper nuevo llamado desde el mismo sitio): registra `DivisionByZeroError`, `InvalidShiftError`, `InvalidRepeatError`, `FloatNanError` como clases concretas (no abstractas), cada una `implements RuntimeError`, con un campo `reason: String` y los cuatro métodos de `Error`/`Throwable` (`message`, `code`, `cause`, `stack_trace`) poblados en su tabla — mismo mecanismo de índices alineados que D4 ya usa para que `catch RuntimeError(e)` despache bien.
- [x] 1.2 Confirmar (test) que `catch DivisionByZeroError(e)`, `catch RuntimeError(e)` y `catch Throwable(e)` type-checkean contra las cuatro clases nuevas igual que contra cualquier `RuntimeError` de usuario.
- [x] 1.3 Tests: un caso por clase que confirme que es instanciable-solo-por-el-compilador (un usuario no puede escribir `DivisionByZeroError("x")` a mano — considerar si esto necesita un rechazo explícito o si basta con que ninguna sintaxis de construcción exponga el constructor; documentar la decisión tomada en `design.md` si difiere de lo ya escrito)

## 2. IR — cuerpos de método sintetizados a mano (D8)

- [x] 2.1 En `zirk-ir::lower()`, junto a donde se sintetiza `UNREACHABLE_ABSTRACT_METHOD`, construir a mano las funciones IR de `construct`/`message`/`code`/`cause`/`stack_trace` para las cuatro clases (16 cuerpos triviales) — cada una un único bloque, sin ramas salvo donde el propio cuerpo lo pida (ninguno de los cinco necesita rama)
- [x] 2.2 Verificar con `zirk_ir::verify` que estos cuerpos hechos a mano pasan sin error (índices de campo correctos, tipos de retorno correctos)
- [x] 2.3 Constantes de texto compartidas para los cuatro `reason`/`code()` (ver Riesgos en `design.md` sobre duplicación con `zirk-runtime/src/failure.rs`)

## 3. IR — los cuatro chequeos movidos desde codegen (D10)

- [x] 3.1 División por cero: chequeo movido de `checked_division` (`zirk-codegen-llvm/src/emit.rs`) a `zirk-ir/src/lower.rs`, antes de lowerar `BinaryOp::Div`/`Mod` sobre enteros — construye `DivisionByZeroError`, llama `zirk_rt_throw`, corre `lower_pending_exception_dispatch`
- [x] 3.2 Shift inválido: mismo patrón, movido de `checked_shift`
- [x] 3.3 Repeat inválido: movido de `zirk_rt_invalid_repeat` (`zirk-runtime/src/string.rs`) — el chequeo del conteo pasa a `zirk-ir`, antes de la llamada a la función de runtime que repite; confirmar si `str_repeat` en runtime necesita seguir existiendo con su propio chequeo como red de seguridad o si puede asumir un conteo ya validado (documentar la decisión)
- [x] 3.4 `NaN`: movido de `check_not_nan` — a diferencia de los otros tres, el chequeo va *después* de emitir la operación `Float`, sobre el resultado
- [x] 3.5 Simplificar `checked_division`/`checked_shift`/`check_not_nan` en `emit.rs` para que solo emitan la operación (retirar `trap_if` de los tres); confirmar si `trap_if` sigue teniendo otros llamadores (cast inválido, contrato faltante, allocación fallida) antes de considerar retirarlo también — probablemente sigue usándose, no retirar la función en sí
- [x] 3.6 Tests manuales (un `.zrk` real compilado y ejecutado, no solo unit tests) por cada uno de los cuatro: `try { 1 / 0; } catch DivisionByZeroError(e) { ... }` corriendo el `catch`, con salida verificada

## 4. `lower_throws_check` incondicional (D11)

- [x] 4.1 `lower_throws_check`/`call_throws` corren después de toda llamada (función directa, método, método de contrato, y sus contrapartes en posición de sentencia), no solo cuando el `throws` declarado del objetivo es no vacío
- [x] 4.2 Confirmar que esto no rompe ningún test existente por costo/side-effect inesperado (una llamada dentro de una función sin ningún `try` activo y sin `throws` propio ahora también comprueba el slot pendiente y debe re-lanzar/propagar correctamente sin `try_stack` local)
- [x] 4.3 Test: una falla nativa dentro de una función `g()` llamada por `f()`, sin que ninguna de las dos declare `throws` ni tenga `try` propio, capturada por un `try`/`catch` en el llamador de `f()` — confirma que D11 hace observable la falla implícita a través de dos niveles de llamada

## 5. Cierre

- [x] 5.1 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --check` en verde, con `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20` exportado
- [x] 5.2 Corpus válido: al menos un `.zrk` que capture cada una de las cuatro fallas, y uno que deje una sin capturar y confirme que el proceso sigue abortando con código de salida distinto de cero (comportamiento no capturado sin cambios)
- [x] 5.3 `design.md`: sección `## Decisions` revisada tras la implementación real (igual que `fase-4b` reemplazó su mecanismo de propagación original documentado antes de implementar por el que realmente se construyó) — actualizar D8/D10/D11 si la implementación real difiere de lo aquí escrito
- [x] 5.4 Confirmar que ningún test existente que dependía del aborto directo de estas cuatro fallas (buscar `division by zero`, `shift`, `repeat`, `NaN` en `zirk-runtime`/`zirk-codegen-llvm` tests) se rompió por el cambio de comportamiento — si alguno asumía el mensaje exacto de `fatalError`, actualizarlo
