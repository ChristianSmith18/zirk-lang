## MODIFIED Requirements

### Requirement: Literales enteros

El lexer SHALL reconocer literales enteros en base decimal, hexadecimal (`0x`) y
binaria (`0b`), admitiendo `_` como separador entre dígitos, según
`ZIRK_LANGUAGE_SPEC.md` sección 3 y `docs/handbook/11-reference/04-literals.md`.

#### Scenario: Entero simple
- **WHEN** se tokeniza `42`
- **THEN** se produce un literal entero de valor 42

#### Scenario: Separador de millares
- **WHEN** se tokeniza `1_000_000`
- **THEN** se produce un literal entero de valor 1000000

#### Scenario: Separador en posición inválida
- **WHEN** se tokeniza `_1000` o `1000_`
- **THEN** se emite un diagnóstico de error con código estable, causa y ayuda

#### Scenario: Base hexadecimal y binaria
- **WHEN** se tokeniza `0xff` o `0b1010`
- **THEN** se produce un literal entero de valor 255 y 10 respectivamente
- **AND** NO se emite un diagnóstico de sufijo inválido

## ADDED Requirements

### Requirement: Literales fraccionarios y notación científica

El lexer SHALL reconocer literales fraccionarios y en notación científica como
literales `Float`, distintos de un entero seguido de un acceso a miembro.

Sin esta regla, `1.5` se tokeniza como `1`, `.` y `5`, que es la peor forma de
fallar: el lenguaje no puede decir "todavía no" sobre algo que ni siquiera ve.

#### Scenario: Literal fraccionario
- **WHEN** se tokeniza `1.5`
- **THEN** se produce un único literal Float
- **AND** NO se produce la secuencia entero, punto, entero

#### Scenario: Notación científica
- **WHEN** se tokeniza `6.02e23` o `1e2`
- **THEN** se produce un único literal Float con su exponente

#### Scenario: Sufijo de ancho
- **WHEN** se tokeniza `1.5f32`
- **THEN** el literal conserva el ancho solicitado para el chequeo semántico

#### Scenario: Float diferido a su fase
- **WHEN** un literal Float aparece en un programa de una fase que no implementa la familia `Float`
- **THEN** el diagnóstico nombra el literal e indica la fase en que llega

### Requirement: Tokens de potencia

El lexer SHALL reconocer `**` y `**=` como tokens propios, aplicando la regla de
coincidencia más larga antes que la multiplicación.

#### Scenario: Potencia
- **WHEN** se tokeniza `value ** 2` o `value **= 2`
- **THEN** se producen los tokens de potencia y de potencia compuesta
- **AND** NO se producen dos tokens de multiplicación consecutivos

### Requirement: Operadores bit a bit y de desplazamiento

El lexer SHALL reconocer `&`, `|`, `^`, `~`, `<<`, `>>` y sus formas compuestas
de asignación, presentes en los niveles 7 a 10 y 18 de
`docs/handbook/11-reference/02-operators-and-precedence.md`.

#### Scenario: Operador bit a bit
- **WHEN** se tokeniza `flags & mask` o `value << 2`
- **THEN** se produce el token del operador correspondiente
- **AND** NO se emite un diagnóstico de carácter no reconocido

#### Scenario: Conjunción lógica frente a bit a bit
- **WHEN** se tokeniza `a && b` y `a & b`
- **THEN** se producen dos tokens distintos, por coincidencia más larga

### Requirement: Literales de carácter

El lexer SHALL reconocer literales de carácter delimitados por comillas simples,
preservando íntegro su contenido Unicode para la validación de grafema que hace
el análisis semántico.

El lexer NO SHALL decidir si el contenido es exactamente un grafema: esa
comprobación pertenece a la semántica de `Char`.

#### Scenario: Carácter ASCII
- **WHEN** se tokeniza `'a'`
- **THEN** se produce un literal de carácter con ese contenido

#### Scenario: Grafema compuesto
- **WHEN** un literal de carácter contiene un emoji de familia formado por varios code points
- **THEN** se produce un único literal de carácter que conserva todos sus code points

#### Scenario: Literal de carácter sin cerrar
- **WHEN** un literal de carácter alcanza el fin de línea o de archivo sin su comilla de cierre
- **THEN** se emite un diagnóstico que señala su apertura

### Requirement: Literales regex

El lexer SHALL reconocer literales regex delimitados como `re'pattern'`,
preservando los escapes para el parser de expresiones regulares.

#### Scenario: Literal regex
- **WHEN** el source contiene `re'^[0-9]+$'`
- **THEN** se produce un único literal regex con el texto del patrón

#### Scenario: Regex sin cerrar
- **WHEN** un literal regex alcanza el fin de línea o de archivo sin su comilla de cierre
- **THEN** se emite un diagnóstico que señala la apertura `re'`

### Requirement: Interpolación en literales de cadena

El lexer SHALL reconocer la interpolación `{ expression }` dentro de un literal
de cadena, con llaves balanceadas, produciendo las partes literales y las
expresiones incrustadas como elementos distintos del literal.

Las mismas reglas de balanceo SHALL aplicarse a un límite de rango interpolado
como `0..{number}`.

