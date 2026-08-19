## ADDED Requirements

### Requirement: Literales de anchos enteros y `Float`

La gramática SHALL reconocer un literal entero como cualquiera de los anchos con o sin signo cuando el contexto lo determina, y un literal fraccionario (con notación científica opcional) como `Float`, ambos con `_` como separador visual.

#### Scenario: Separador visual en un literal ancho
- **WHEN** se escribe `1_000_000`
- **THEN** se lexea como el entero `1000000`

#### Scenario: Notación científica
- **WHEN** se escribe `1e2`
- **THEN** se lexea como un literal `Float` de valor `100.0`

### Requirement: Literal `Char`

La gramática SHALL reconocer un literal `Char` delimitado por comillas simples, capaz de contener un grapheme Unicode extendido de más de un code point.

#### Scenario: Literal de un carácter ASCII
- **WHEN** se escribe `'a'`
- **THEN** se lexea como un literal `Char`

#### Scenario: Delimitador sin cerrar
- **WHEN** un literal `Char` no tiene su comilla de cierre antes del fin de línea
- **THEN** se emite un diagnóstico léxico

### Requirement: Operadores bitwise y de shift

La gramática SHALL reconocer `&`, `|`, `^`, `~`, `<<`, `>>` como operadores binarios (`~` unario), en los niveles de precedencia que `ZIRK_LANGUAGE_SPEC.md` fija para ellos, distintos de los lógicos `&&`/`||`.

#### Scenario: Precedencia distinta de la lógica
- **WHEN** se escribe una expresión que combina `&` con `&&`
- **THEN** se parsea según la precedencia de cada operador, no como si fueran el mismo

### Requirement: Interpolación en literales de `String`

La gramática SHALL reconocer `{expr}` dentro de un literal de `String` como una expresión interpolada, con `\{` como escape para una llave literal.

#### Scenario: Interpolación simple
- **WHEN** se escribe `"Hola, {nombre}"`
- **THEN** se parsea como texto literal más una expresión interpolada `nombre`

#### Scenario: Llave escapada
- **WHEN** se escribe `"\{no interpolado\}"`
- **THEN** se parsea como texto literal con llaves, sin expresión interpolada
