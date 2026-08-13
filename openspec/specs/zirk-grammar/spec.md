# zirk-grammar

## Purpose

Defines the grammar of the language and the construction of the syntax tree, together with the errors it reports.

The parser decides whether a program is well *formed*, not whether it makes *sense*: that belongs to the type system.

## Requirements

### Requirement: Declaración de funciones

El parser SHALL reconocer declaraciones de función con la forma de `ZIRK_LANGUAGE_SPEC.md` sección 6: `fn` nombre, lista de parámetros tipados, tipo de retorno tras `:`, y cuerpo entre llaves.

#### Scenario: Función sin parámetros
- **WHEN** se parsea `fn main(): Void { }`
- **THEN** se produce una declaración de función llamada `main`, sin parámetros y con retorno `Void`

#### Scenario: Función con parámetros
- **WHEN** se parsea `fn add(a: Int32, b: Int32): Int32 { return a + b; }`
- **THEN** se produce una función con dos parámetros tipados y retorno `Int32`

#### Scenario: Tipo de retorno ausente
- **WHEN** una función se declara sin tipo de retorno
- **THEN** se emite un diagnóstico que señala dónde se esperaba el tipo
- **AND** la ayuda indica que el tipo de retorno es obligatorio en esta fase

### Requirement: Declaración de variables

El parser SHALL reconocer declaraciones con `mut` e `inmut`, con anotación de tipo opcional cuando haya inicializador.

#### Scenario: Variable con tipo explícito
- **WHEN** se parsea `mut count: Int32 = 0;`
- **THEN** se produce una declaración mutable con tipo `Int32` e inicializador

#### Scenario: Variable con tipo inferido
- **WHEN** se parsea `mut count = 0;`
- **THEN** se produce una declaración mutable sin anotación de tipo

#### Scenario: Declaración sin inicializador ni tipo
- **WHEN** se parsea `mut count;`
- **THEN** se emite un diagnóstico indicando que falta el tipo o el inicializador

### Requirement: Expresiones y precedencia

El parser SHALL construir expresiones respetando la precedencia y asociatividad convencionales de los operadores de `ZIRK_LANGUAGE_SPEC.md` sección 4.

De mayor a menor precedencia: unarios (`!`, `-`); multiplicativos (`*`, `/`, `%`); aditivos (`+`, `-`); comparación (`<`, `<=`, `>`, `>=`); igualdad (`==`, `!=`); conjunción (`&&`); disyunción (`||`).

#### Scenario: Precedencia multiplicativa sobre aditiva
- **WHEN** se parsea `1 + 2 * 3`
- **THEN** el árbol representa `1 + (2 * 3)`

#### Scenario: Asociatividad izquierda
- **WHEN** se parsea `10 - 4 - 3`
- **THEN** el árbol representa `(10 - 4) - 3`

#### Scenario: Paréntesis alteran la precedencia
- **WHEN** se parsea `(1 + 2) * 3`
- **THEN** el árbol representa la suma como operando izquierdo del producto

#### Scenario: Conjunción sobre disyunción
- **WHEN** se parsea `a || b && c`
- **THEN** el árbol representa `a || (b && c)`

### Requirement: Sentencia condicional

El parser SHALL reconocer `if` y `else` como sentencia, con cuerpos siempre entre llaves.

En esta fase `if` **no** se admite como expresión, aunque el spec lo permita: es Fase 2.

#### Scenario: Condicional simple
- **WHEN** se parsea `if x > 0 { }`
- **THEN** se produce una sentencia condicional sin rama alternativa

#### Scenario: Condicional con alternativa
- **WHEN** se parsea `if x > 0 { } else { }`
- **THEN** se produce una sentencia condicional con ambas ramas

#### Scenario: Encadenamiento
- **WHEN** se parsea `if a { } else if b { } else { }`
- **THEN** se produce un condicional cuya rama alternativa es otro condicional

#### Scenario: Cuerpo sin llaves
- **WHEN** se parsea `if x > 0 return;`
- **THEN** se emite un diagnóstico indicando que el cuerpo debe ir entre llaves

### Requirement: Llamadas a función y retorno

El parser SHALL reconocer llamadas con argumentos posicionales y la sentencia `return`.

#### Scenario: Llamada con argumentos
- **WHEN** se parsea `add(1, 2)`
- **THEN** se produce una llamada con dos argumentos

#### Scenario: Retorno con valor
- **WHEN** se parsea `return a + b;`
- **THEN** se produce un retorno cuya expresión es la suma

#### Scenario: Retorno sin valor
- **WHEN** se parsea `return;`
- **THEN** se produce un retorno sin expresión

### Requirement: Punto y coma opcional

El parser SHALL admitir la omisión del punto y coma cuando no haya ambigüedad, según `ZIRK_LANGUAGE_SPEC.md` sección 1.

#### Scenario: Sentencias sin punto y coma
- **WHEN** se parsean sentencias separadas por saltos de línea y sin `;`
- **THEN** el árbol resultante es equivalente al de las mismas sentencias con `;`

### Requirement: Construcciones fuera del subset

El parser SHALL emitir un diagnóstico específico ante construcciones que existen en el lenguaje pero no están implementadas, distinguiéndolas de errores de sintaxis.

#### Scenario: Construcción de fase posterior
- **WHEN** se parsea `class`, `for`, `while`, `loop`, `match`, `try`, `task`, `parallel` o `thread`
- **THEN** el diagnóstico SHALL nombrar la construcción
- **AND** SHALL indicar que no está implementada todavía
- **AND** NO SHALL reportarse como token inesperado

#### Scenario: Importación de módulos
- **WHEN** se parsea una sentencia `import`
- **THEN** el diagnóstico indica que los módulos multi-archivo llegan en una fase posterior

### Requirement: Ubicación en todo nodo

Todo nodo del árbol SHALL exponer el span del source que lo originó.

Un nodo sin ubicación no puede producir el diagnóstico que exige `ZIRK_COMPILER_SPEC.md` sección 8.

#### Scenario: Span de un nodo cualquiera
- **WHEN** se inspecciona cualquier nodo del árbol producido
- **THEN** expone la ubicación de inicio y fin en el source