#### Scenario: Cadena interpolada
- **WHEN** se tokeniza `"value={value}"`
- **THEN** el literal conserva la parte textual y la expresión incrustada por separado
- **AND** las llaves NO forman parte del texto

#### Scenario: Llaves anidadas
- **WHEN** una interpolación contiene a su vez llaves balanceadas
- **THEN** la interpolación se cierra en su llave correspondiente y no en la primera

#### Scenario: Interpolación sin cerrar
- **WHEN** una interpolación no se cierra antes del fin del literal
- **THEN** se emite un diagnóstico que señala su apertura

#### Scenario: Límite de rango interpolado
- **WHEN** el source contiene `0..{number}.step(1)`
- **THEN** el flujo de tokens conserva el límite interpolado como expresión dentro del rango

### Requirement: Literales de duración

El lexer SHALL reconocer literales de duración formados por un número y uno de
los sufijos `ns`, `us`, `ms`, `s`, `m`, `h`, `d` o `w`, admitiendo signo
negativo, sin tratar `m` como mes calendario.

#### Scenario: Sufijos de duración
- **WHEN** el source contiene `10ns`, `500ms`, `2h`, `3d` o `-3s`
- **THEN** cada uno se emite como literal de duración con su unidad

#### Scenario: El mes calendario no es una duración
- **WHEN** un desarrollador necesita un mes de calendario
- **THEN** la forma documentada es un constructor de `Period`, no un sufijo de duración

### Requirement: Palabras clave restantes del lenguaje completo

El lexer SHALL reconocer también `do`, `yield`, `interface` y `trait` como
palabras clave del lenguaje completo.

Reconocerlas es lo que permite que `interface Usuario { }` produzca "no
implementado todavía" en vez de un error de sintaxis sobre un identificador.

`strict` y `value` NO SHALL ser palabras reservadas. Ambas aparecen en el
lenguaje únicamente en una posición fija —`inmut::strict` y `value class`— y
reservarlas invalidaría `mut value = 1;` y `match r { Ok(value) => ... }`, que
son Zirk corriente y aparecen en los propios ejemplos del spec. Se reconocen por
posición.

#### Scenario: Contrato de una fase posterior
- **WHEN** se tokeniza `interface` o `trait`
- **THEN** se produce el token de palabra clave correspondiente
- **AND** NO se produce un identificador

#### Scenario: Bucle de post-condición
- **WHEN** se tokeniza `do { } while pending;`
- **THEN** `do` y `while` se producen como palabras clave

#### Scenario: Palabra contextual como identificador
- **WHEN** se tokeniza `mut value = 1;` o `mut strict = true;`
- **THEN** `value` y `strict` se producen como identificadores

### Requirement: Atribución de fase correcta por token

Cada palabra clave y cada operador fuera del subset implementado SHALL declarar
la fase que el roadmap le asigna.

#### Scenario: Palabra clave de manejo de errores
- **WHEN** se consulta la fase de `default`
- **THEN** declara la fase de `try`/`catch`, no la de los decoradores

#### Scenario: Operador de tubería
- **WHEN** se consulta la fase de `|>`
- **THEN** declara la fase del estilo funcional, no la de objetos

### Requirement: Token de rango inclusivo

El lexer SHALL reconocer `..=` como token propio, distinto de `..` y de `.`, aplicando coincidencia más larga.

#### Scenario: Rango inclusivo
- **WHEN** se tokeniza `0..=10`
- **THEN** se producen los tokens de rango inclusivo, no `..` seguido de `=`

## REMOVED Requirements

### Requirement: Confirmed authorial tokens and literals

**Reason**: Declaraba la intención —`**`, `**=`, `do`/`gen`/`yield`, `..=` y literales regex— sin los escenarios que la vuelven verificable. Los requisitos detallados de este cambio la cubren entera y con casos comprobables, y mantener la regla en dos sitios garantiza que se separen.

**Migration**: `**` y `**=` pasan a *Tokens de potencia*; los literales regex y su diagnóstico de literal sin cerrar, a *Literales regex*; `do`, `gen` y `yield`, a *Palabras clave restantes del lenguaje completo*; `..=`, a *Token de rango inclusivo*.

### Requirement: Range interpolation tokens

**Reason**: El balanceo de llaves de un límite de rango interpolado es el mismo mecanismo que el de una cadena interpolada, y describirlo aparte invitaba a que divergieran.

**Migration**: Cubierto por el escenario *Límite de rango interpolado* de *Interpolación en literales de cadena*.

### Requirement: Float and temporal literal vocabulary

**Reason**: Mezclaba dos reglas de capas distintas: los sufijos de duración, que son léxicos, y el reconocimiento de los nombres de tipo `Float*`, que es del sistema de tipos.

**Migration**: Los sufijos de duración pasan a *Literales de duración*, incluido que `m` son minutos y nunca meses. El reconocimiento de los nombres `Float*` vive en *Familia `Float` y tipos temporales reconocidos como pendientes*, de `zirk-type-system`.

### Requirement: Grapheme character literal

**Reason**: Duplicaba, del lado léxico, la regla de grafema que `zirk-type-system` ya define para `Char`.

**Migration**: La preservación íntegra del contenido Unicode pasa a *Literales de carácter*, con el emoji de familia como escenario. Qué cuenta como un grafema sigue en *Grapheme Char*, de `zirk-type-system`.
