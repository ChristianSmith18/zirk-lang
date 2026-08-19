## 1. Auditoría previa (bloquea todo lo demás)

- [ ] 1.1 Listar cada sitio en `zirk-sema`/`zirk-ir`/`zirk-codegen-llvm` que compara contra `Base::Int32`/`IrType::Int32` directamente, y clasificar cada uno: generaliza a "cualquier entero" sin más, o asume 32 bits por una razón real que necesita una decisión explícita (por ejemplo, el ancho de un discriminante de enum)
- [ ] 1.2 Con esa lista, confirmar o recortar el alcance de esta fase antes de escribir ningún ancho nuevo (design.md, riesgo de alcance)

## 2. Léxico

- [ ] 2.1 Literales enteros con sufijo o inferencia de ancho, según lo que la gramática existente ya distinga
- [ ] 2.2 Literales `Float`, con notación científica y default `Float64`
- [ ] 2.3 Literal `Char`, delimitado por comillas simples, capaz de un grapheme extendido de más de un code point
- [ ] 2.4 `{expr}` dentro de un literal de `String`, con `\{`/`\}` como escape
- [ ] 2.5 Tests: un caso válido y uno inválido por cada literal nuevo

## 3. Gramática

- [ ] 3.1 Operadores bitwise y de shift (`&`, `|`, `^`, `~`, `<<`, `>>`), en los niveles de precedencia que el spec fija, distintos de `&&`/`||`
- [ ] 3.2 AST para una expresión interpolada: texto literal más una lista de sub-expresiones, preservando el orden de aparición
- [ ] 3.3 Tests: un caso válido y uno inválido por cada regla nueva

## 4. Tipos — anchos enteros

- [ ] 4.1 Registrar `Int8`/`Int16`/`Int64`/`Int128` y la familia `UInt8`…`UInt128` como tipos del chequeador
- [ ] 4.2 Aritmética comprobada por ancho y señal, reutilizando el mecanismo de `Int32` de la Fase 1 (ver 1.1)
- [ ] 4.3 Ensanchamiento implícito sin ambigüedad; angostamiento, cambio de señal y cualquier conversión con pérdida, siempre explícitos
- [ ] 4.4 Operadores bitwise/shift sobre enteros, con la misma regla de conversión entre anchos que la aritmética ordinaria
- [ ] 4.5 Tests: un caso válido y uno inválido por cada regla nueva

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
- [ ] 8.6 Desazucarar `"{expr}"` a texto literal concatenado con `expr.to_string()`
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
