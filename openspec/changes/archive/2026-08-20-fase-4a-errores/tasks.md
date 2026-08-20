## 1. `Base::Never` y `fatalError`

- [x] 1.1 Añadir `Base::Never` a `zirk-sema`: `accepts` lo admite en cualquier parte, `unify` no aporta nada en un punto de unión (D2)
- [x] 1.2 Registrar `fatalError(message: String): Never` como función reconocida por el compilador
- [x] 1.3 `IrType::Never`, `InstKind::FatalError`, y su verificación en `verify.rs`
- [x] 1.4 Lowering: `lower_ternary`/`lower_if_expr` sin `Store`+`Jump` en la rama `Never`; `lower_expr_as` cae a `default_value` para un inicializador que diverge
- [x] 1.5 `zirk_rt_fatal_error` en el runtime, símbolo `noreturn` en `zirk-codegen-llvm`
- [x] 1.6 Rechazar `enum` sin variantes, indicando `Never` en el mensaje
- [x] 1.7 Tests: un caso válido y uno inválido por cada regla nueva, más corpus (`never_and_fatal_error.zrk`, `empty_enum_instead_of_never.zrk`)

## 2. `Result<T,E>` — registro y construcción

- [x] 2.1 Auditar la maquinaria de instanciación de enum genérico existente (`Iteration<T>`) para arity 2 antes de comprometerse a la forma completa
- [x] 2.2 `register_native_result_enum`: inyectar `Result` en `self.enums` igual que `register_native_iteration_contracts` hace con `Iteration<T>`
- [x] 2.3 `expected_type` (D1): campo de un solo uso en `Checker`, sembrando `infer_type_params` desde el contexto — necesario para que `Result.Ok(5)` resuelva `E` sin que ningún argumento lo determine
- [x] 2.4 Corregir `resolve_written_type` en `zirk-ir/lower.rs`: la rama de enum no consultaba `reference.arguments` (bug E, D5) — `Result<Int32,String>` escrito como anotación resolvía a la plantilla vacía, no a la instancia

## 3. `Result<T,E>` — `match` y exhaustividad

- [x] 3.1 `expect_pattern_type` no reconocía un patrón contra `Base::EnumInstance` (bug A)
- [x] 3.2 `check_variant_pattern` no sustituía los tipos de campo por los argumentos concretos de la instancia (bug B)
- [x] 3.3 `check_exhaustive` no resolvía el id de enum desde `Base::EnumInstance` (bug C)
- [x] 3.4 IR: `lower_match` y `declare_pattern_types` indexaban `module.enums[]` con el id de la plantilla en vez de la copia especializada (bug D)
- [x] 3.5 Tests: construcción y `match` desestructurando ambas variantes, corpus (`result_construction_and_match.zrk`)

## 4. `Result<T,E>` — API de métodos

- [x] 4.1 Despacho estructural en el chequeador (`check_result_method_call`), igual patrón que `to_string()` nativo — sin tabla de métodos de enum
- [x] 4.2 `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `unwrap`, `unwrap_error` en IR (`Lowering::result_method`/`lower_result_method_call`)
- [x] 4.3 `opens_blocks` no sabía que una llamada a un método de `Result` abre bloques por sí misma, no solo por sus argumentos (D6) — convertido a método de `Lowering`
- [x] 4.4 `unwrap`/`unwrap_error` sobre la variante equivocada invocan `fatalError` (D7), reutilizando `lower_fatal_error` extraído de `lower_fatal_error_call`
- [x] 4.5 `get_or_else` movido fuera de alcance tras chocar con la decisión D9 (tipos función sin sintaxis) — no es un caso especial de `Result`, es una restricción general del lenguaje (D4)
- [x] 4.6 Tests unitarios de los siete métodos, más casos inválidos (método desconocido, tipo de argumento incorrecto, aridad incorrecta)

## 5. Consumo obligatorio

- [x] 5.1 `require_result_consumed`: una sentencia de expresión cuyo tipo es una instancia de `Result` es un error (`E0433`)
- [x] 5.2 `_ = expr;`: `check_assign` reconoce el nombre `_` como descarte explícito, sin resolverlo como binding
- [x] 5.3 IR: `lower_assign` descarta el valor de `_ = expr;` sin buscar un slot que nunca se declaró
- [x] 5.4 Tests: un caso válido (`_ = load();`) y uno inválido (sentencia descartada) por cada camino, más corpus inválido (`discarded_result.zrk`)

## 6. Cierre

- [x] 6.1 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --check` en verde
- [x] 6.2 `design.md`: sección `## Decisions` completada (D1–D8)
- [x] 6.3 `proposal.md`: alcance corregido para mover `get_or_else` junto a los combinadores genéricos
