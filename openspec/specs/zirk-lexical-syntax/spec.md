# zirk-lexical-syntax

## Purpose

Defines the lexicon of Zirk: tokens, literals, comments, locations and lexical errors.

The keywords of the whole language are recognized, not only those of the implemented subset, so a construct from a later phase can be told apart from a syntax error.
## Requirements
### Requirement: Tokenización del subset

El lexer SHALL convertir texto fuente `.zrk` en una secuencia de tokens, cada uno con su ubicación en el source.

Los tokens del subset son: identificadores, palabras clave, literales enteros, literales de cadena, literales booleanos, operadores, delimitadores y fin de archivo.

#### Scenario: Programa mínimo
- **WHEN** se tokeniza `fn main(): Void { }`
- **THEN** se produce la secuencia: palabra clave `fn`, identificador `main`, `(`, `)`, `:`, identificador de tipo `Void`, `{`, `}`, fin de archivo

#### Scenario: Ubicación de cada token
- **WHEN** se tokeniza cualquier entrada
- **THEN** cada token expone archivo, línea y columna de inicio, 1-based
- **AND** la columna cuenta caracteres Unicode, no bytes

### Requirement: Literales enteros

El lexer SHALL reconocer literales enteros decimales, admitiendo `_` como separador según `ZIRK_LANGUAGE_SPEC.md` sección 3.

#### Scenario: Entero simple
- **WHEN** se tokeniza `42`
- **THEN** se produce un literal entero de valor 42

#### Scenario: Separador de millares
- **WHEN** se tokeniza `1_000_000`
- **THEN** se produce un literal entero de valor 1000000

#### Scenario: Separador en posición inválida
- **WHEN** se tokeniza `_1000` o `1000_`
- **THEN** se emite un diagnóstico de error con código estable, causa y ayuda

### Requirement: Literales de cadena

El lexer SHALL reconocer literales de cadena delimitados por comillas dobles, con secuencias de escape.

#### Scenario: Cadena simple
- **WHEN** se tokeniza `"Hola"`
- **THEN** se produce un literal de cadena con contenido `Hola`

#### Scenario: Secuencias de escape
- **WHEN** una cadena contiene `\n`, `\t`, `\"` o `\\`
- **THEN** el literal las representa como salto de línea, tabulación, comilla y barra invertida

#### Scenario: Cadena sin cerrar
- **WHEN** una cadena no se cierra antes del fin de línea o de archivo
- **THEN** se emite un diagnóstico que señala la apertura de la cadena
- **AND** la ayuda indica que falta la comilla de cierre

#### Scenario: Escape desconocido
- **WHEN** una cadena contiene una secuencia de escape no reconocida
- **THEN** se emite un diagnóstico que señala la secuencia

### Requirement: Literales booleanos

El lexer SHALL reconocer `true` y `false` como literales booleanos, no como identificadores.

#### Scenario: Valores booleanos
- **WHEN** se tokeniza `true` o `false`
- **THEN** se produce un literal booleano

### Requirement: Comentarios

El lexer SHALL reconocer comentarios de línea `//` y de bloque `/* ... */`, descartándolos de la secuencia de tokens.

#### Scenario: Comentario de línea
- **WHEN** una línea contiene `// texto`
- **THEN** el contenido desde `//` hasta el fin de línea no produce tokens

#### Scenario: Comentario de bloque
- **WHEN** el source contiene `/* texto */`
- **THEN** el contenido delimitado no produce tokens

#### Scenario: Comentario de bloque sin cerrar
- **WHEN** un comentario de bloque no se cierra antes del fin de archivo
- **THEN** se emite un diagnóstico que señala la apertura

### Requirement: Palabras reservadas del lenguaje completo

El lexer SHALL reconocer como palabras clave las del lenguaje completo, no solo las del subset implementado.

Reconocerlas permite que el parser distinga una construcción no implementada de un error de sintaxis, y que el diagnóstico sea comprensible.

#### Scenario: Palabra clave fuera del subset
- **WHEN** se tokeniza `class`, `for`, `match`, `task` u otra palabra clave del lenguaje completo
- **THEN** se produce el token de palabra clave correspondiente
- **AND** NO se produce un identificador

### Requirement: Sensibilidad a mayúsculas

El lexer SHALL distinguir mayúsculas de minúsculas, según `ZIRK_LANGUAGE_SPEC.md` sección 1.

#### Scenario: Identificador que difiere solo en capitalización
- **WHEN** se tokenizan `total` y `Total`
- **THEN** se producen dos identificadores distintos

### Requirement: Carácter no reconocido

El lexer SHALL emitir un diagnóstico ante cualquier carácter que no pertenezca al léxico, en vez de descartarlo silenciosamente.

#### Scenario: Carácter inválido
- **WHEN** el source contiene un carácter que no inicia ningún token válido
- **THEN** se emite un diagnóstico con la ubicación exacta del carácter

### Requirement: Confirmed authorial tokens and literals
The lexer SHALL recognize `**` and `**=`, the `do`, `gen`, and `yield` words, the inclusive range token `..=`, and regex literals delimited as `re'pattern'`. Regex scanning SHALL preserve escapes for the regex parser and SHALL diagnose an unterminated literal at its opening delimiter.

#### Scenario: Exponentiation tokens
- **WHEN** source contains `value ** 2` or `value **= 2`
- **THEN** the lexer emits exponentiation and compound-exponentiation tokens rather than two multiplication tokens

#### Scenario: Regex literal
- **WHEN** source contains `re'^[0-9]+$'`
- **THEN** the lexer emits one typed regex literal with the pattern text

#### Scenario: Unterminated regex
- **WHEN** a regex literal reaches end of line or file without its closing quote
- **THEN** a lexical diagnostic points to the opening `re'`

### Requirement: Range interpolation tokens
The lexer SHALL preserve `{ expression }` interpolation embedded in an inline range bound such as `0..{number}` using the same balanced interpolation rules as other interpolated expressions.

#### Scenario: Computed range bound
- **WHEN** source contains `0..{number}.step(1)`
- **THEN** the token stream retains the interpolated bound as an expression inside the range

### Requirement: Float and temporal literal vocabulary
The lexer SHALL recognize `Float*` type names and duration suffixes `ns`, `us`, `ms`, `s`, `m`, `h`, `d`, and `w` without treating `m` as a calendar month. Calendar months and years SHALL be expressed through `Period` constructors or ISO period strings.

#### Scenario: Duration suffixes
- **WHEN** source contains `10ns`, `500ms`, `2h`, or `3d`
- **THEN** each is emitted as a duration literal carrying its unit

#### Scenario: Period month is not a duration literal
- **WHEN** a developer needs one calendar month
- **THEN** the documented source form is `Period.months(1)` rather than an ambiguous duration suffix

### Requirement: Grapheme character literal
The lexer SHALL preserve the complete Unicode content of a quoted character literal for grapheme validation and SHALL NOT equate one `Char` with one byte or one code point.

#### Scenario: Family emoji character
- **WHEN** a quoted character literal contains one family emoji grapheme
- **THEN** the lexer emits one character literal for semantic grapheme validation
