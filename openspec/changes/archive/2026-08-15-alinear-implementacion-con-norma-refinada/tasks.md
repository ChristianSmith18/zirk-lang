> **El orden importa aquí, a diferencia de la Fase 3.**
>
> Las decisiones van antes que el código, porque los tokens nuevos declaran
> fases que el roadmap todavía no define. Y el léxico va antes que el
> chequeador, porque un tipo pendiente sin token que lo produzca no se puede
> probar.
>
> Cada caja marcada significa que su regla está cubierta de punta a punta —
> reconocimiento, diferimiento con fase y tests— no que una capa esté terminada.

## 1. Decisiones y roadmap

- [x] 1.1 Enmendar `ADR-005-representacion-string.md`: el handle opaco es la identidad observable que compara `is`, y la igualdad, el hash y la normalización son responsabilidad del runtime (D7)
- [x] 1.2 Escribir la decisión de identidad, igualdad y normalización de `String`: `is` por referente, `==` por contenido canónico, hash sobre forma canónica, y el orden de caminos rápidos de D5
- [x] 1.3 Añadir a `ZIRK_ROADMAP.md` la Fase 3b — escalares, conversiones y texto completo: anchos enteros, familia `Float`, `Char` grafémico, conversión contextual profunda, bitwise y desplazamientos, interpolación
- [x] 1.4 Asignar en el roadmap `inmut::strict` a la Fase 4 y la familia temporal a la Fase 7, anotada como tipos nativos conocidos por el compilador y no como objetos de biblioteca
- [x] 1.5 Verificar que ninguna característica de las fuentes normativas queda sin fase dueña
- [x] 1.6 Asignar slicing, `Range.step()`/`.reverse()` y regex a la Fase 7, y crear la Fase 7b para generadores `fn gen`/`yield` y el operador `|>`
- [x] 1.7 Escribir en `ZIRK_LANGUAGE_SPEC.md` sección 5 la regla de paréntesis opcionales en headers de control, ausente de toda fuente normativa (D10)

## 1b. Gramática — formas de sentencia que la norma define

- [x] 1b.1 Aceptar paréntesis opcionales en el header del `for` tradicional, que hoy los exige, produciendo el mismo árbol con y sin ellos
- [x] 1b.2 Aceptar paréntesis opcionales en `for ... in` y en `match`
- [x] 1b.3 Parsear `do { ... } while condición;` como bucle de post-condición
- [x] 1b.4 Parsear el `if` de efecto que gobierna una sentencia sin llaves, rechazando `else` sobre esa forma (D11)
- [x] 1b.5 Parsear la expresión ternaria `cond ? a : b`, asociativa a la derecha
- [x] 1b.6 Admitir `++` y `--` en posición de expresión con semántica postfija y prefija, exigiendo lugar asignable y mutable, y retirar el diagnóstico cuyo motivo caducó (D12)
- [x] 1b.7 Bajar `do ... while` a los mismos bloques básicos que `while` con el salto inicial invertido, y el ternario a los del `if` expresión
- [x] 1b.8 Bajar el incremento como expresión preservando el orden de evaluación de cada forma
- [x] 1b.9 Tests: cada forma nueva con un caso válido y uno inválido; `for` con y sin paréntesis produce el mismo árbol; `i++` como expresión devuelve el valor previo y `++i` el incrementado

## 2. Léxico — tokens y palabras clave

- [x] 2.1 Añadir los tokens de potencia `**` y `**=`, con coincidencia más larga antes que la multiplicación
- [x] 2.2 Añadir los tokens bit a bit y de desplazamiento `&`, `|`, `^`, `~`, `<<`, `>>` y sus compuestos, sin romper `&&` ni `||`
- [x] 2.3 Añadir las palabras clave `do`, `yield`, `interface` y `trait`. `strict` y `value` quedan **contextuales**, no reservadas: `mut value = 1` y `match r { Ok(value) => ... }` son código válido y frecuente, y reservarlas lo rompería. `inmut::strict` y `value class` se reconocen por posición
- [x] 2.4 Corregir la atribución de fase de `default` a la fase de `try`/`catch`, la de `|>` a la del estilo funcional, y la de `zirk check`, que prometía una Fase 2 ya completada sin él
- [x] 2.5 Declarar la fase de cada token y palabra clave nuevos según el roadmap ya actualizado
- [x] 2.6 Tests: cada token nuevo se produce como tal, `&&` frente a `&` y `**` frente a `*` `*`, y cada palabra clave nueva no se lexa como identificador

## 3. Léxico — literales numéricos

- [x] 3.1 Reconocer enteros hexadecimales `0x` y binarios `0b`, con `_` entre dígitos
- [x] 3.2 Reconocer literales fraccionarios y en notación científica como literales Float distintos de entero-punto-entero
- [x] 3.3 Reconocer el sufijo de ancho en literales Float, conservándolo para la semántica
- [x] 3.4 Reconocer literales de duración con los sufijos `ns`, `us`, `ms`, `s`, `m`, `h`, `d`, `w`, admitiendo signo negativo y sin tratar `m` como mes
- [x] 3.5 Tests: `1.5` produce un literal y no tres tokens; `0xff`, `0b1010`, `6.02e23`, `1e2`, `1.5f32`, `250ms` y `-3s` se reconocen; un sufijo inventado sigue diagnosticándose

