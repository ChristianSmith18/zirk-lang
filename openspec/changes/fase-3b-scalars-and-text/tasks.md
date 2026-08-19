## 1. Auditoría previa (bloquea todo lo demás)

- [x] 1.1 Listar cada sitio en `zirk-sema`/`zirk-ir`/`zirk-codegen-llvm` que compara contra `Base::Int32`/`IrType::Int32` directamente, y clasificar cada uno: generaliza a "cualquier entero" sin más, o asume 32 bits por una razón real que necesita una decisión explícita (por ejemplo, el ancho de un discriminante de enum) — resultado completo en `design.md`, addendum de D1: ~70 sitios en total, concentrados en `zirk-sema/checker.rs` (decisión real: tabla de aritmética nativa, impresión, default, orden de interning) y `zirk-ir/lower.rs` (mayormente mecánico); el backend LLVM es casi gratis, un solo sitio
- [x] 1.2 Con esa lista, confirmar o recortar el alcance de esta fase antes de escribir ningún ancho nuevo (design.md, riesgo de alcance) — decidido: una sola migración atómica con los diez anchos desde el principio, no una generalización seguida de anchos agregados después; la exhaustividad de `match` en Rust garantiza que ningún sitio quede a medias

## 2. Léxico

- [ ] 2.1 Literales enteros con sufijo o inferencia de ancho, según lo que la gramática existente ya distinga
- [ ] 2.2 Literales `Float`, con notación científica y default `Float64`
- [ ] 2.3 Literal `Char`, delimitado por comillas simples, capaz de un grapheme extendido de más de un code point
- [x] 2.4 `{expr}` dentro de un literal de `String`, con `\{`/`\}` como escape — encontrado ya implementado en el léxico desde antes de esta fase (`TokenKind::InterpolatedStr`/`StrPart`, con conteo de profundidad de llaves para que una expresión anidada con las suyas propias no cierre la interpolación antes de tiempo, y el escape ya reconocido); nada que hacer aquí, solo confirmar y dejar de gatearlo en el parser (3.2)
- [x] 2.5 Tests: un caso válido y uno inválido por cada literal nuevo — hecho para interpolación (ya cubierto por tests de léxico preexistentes, ver 2.4); pendiente `Float`/`Char`

## 3. Gramática

