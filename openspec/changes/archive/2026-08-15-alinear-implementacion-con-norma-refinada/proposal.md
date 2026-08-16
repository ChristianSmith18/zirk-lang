## Why

La documentación normativa se refinó después de que las Fases 0–2 ya estuvieran
implementadas: la familia `Decimal` desapareció en favor de `Float`, `Char` pasó
a ser un grafema Unicode, `String` quedó definida como referencia mutable
compartida indexada por grafemas, y aparecieron la familia temporal, la
conversión contextual profunda y `inmut::strict` con análisis de aliases.

El compilador todavía no sabe nada de eso. Su **semántica implementada es
correcta** —división truncada hacia cero, resto con signo del dividendo,
overflow y división por cero como errores controlados, ausencia de truthiness y
de conversiones implícitas—, pero su **puerta de entrada miente**: el lexer no
reconoce literales que el lenguaje tiene, y la tabla de tipos pendientes anuncia
una familia `Decimal` que ya no existe.

Hay un caso peor que un diagnóstico equivocado: `1.5` se tokeniza en silencio
como `1`, `.`, `5`. El lenguaje no tiene forma de decir "todavía no", porque ni
siquiera ve el literal.

Esto se cierra **antes** de la Fase 3 porque la Fase 3 opera justamente sobre
las estructuras desalineadas —su tarea 1.2 manipula la tabla de fases del lexer—
y porque varias características documentadas no tienen fase asignada: sin
asignarlas, o se cuelan en la Fase 3 y la desbordan, o quedan como deuda sin
fecha.

## What Changes

### Léxico: reconocer todo el lenguaje, implementar solo lo de siempre

El lexer ya tiene la disciplina correcta —reconocer construcciones de fases
posteriores para poder decir "llega en Fase N" en vez de "token inesperado"—.
Esta vez se aplica a lo que faltaba:

- Literales `Float` (`1.5`, `6.02e23`, sufijos de ancho), enteros hexadecimales
  y binarios (`0xff`, `0b1010`).
- `**` y `**=`, que hoy se leen como dos multiplicaciones seguidas.
- Literales `Char` (`'a'`, `'👨‍👩‍👧‍👦'`) y literales regex `re'...'`, con el
  diagnóstico de regex sin cerrar apuntando a su apertura.
- Interpolación `"value={value}"` con llaves balanceadas, incluida la de límites
  de rango `0..{number}`.
- Operadores bit a bit y de desplazamiento (`&`, `|`, `^`, `~`, `<<`, `>>`) y
  sus compuestos, presentes en los niveles 7–10 de la tabla de precedencia.
- Literales de duración (`250ms`, `-3s`, `2h`), sin tratar `m` como mes.
- Las palabras clave que faltaban: `do`, `yield`, `interface`, `trait`,
  `strict`, `value`.

Ninguna gana semántica en este cambio. Todas se reconocen y se difieren con su
fase, que es exactamente lo que el lexer ya hace con `class` o `task`.

### Tabla de tipos pendientes: dejar de enseñar un lenguaje que no existe

- **BREAKING (documental):** se retiran `Decimal16`, `Decimal32`, `Decimal64`,
  `Decimal128`, `Dec` y `Decimal`. No son tipos de Zirk. Un `Decimal` exacto
  podría llegar algún día como tipo de biblioteca, y sería otra cosa.
- Se añaden `Float16`, `Float32`, `Float64`, `Float128` y `Float`, los anchos
  enteros que faltaban, y la familia temporal completa, cada uno declarando su
  fase real.
- Se habilitan **ahora** los aliases `Int`, `Integer` y `UInt`. `Int` e
  `Integer` son exactamente `Int32`, que ya está implementado: bloquearlos era
  bloquear un nombre, no una capacidad. `UInt` sigue difiriéndose junto con su
  objetivo `UInt32`.
- Se corrigen atribuciones de fase equivocadas: `default` pertenece a
  `try`/`catch` (Fase 4), no a los decoradores; `|>` no es de la Fase 3.

### Identidad e igualdad de `String`

Se fija la regla observable y se optimiza por debajo:

- `is` compara identidad: dos bindings que aliasan la misma `String` son
  idénticos, dos con el mismo contenido no.
- `==` compara contenido y es indiferente a la normalización Unicode:
  `"hó" == "hó"` es `true` aunque una esté en NFC y la otra en NFD.
- El hash se calcula sobre la forma canónica, para que `Map<String, _>` nunca
  contradiga a `==`.

El handle opaco del ADR-005 no se reemplaza: **es** la identidad que `is`
compara, y es lo que permite que grafemas, normalización y caché vivan enteros
dentro del runtime.

### Formas de sentencia que la norma tiene y el compilador no

La auditoría de fases destapó un segundo grupo: sintaxis definida por la norma
que el parser nunca implementó, y una regla del lenguaje que no estaba escrita
en ninguna fuente.

- **Los paréntesis del header de una estructura de control son opcionales.** Era
  una regla real del lenguaje que ningún documento recogía, y por eso el parser
  terminó **exigiéndolos** en el `for` tradicional: hoy `for (mut i = 0; ...)`
  compila y `for mut i = 0; ...` —la forma que la norma escribe— se rechaza. Se
  escribe la regla en `ZIRK_LANGUAGE_SPEC.md` §5 y se acepta ambas formas en
  todas las estructuras, con la forma sin paréntesis como canónica.
