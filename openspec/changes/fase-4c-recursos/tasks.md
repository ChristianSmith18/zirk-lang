## 1. Gramática

- [x] 1.1 `MatchExpr` gana `with_binding: Option<Ident>` (`zirk-ast`)
- [x] 1.2 `parse_match` reestructurado: scrutinee primero, `with binding`
      opcional después de él, antes de `{` — reemplaza el guard que emitía
      `NOT_IMPLEMENTED` en la posición equivocada
- [x] 1.3 Tests: un caso válido (`match ... with binding { ... }` parsea con
      `with_binding` poblado) y confirmación de que la ausencia de `with`
      sigue dando `None`

## 2. `Resource<E>` como contrato nativo

- [x] 2.1 `Checker::register_native_resource_contract`: `interface
      Resource<E from Error> { fn close(): Result<Void,E>; fn is_closed():
      Boolean; }`, registrada después de `register_native_exception_hierarchy`
      (necesita `Error` para la restricción de `E`) y de
      `register_native_result_enum` (necesita `Result` para el retorno de
      `close()`)
- [x] 2.2 `is_native_contract_name` incluye `"Resource"`
- [x] 2.3 `resolve_implements_args`'s propio gate `NOT_LOWERED` exceptúa
      `Resource<E>`: no necesita tabla de despacho especializada, ya que
      `close()`/`is_closed()` siempre se alcanzan por despacho estático sobre
      la clase concreta, nunca a través de una referencia tipada por el
      contrato

## 3. Chequeador — validación de `match ... with`

- [x] 3.1 `Checker::check_resource_match_scrutinee`: el scrutinee debe ser
      `Result<R,Err>` no nulable (`E0438`)
- [x] 3.2 `Checker::check_resource_binding_type`: la rama que destructura
      `with_binding`'s nombre en su propio patrón debe tener un tipo que
      implemente `Resource<E>` (`E0438`)
- [x] 3.3 Ninguna rama destructura el nombre de `with_binding`: error
      (`E0438`)
- [x] 3.4 Código nuevo `codes::INVALID_RESOURCE_MATCH` (`E0438`, siguiente
      libre tras `E0437`)

## 4. IR — cierre automático real (D1, D2)

- [x] 4.1 `FunctionLowering::pattern_binds`: si el patrón de una rama
      (bare o dentro de `Variant.bindings`) nombra `with_binding`
- [x] 4.2 `FunctionLowering::resource_close_block`: sintetiza
      `binding.close();` como un `ast::Block` de una sentencia — un
      `ast::Expr::Call` sobre un `ast::Expr::Field`, sin pasar por el
      chequeador
- [x] 4.3 `lower_match`: la rama que adquiere el recurso empuja un
      `TryFrame { catches: vec![], finally: Some(<bloque sintético>) }`
      antes de bajar su cuerpo y lo desapila después — reutiliza
      `run_finally_through`/`lower_pending_exception_dispatch` de
      `fase-4b-excepciones` sin cambios para `return`/`break`/`continue`/una
      excepción propagada
- [x] 4.4 Finalización normal (la rama no sale temprano): corre el `finally`
      sintético antes de guardar el resultado y saltar al bloque de
      continuación, el mismo patrón que `lower_try`'s propio chequeo
      post-cuerpo

## 5. Gap preexistente descubierto: `Result<Void,E>` en codegen (D4)

- [x] 5.1 `enum_struct`: un campo `Void` ocupa un struct LLVM de tamaño cero
      (`{}`) en vez de hacer panic, preservando la numeración de índices de
      `EnumLayout.variants`
- [x] 5.2 `InstKind::BuildEnum`: salta la inserción de valor para un campo
      `Void` — no hay operando que insertar
- [x] 5.3 `InstKind::LoadField`: retorna `None` cuando el tipo del resultado
      es `Void`, igual que cualquier otra instrucción que no produce valor
- [x] 5.4 `value_struct`/`object_struct`/captura de closures deliberadamente
      sin tocar: el chequeador nunca admite un campo o una captura `Void`
      (`VOID_VARIABLE`), así que ningún programa ejercita esos caminos

## 6. Corpus y tests

- [x] 6.1 `crates/zirk-cli/tests/corpus/valid/resources_match_with_closes_on_every_exit.zrk`
      + `.out`: una clase implementa `Resource<OpenError>`, `match ... with`
      cierra en finalización normal y en un `return` temprano
- [x] 6.2 `crates/zirk-cli/tests/corpus/valid/resources_match_with_closes_on_thrown_exception.zrk`
      + `.out`: el mismo mecanismo cierra el recurso cuando una excepción se
      propaga a través de la rama que lo adquirió
- [x] 6.3 `crates/zirk-sema/tests/typing.rs`: siete tests nuevos — clase que
      implementa `Resource`, `match ... with` aceptado (finalización normal
      y `return` temprano), clase que no implementa el contrato completo
      (`MISSING_IMPLEMENTATION`), y tres formas de `INVALID_RESOURCE_MATCH`
      (ninguna rama destructura el binding, scrutinee no es `Result`,
      binding no implementa `Resource<E>`)
- [x] 6.4 `crates/zirk-parser/tests/grammar.rs`: la entrada `match with`
      de la lista de "construcciones de fases posteriores" se retira (ya
      está implementado); `valid_match_with_states_its_phase` se reemplaza
      por dos tests de gramática reales

## 7. Cierre

- [x] 7.1 `cargo test --workspace` (799 tests), `cargo clippy --workspace
      --all-targets`, `cargo fmt --check` en verde
- [x] 7.2 Verificación manual con `zirk run` sobre ambos programas del
      corpus, confirmando el orden de salida exacto (incluida la excepción
      capturada tras el cierre)
- [x] 7.3 `openspec/specs/zirk-resources/spec.md`: delta `MODIFIED
      Requirement` narrowing la primera de sus cinco secciones a lo que este
      cambio realmente implementa