- [x] 3.1 Operadores bitwise y de shift (`&`, `|`, `^`, `~`, `<<`, `>>`), en los niveles de precedencia que el spec fija, distintos de `&&`/`||` — precedencia propia decidida durante la implementación (el roadmap no fija números exactos): bitwise entre comparación y aditiva, shift más apretado que los otros tres, mismo orden relativo que C/Rust/Swift (`Or=1, And=2, Eq/Is=3, Compare=4, Coalesce=5, BitOr=6, BitXor=7, BitAnd=8, Shl/Shr=9, Add/Sub=10, Mul/Div/Rem=11`). El léxico ya tenía cada token (`Amp`, `Pipe`, `Caret`, `Tilde`, `Shl`, `Shr` y sus formas `=`) modelado desde antes de esta fase, con su propio gate `Phase::THREE_B` en `TokenKind::phase()` — retirado para esta familia (no para `**`, que sigue esperando a `Float`)
- [x] 3.2 AST para una expresión interpolada: texto literal más una lista de sub-expresiones, preservando el orden de aparición — `Expr::Interpolated(InterpolatedStrExpr)` nuevo, con `parts: Vec<InterpolatedPart>` (`Literal(String)` | `Expr(Expr)`). El parser retira su propio gate (`pending_literal`) y re-lexa cada `{expr}` por separado — el léxico guarda el texto crudo en vez de tokenizarlo inline precisamente para esto (`StrPart::Expr`'s own doc comment) — desplazando cada span resultante por el offset donde la interpolación empieza, así un diagnóstico dentro de `{expr}` señala el lugar real del archivo en vez de la posición 0 de un texto que nadie escribió como archivo propio
- [x] 3.3 Tests: un caso válido y uno inválido por cada regla nueva — hecho para bitwise/shift (`crates/zirk-cli/tests/corpus/valid/bitwise_and_shift.zrk`, `invalid/bitwise_on_non_integer.zrk`, más `typing.rs`) y para interpolación (`valid/string_interpolation.zrk`, `invalid/interpolation_of_non_printable.zrk`, más forma del árbol en `grammar.rs` y tipos en `typing.rs`)

## 4. Tipos — anchos enteros

- [ ] 4.1 Registrar `Int8`/`Int16`/`Int64`/`Int128` y la familia `UInt8`…`UInt128` como tipos del chequeador
- [ ] 4.2 Aritmética comprobada por ancho y señal, reutilizando el mecanismo de `Int32` de la Fase 1 (ver 1.1)
- [ ] 4.3 Ensanchamiento implícito sin ambigüedad; angostamiento, cambio de señal y cualquier conversión con pérdida, siempre explícitos
- [x] 4.4 Operadores bitwise/shift sobre enteros — hecho para `Int32` (el único ancho que existe todavía; el resto de anchos es 4.1/1.2, migración aparte). `BinaryOp`/`UnaryOp` de `zirk-ast` y de la IR ganan `BitAnd`/`BitOr`/`BitXor`/`Shl`/`Shr`/`BitNot`; el checker exige números en ambos operandos (`expect_numeric`, mismo mensaje que la comparación) y tipa `Int32`. Un shift por una cantidad negativa o `>= 32` es indefinido a nivel de LLVM, así que se comprueba antes de la instrucción nativa (`checked_shift`, mismo patrón que `checked_division`) y aborta con `zirk_rt_invalid_shift` — símbolo de runtime nuevo, siguiendo la granularidad ya establecida (uno por categoría de fallo, no uno genérico). `&=`/`|=`/`^=`/`<<=`/`>>=` se desazucaran igual que `+=` ya lo hacía. Verificado con un programa real compilado y corrido, incluido el camino de fallo (shift por `-1`, exit code 70)
- [ ] 4.5 Tests: un caso válido y uno inválido por cada regla nueva — hecho para bitwise/shift (ver 3.3); pendiente ensanchamiento/angostamiento/cambio de señal, que necesitan 4.1

## 5. Tipos — `Float`

- [ ] 5.1 Registrar `Float16`/`Float32`/`Float64`/`Float128`, con `Float` alias de `Float64`
- [ ] 5.2 Infinitos explícitos como valores válidos; identificar cada operación que bajo IEEE 754 produciría `NaN` y marcarla para el chequeo en tiempo de ejecución (diseño: qué operaciones necesitan la comprobación, no solo cuáles existen)
- [ ] 5.3 Aritmética mixta entero/`Float` produce `Float`
- [ ] 5.4 Tests: un caso válido y uno inválido por cada regla nueva, incluido al menos un caso de `NaN` evitado en tiempo de ejecución

## 6. Tipos — `Char`

- [ ] 6.1 Registrar `Char` como tipo del chequeador
- [ ] 6.2 Resolver la pregunta abierta de representación (design.md) antes de fijar cómo se tipa un literal o el elemento de iterar un `String`
- [ ] 6.3 `for ... in` sobre `String` produce `Char`, retirando la deuda anotada desde la Fase 2
- [ ] 6.4 Tests: un caso válido y uno inválido por cada regla nueva, incluido un grapheme extendido de más de un code point

## 7. Tipos — conversión contextual profunda

- [ ] 7.1 Un constructor explícito (`Float(expr)`, `String(expr)`, …) establece contexto sobre el árbol de operadores compatible que contiene directamente
- [ ] 7.2 El contexto convierte operandos antes de operar, no el resultado después
- [ ] 7.3 El contexto no muta los operandos originales ni cruza a una función llamada dentro de la expresión
- [ ] 7.4 Decidir si el contexto cruza un operador sobrecargado por contrato (design.md, pregunta abierta) y verificarlo
- [ ] 7.5 Tests: un caso válido y uno inválido por cada regla nueva

## 8. Tipos — `to_string()` e interpolación

- [ ] 8.1 Confirmar el nombre y la convención exactos del contrato (design.md, pregunta abierta) contra `ZIRK_LANGUAGE_SPEC.md`/`ZIRK_STDLIB_SPEC.md`
- [ ] 8.2 Registrar `to_string()` como contrato reservado, en la misma familia que los contratos de operador de la Fase 3
- [ ] 8.3 Implementación nativa de `to_string()` para cada escalar (los diez enteros, `Float`, `Boolean`, `Char`, `String`)
- [ ] 8.4 Un tipo del usuario (clase, record, enum) puede implementar `to_string()`
- [ ] 8.5 `print`/`println` rutean por `to_string()`; retirar `require_printable` y su lista cerrada
- [ ] 8.6 Desazucarar `"{expr}"` a texto literal concatenado con `expr.to_string()` — la forma de la bajada ya existe (`lower_interpolated`, cadena de `InstKind::Concat`, cada pieza pasada por `InstKind::ToString` cuando no es ya `String`) y quedó verificada con programas reales, incluida una expresión interpolada que abre bloques (`if`) — un caso donde los valores ya computados debían pasar por slot para no cruzar bloques (ADR-007), igual que `lower_held_args` ya hace para los argumentos de una llamada. Lo que falta es que esa conversión sea `to_string()` de verdad: hoy reutiliza el mismo `require_printable`/`ToString` fijo que `println` (Int32/Boolean/String únicamente) porque el contrato real todavía no existe (8.1-8.5) — cuando aterrice, este mismo desazúcar pasa a despachar por él en vez de por la lista cerrada, sin cambiar la forma de la cadena de `Concat`
- [ ] 8.7 Tests: un caso válido y uno inválido por cada regla nueva

## 9. Runtime

- [ ] 9.1 Soporte de fallo controlado para la comprobación de `NaN`/dominio inválido de `Float` (mismo patrón que overflow de entero, `zirk-runtime/src/failure.rs`)
- [ ] 9.2 Lo que `Char` necesite del runtime, una vez resuelta la pregunta abierta de representación (6.2)

## 10. IR

- [ ] 10.1 Tipo entero de la IR parametrizado por ancho y señal, no una variante por ancho (design.md D1)
- [ ] 10.2 Tipo `Float` de la IR, parametrizado por ancho
- [ ] 10.3 Tipo `Char` de la IR, según la representación decidida en 6.2
- [ ] 10.4 Bajar la comprobación de overflow por ancho/señal, reutilizando el patrón de `Int32`
- [ ] 10.5 Bajar la comprobación de `NaN`/dominio inválido de `Float`
- [ ] 10.6 Bajar la conversión contextual: operandos convertidos antes de la operación en la IR resultante
- [ ] 10.7 Bajar la interpolación: llamadas a `to_string()` concatenadas, en orden
- [ ] 10.8 Extender el verificador a los tipos y comprobaciones nuevas
- [ ] 10.9 Tests: IR esperada para cada construcción nueva

## 11. Backend LLVM

- [ ] 11.1 Emitir cada ancho entero sobre el `IntType` de LLVM correspondiente, con el intrínseco de overflow con o sin signo según la señal
- [ ] 11.2 Emitir cada ancho `Float` sobre el `FloatType` de LLVM correspondiente
- [ ] 11.3 Emitir la comprobación de `NaN`/dominio inválido explícitamente alrededor de la operación nativa
- [ ] 11.4 Emitir el despacho a `to_string()` con el mismo mecanismo que cualquier otra llamada a método (directo, tabla propia o de contrato, ADR-013)
- [ ] 11.5 Lo que `Char` necesite del backend, según 6.2/9.2
- [ ] 11.6 Tests: el módulo LLVM generado verifica para cada construcción nueva

## 12. Verificación de punta a punta

- [ ] 12.1 Ampliar el corpus con programas válidos: cada ancho entero, `Float`, `Char`, conversión contextual, bitwise/shift, `to_string()` en un tipo del usuario, interpolación
- [ ] 12.2 Ampliar el corpus con programas inválidos: overflow por ancho, `NaN` evitado, conversión sin marcar, tipo sin `to_string()`
- [ ] 12.3 Verificar que el corpus de las fases 1 a 3 sigue verde sin modificarlo, salvo los sitios que la auditoría de 1.1 identificó como necesarios
- [ ] 12.4 Sondear cruces entre construcciones: `Float` genérico (`Box<Float64>`), `Char` en un record, `to_string()` sobre un value class, interpolación dentro de un contrato de operador
- [ ] 12.5 Confirmar que CI pasa en las cuatro plataformas de la matriz

## 13. Cierre

- [ ] 13.1 Actualizar `docs/init/ZIRK_AGENT_PROMPT.md` con el estado de la fase
- [ ] 13.2 Registrar en un ADR la representación de `Char`, dada su complejidad real (design.md)
- [ ] 13.3 Resolver o registrar como pendientes las preguntas abiertas del design
- [ ] 13.4 Revisar qué deudas de fases anteriores quedan vivas y con qué fecha
