## 1. Léxico y gramática

- [ ] 1.1 `Fn(P...) => R`/`Function(P...) => R` parseable en: tipo de parámetro, tipo de retorno, tipo de campo/atributo, argumento de tipo genérico, anotación de variable local — reemplazando `Parser::reject_function_type` por un parseo real
- [ ] 1.2 Labels de parámetro, `name?: T` opcional, `...values: T` variadic dentro de la lista de parámetros del tipo callable
- [ ] 1.3 Tests: un caso válido y uno inválido por cada posición nueva

## 2. Chequeador — tipo callable real

- [ ] 2.1 `resolve_written_type` (o su equivalente) construye un `Base::Function`/`FnType` real desde la anotación escrita, reutilizando `Checker::intern_fn_type` (interning estructural, ya existe) — no el camino no-interned que usan hoy los lambdas
- [ ] 2.2 Compatibilidad contravariante en parámetros / covariante en resultado, con labels/opcionalidad/variadic coincidentes, para: función nombrada, lambda sin capturas, método bound/unbound
- [ ] 2.3 `expect_assignable`: una función nombrada o lambda sin capturas hacia una posición `Fn(...) => R` se acepta estructuralmente (D12) — retirar o acotar el rechazo actual de "cada closure tiene su propio tipo" para este caso específico, sin tocar el rechazo entre dos closures con capturas distintas
- [ ] 2.4 D14: un closure con capturas satisface una posición `Fn(...) => R` solo cuando el literal aparece directamente en esa posición (inicializador, argumento en su sitio de llamada, expresión de `return`, valor de campo) — no una referencia a una variable ya vinculada
- [ ] 2.5 D14: el tipo de retorno declarado de una función que es un `Fn(...) => R` con capturas exige que todo `return` que produzca un `Base::Function` remita al mismo literal (mismo span/id) — dos literales distintos se rechazan con un diagnóstico nuevo y claro, no `TYPE_MISMATCH` genérico
- [ ] 2.6 D14: reasignar un local `Fn(...) => R` de un literal con capturas a uno distinto se rechaza con el mismo diagnóstico
- [ ] 2.7 `invalid_closure_returned_from_a_function` e `invalid_generic_inference_from_a_closure_argument` (`crates/zirk-sema/tests/typing.rs`) pasan a ser casos válidos para la forma de un solo literal; añadir el caso que sigue rechazado (dos literales distintos) como test nuevo
- [ ] 2.8 `is` entre dos valores callable se acepta (D15); `==`/`!=` siguen rechazados sin cambios — test de ambos
- [ ] 2.9 Lambda recursiva con tipo de binding explícito (`mut fact: Fn(Int32) => Int32 = (n: Int32): Int32 => ...;`) — test manual con un programa real

## 3. IR y codegen

- [ ] 3.1 Confirmar (no asumir) que un closure con capturas, devuelto/almacenado/pasado bajo el caso de un solo literal (D14), lowera y ejecuta correctamente — programa real que retorna un closure con una captura y lo invoca después de que la función que lo creó retornó
- [ ] 3.2 `is` entre dos valores `IrType::Closure`: comparación de identidad sobre el struct completo (función + capturas), generalizando el mismo patrón que `fase-4c`'s fix de `is` sobre `T?` en `emit.rs`
- [ ] 3.3 Verificar que ningún camino de lowering asuma todavía "un closure nunca escapa" (buscar comentarios tipo "cannot escape in this phase" en `lower.rs` y actualizarlos si ya no aplican)

## 4. Cierre

- [ ] 4.1 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --check` en verde, con `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20` exportado
- [ ] 4.2 Corpus válido: al menos un `.zrk` real que (a) asigna una función nombrada a un `Fn(...) => R` local, (b) retorna un closure con una captura y lo invoca desde `main` después del retorno, (c) usa una lambda recursiva con binding explícito
- [ ] 4.3 Corpus inválido: al menos un `.zrk` que intente que dos closures con capturas distintas satisfagan la misma posición `Fn(...) => R` (dos `return` con literales distintos), confirmando el diagnóstico nuevo de D14, no un panic ni un `TYPE_MISMATCH` genérico confuso
- [ ] 4.4 `design.md`: sección `## Decisions` revisada tras la implementación real si algo difiere de D12–D15 (mismo patrón que fases anteriores)
- [ ] 4.5 Actualizar `docs/handbook/11-reference/12-feature-status.md` y `docs/init/ZIRK_ROADMAP.md` (Phase 4d): reflejar exactamente qué quedó cubierto (nombradas/sin-capturas interoperables, un solo literal con capturas, `is`) y qué sigue pendiente (polimorfismo general D13, celda compartida para capturas mutables, `.clone()`)