## 4. Léxico — texto

- [x] 4.1 Reconocer literales de carácter entre comillas simples, preservando íntegro su contenido Unicode sin validar el grafema
- [x] 4.2 Diagnosticar un literal de carácter sin cerrar señalando su apertura
- [x] 4.3 Reconocer literales regex `re'pattern'` preservando los escapes para el parser de expresiones regulares
- [x] 4.4 Diagnosticar un regex sin cerrar señalando la apertura `re'`
- [x] 4.5 Reconocer la interpolación `{ expression }` en literales de cadena con llaves balanceadas, produciendo partes literales y expresiones como elementos distintos (D3)
- [x] 4.6 Aplicar el mismo balanceo al límite de rango interpolado `0..{number}`
- [x] 4.7 Diagnosticar una interpolación sin cerrar señalando su apertura
- [x] 4.8 Tests: un emoji de familia produce un literal de carácter con todos sus code points; llaves anidadas cierran donde corresponde; cada forma sin cerrar apunta a su apertura

## 5. Diferimiento con fase

- [x] 5.1 Diferir en el chequeador el literal Float con el diagnóstico de su fase, sin reinterpretarlo (D2)
- [x] 5.2 Diferir el literal de carácter, el literal regex, el literal de duración y la cadena interpolada con la fase que corresponde a cada uno
- [x] 5.3 Diferir los operadores de potencia, bit a bit y desplazamiento con su fase
- [x] 5.4 Diferir las palabras clave nuevas con su fase
- [x] 5.5 Tests: cada forma nueva produce un diagnóstico que la nombra e indica su fase, y ninguna produce error de sintaxis genérico ni carácter no reconocido

## 6. Tabla de tipos y aliases

- [x] 6.1 Retirar `Decimal16`, `Decimal32`, `Decimal64`, `Decimal128`, `Dec` y `Decimal` de la tabla de tipos pendientes
- [x] 6.2 Añadir `Float16`, `Float32`, `Float64`, `Float128` y `Float` con la fase de la familia Float
- [x] 6.3 Añadir la familia temporal `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration` y `Period` con su fase
- [x] 6.4 Revisar la fase declarada por cada tipo pendiente restante contra el roadmap actualizado
- [x] 6.5 Resolver `Int` e `Integer` a `Int32` en el mapeo de nombres, de modo que tras resolver sean indistinguibles (D4)
- [x] 6.6 Mantener `UInt` diferido con la fase de `UInt32`
- [x] 6.7 Tests: `Decimal64` es tipo desconocido sin fase; `Float64` e `Instant` declaran su fase; `mut count: Int = 0` y `mut count: Integer = 0` compilan y son indistinguibles de `Int32`; `UInt` declara la fase de `UInt32`

## 7. Iteración de `String`

- [x] 7.1 Cambiar el elemento de `for ... in` sobre `String` a `Char` en la regla, difiriéndolo con el diagnóstico de fase mientras `Char` no exista
- [x] 7.2 Tests: iterar un `String` difiere con la fase de `Char` y no liga un elemento `String`

## 8. Identidad, igualdad y hash de `String`

- [x] 8.1 Añadir al handle el campo interno `is_ascii`, privado al runtime. `normalization`, `grapheme_count` y `hash` **no** se añaden: en esta fase toda `String` es canónica y no hay quien construya la otra alternativa, así que serían campos constantes que documentan una intención en vez de un estado. Llegan con la fase que lee texto que el compilador no normalizó
- [x] 8.2 Implementar la igualdad con el orden de caminos de D5: mismo handle, bytes idénticos, ambos canónicos con bytes distintos, y solo entonces comparación canónica
- [x] 8.3 Emitir los literales de cadena ya en forma canónica desde el compilador, marcados como tales (D6)
- [x] 8.4 Derivar el hash de la forma canónica, de modo que dos cadenas iguales por `==` nunca produzcan hashes distintos
- [x] 8.5 Tests: NFC frente a NFD son iguales por `==`; contenidos distintos no lo son; una cadena ASCII se resuelve por bytes; los hashes de formas equivalentes coinciden; un literal descompuesto en el source llega canónico al runtime

## 9. Cierre

- [x] 9.1 Verificar que la batería de tests existente pasa sin modificación, salvo los tests que fijaban explícitamente el comportamiento corregido
- [x] 9.2 Verificar que ningún programa `.zrk` que hoy compila cambia de comportamiento (D9)
- [x] 9.3 Actualizar los specs principales con `openspec sync` y revisar que la tarea 1.2 de la Fase 3 sigue siendo coherente con la tabla de fases resultante