- **`do { ... } while condición;`**, el bucle de post-condición que siempre
  ejecuta su cuerpo una vez.
- **El `if` de efecto que gobierna una sentencia sin llaves**: `if closed return;`.
- **El ternario `condición ? a : b`**, que la norma prefiere para una elección
  corta de valor.
- **`++` y `--` como expresiones.** La Fase 2 los difirió con un motivo escrito
  en el código: "su distinción prefijo/postfijo necesitaría un orden de
  evaluación que ningún documento normativo define". La norma refinada lo define
  —§4 exige preservar la semántica convencional de prefijo y postfijo— así que
  el motivo del diferimiento caducó. Además, `i++` es el paso canónico del `for`
  documentado.

**Ningún programa que hoy compile deja de hacerlo.** Todas estas formas
*amplían* el conjunto de programas válidos: `for (…)` sigue siendo válido, solo
deja de ser la única opción.

### Fase para cada característica documentada

Se introduce en el roadmap la **Fase 3b — escalares, conversiones y texto
completo** (anchos enteros, familia `Float`, `Char` grafémico, conversión
contextual profunda, bitwise y desplazamientos, interpolación). Van juntas
porque dependen entre sí y todas necesitan los contratos de la Fase 3.

`inmut::strict` se asigna a la Fase 4, donde ya vive el análisis que necesita.
La familia temporal y la de colecciones se asignan a la Fase 7, anotadas como
tipos nativos conocidos por el compilador y no como objetos de biblioteca. El
slicing, `Range.step()`/`.reverse()` y regex también van a la Fase 7, que es
donde llega lo que necesitan. Los generadores `fn gen`/`yield` y el operador
`|>` estrenan la **Fase 7b — estilo funcional**, numerada así para no correr las
fases 8 a 12, que existen desde que se escribió el roadmap.

### Explícitamente fuera de alcance

- **Implementar** aritmética `Float`, `Char`, la familia temporal,
  `inmut::strict`, interpolación o bitwise. Este cambio hace que el compilador
  deje de mentir sobre ellos, no los construye. Las formas de sentencia de la
  sección anterior son la excepción deliberada: son gramática pura, sin trabajo
  de tipos, y ya hay que abrir el parser para los paréntesis opcionales.
- **La Fase 3.** Clases, contratos, genéricos y tipos de datos siguen siendo
  suyos y este cambio no toca su alcance.
- **Reescribir la representación de `String`.** Se fija su comportamiento
  observable y se enmienda el ADR; el índice de grafemas se construye cuando lo
  exija la fase que indexa.

## Capabilities

### New Capabilities

- `zirk-feature-phasing`: la disciplina verificable de que toda característica
  documentada tenga una fase dueña, y de que una todavía no implementada
  produzca un diagnóstico que la nombre en vez de un error de sintaxis o de un
  silencio.

### Modified Capabilities

- `zirk-grammar`: paréntesis opcionales en los headers de las estructuras de
  control, `do ... while`, el `if` sin llaves, el ternario y `++`/`--` como
  expresiones.
- `zirk-lexical-syntax`: el vocabulario completo de literales y operadores del
  lenguaje —Float, hexadecimal, binario, `Char`, regex, duración,
  interpolación, potencia, bit a bit y desplazamientos— y las palabras clave que
  faltaban.
- `zirk-type-system`: retirada de la familia `Decimal`, incorporación de la
  familia `Float` y de los tipos temporales como pendientes con fase, aliases
  cortos habilitados, y `for ... in` sobre `String` ligando `Char`.
- `zirk-runtime-io`: identidad observable del handle, igualdad de contenido
  indiferente a la normalización y hash canónico coherente con la igualdad.

## Impact

- `crates/zirk-lexer`: `token.rs` (palabras clave, tokens, atribución de fases)
  y `lib.rs` (reconocedores de números, caracteres, regex e interpolación).
- `crates/zirk-parser`: diferir con su fase las formas léxicas nuevas, y aceptar
  las formas de sentencia que la norma define — paréntesis opcionales,
  `do ... while`, `if` sin llaves, ternario e incremento como expresión.
- `docs/ZIRK_LANGUAGE_SPEC.md` §5: la regla de paréntesis opcionales, que no
  estaba escrita en ninguna fuente.
- `crates/zirk-sema`: `types.rs` (`pending_type`, aliases, resolución por
  nombre) y el elemento de `for ... in` sobre `String`.
- `crates/zirk-runtime`: `string.rs`, igualdad y hash.
- `docs/init/ZIRK_ROADMAP.md`: Fase 3b y asignación de las características
  huérfanas.
- `docs/decisions/`: enmienda del ADR-005 y una decisión nueva sobre identidad,
  igualdad y normalización de `String`.
- `crates/zirk-ir` y `crates/zirk-codegen-llvm`: bajada de `do ... while`, del
  ternario y del incremento como expresión. Es la única parte del cambio que
  llega hasta el binario.
- Ningún programa válido hoy cambia de comportamiento: el conjunto de programas
  válidos solo crece.
