## 1. Léxico y gramática

- [x] 1.1 Palabras clave nuevas `throw`/`throws`; graduar `try`/`catch`/`finally` de reservadas a implementadas
- [x] 1.2 `throw expr;` / `throw;` (relanzamiento, solo válido dentro de un `catch` — el chequeador lo valida, no el parser)
- [x] 1.3 `try { } catch Tipo(nombre) { } ... finally { }`, exigiendo al menos un `catch` o un `finally` (`E0313`)
- [x] 1.4 `throws Tipo (| Tipo)*` en la firma de una función o método, reutilizando `parse_type`'s propia sintaxis de unión
- [x] 1.5 Tests: un caso válido y uno inválido por cada regla nueva

## 2. Jerarquía `Error`/`Throwable`/`RuntimeError`/`StackTrace`

- [x] 2.1 Registrar las tres como `abstract class`, igual mecanismo "inyectar las tablas directamente" que `Result`/`Iteration<T>` (D4)
- [x] 2.2 `StackTrace` como clase concreta mínima, construida estructuralmente (sin `program.classes` real, D4)
- [x] 2.3 Corregir `declare_class` para sembrar `methods` desde `self.native_exceptions` antes de las propias, alineando índices para el despacho virtual (D4)
- [x] 2.4 Corregir `CallVirtual` en `zirk-codegen-llvm` para construir la firma de la llamada indirecta desde el tipo de retorno de la propia instrucción, no desde `self.functions[&layout.methods[index]]` (D4)
- [x] 2.5 Extender `ancestors` de un `ObjectLayout` para incluir `abstract_bases` transitivamente (roadmap Phase 4b), necesario para que `catch Throwable(e)` reconozca en runtime una clase que solo implementa `RuntimeError`

## 3. Chequeador — validación y análisis de efectos

- [x] 3.1 `Checker::implements_abstract_class`: caminata transitiva sobre `abstract_bases`, separada de `is_subclass_of`'s propio chequeo no transitivo (ver Riesgos en `design.md`)
- [x] 3.2 `is_throwable_type`/`resolve_throws_clause`: valida que `throw`/`throws`/`catch` nombren un tipo que implementa `Throwable`
- [x] 3.3 `pending_throws`/`current_throws`: análisis "capturar o declarar" — cada `throw`, relanzamiento, o llamada a una función `throws` se acumula y se verifica contra el `throws` de la función contenedora (`E0434`)
- [x] 3.4 Orden de `catch`: uno que un `catch` anterior ya cubre es inalcanzable (`E0436`)
- [x] 3.5 `throw;` fuera de un `catch` es un error (`E0437`)
- [x] 3.6 `return`/`break`/`continue`/`throw` directamente dentro de un `finally` es un error (`E0435`) — regla más estricta que la del spec ("solo cuando reemplazaría un resultado activo"), deliberadamente (D3)
- [x] 3.7 `check_try` participa en el análisis de "todo camino retorna" (cuerpo y cada `catch` deben retornar)
- [x] 3.8 Tests: un caso válido y uno inválido por cada regla nueva

## 4. IR y ejecución real (D1, D2, D3)

- [x] 4.1 Runtime (`zirk-runtime`): `zirk_rt_throw`/`zirk_rt_has_pending_exception`/`zirk_rt_take_pending_exception` sobre un slot `thread_local`, más `zirk_rt_is_instance` (variante booleana de `zirk_rt_check_cast`) y `zirk_rt_uncaught_exception`
- [x] 4.2 IR: `InstKind::Throw`/`HasPendingException`/`TakePendingException`/`IsInstance`/`Undefined`, con su verificación en `verify.rs` y codegen en `emit.rs`
- [x] 4.3 `Lowering::try_stack`: cada `catch` construye su bloque manejador y su slot de binding antes de bajar el cuerpo del `try` (D2)
- [x] 4.4 `lower_throw`/`lower_pending_exception_dispatch`: toma la excepción pendiente, prueba cada `catch` activo de adentro hacia afuera, salta al primero que coincide o repropaga tras correr cada `finally` en el camino (D1/D3)
- [x] 4.5 `lower_throws_check`: después de cada llamada cuyo objetivo declara `throws`, comprueba la excepción pendiente — cableado en los cuatro sitios de llamada (función directa, método, método de contrato, y sus contrapartes en posición de sentencia)
- [x] 4.6 `opens_blocks` no necesitó extenderse para llamadas `throws` en posición de sentencia (ya cubierto por `lower_expr_for_effect`'s propio cableado), pero el propio checkpoint de `lower_throw`/`lower_throws_check` sí necesitó descubrir y corregir un bug real: `lower_throw` retornaba incondicionalmente de la función en vez de consultar `try_stack` primero
- [x] 4.7 `emit_c_entrypoint`: tras `main`, comprueba la excepción pendiente y aborta vía `zirk_rt_uncaught_exception` si sigue activa (una excepción no capturada en `main` sale con código distinto de cero)
- [x] 4.8 Tests: escenarios manuales verificados (catch por tipo concreto, catch por `Throwable`, relanzamiento a través de una función `throws`, propagación no capturada), corpus válido e inválido

## 5. Cierre

- [x] 5.1 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --check` en verde
- [x] 5.2 `design.md`: sección `## Decisions` completada (D1–D4) tras reemplazar el mecanismo de propagación original (retorno invisible) por el slot `thread_local` realmente implementado
- [x] 5.3 Regresiones de `zirk-ir/tests/lowering.rs` corregidas: los tests que asumían `objects[0]`/`Object(0)` para la primera clase de usuario ahora usan `Module::object_id` por nombre, ya que las cuatro clases nativas se registran antes
- [x] 5.4 Regresión de `zirk-parser/tests/grammar.rs` corregida: `try`/`catch`/`finally` ya no son "de fase posterior"; el código `UNEXPECTED_TOKEN` que usaba el propio `try` vacío se reemplazó por `E0313` (`EMPTY_TRY`), dedicado
