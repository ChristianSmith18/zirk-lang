# zirk-scalars Specification

## Purpose
TBD - created by archiving change fase-3b-scalars-and-text. Update Purpose after archive.
## Requirements
### Requirement: Familia completa de anchos enteros

El sistema de tipos SHALL reconocer `Int8`, `Int16`, `Int32`, `Int64`, `Int128` con signo y `UInt8`, `UInt16`, `UInt32`, `UInt64`, `UInt128` sin signo, con `Int`/`Integer` como alias de `Int32`.

#### Scenario: Literal en un ancho explícito
- **WHEN** una variable se anota `Int8` y se le asigna un literal entero dentro de su rango
- **THEN** el chequeo tiene éxito y el valor se representa en 8 bits con signo

#### Scenario: Literal fuera de rango para su ancho
- **WHEN** un literal excede el rango representable del ancho anotado
- **THEN** se emite un diagnóstico en tiempo de compilación, sin esperar a tiempo de ejecución

### Requirement: Aritmética entera comprobada en cada ancho

Toda operación aritmética sobre un tipo entero, de cualquier ancho y señal, SHALL producir un error controlado en tiempo de ejecución si el resultado no es representable en ese ancho, en vez de envolver silenciosamente.

#### Scenario: Overflow en un ancho angosto
- **WHEN** una suma sobre `Int8` produce un resultado mayor a 127
- **THEN** el programa termina con un error controlado que identifica la operación

#### Scenario: Aritmética sin signo bajo cero
- **WHEN** una resta sobre `UInt32` produciría un resultado negativo
- **THEN** el programa termina con un error controlado, no con un valor envuelto al techo del rango

### Requirement: Conversión entre anchos enteros

El chequeador SHALL permitir el ensanchamiento implícito de un entero a un ancho mayor de la misma señal cuando no sea ambiguo, y SHALL exigir una conversión explícita para angostamiento, cambio de señal, o cuando el contexto no determina un único ancho de destino.

#### Scenario: Ensanchamiento implícito sin ambigüedad
- **WHEN** un `Int8` se pasa donde se espera `Int32`
- **THEN** el chequeo tiene éxito sin una conversión escrita

#### Scenario: Angostamiento sin conversión explícita
- **WHEN** un `Int32` se asigna a una variable `Int8` sin una conversión explícita
- **THEN** se emite un diagnóstico de tipos

#### Scenario: Cambio de señal sin conversión explícita
- **WHEN** un `Int32` se asigna a una variable `UInt32` sin una conversión explícita
- **THEN** se emite un diagnóstico de tipos

### Requirement: Familia `Float` sin `NaN` válido

El sistema de tipos SHALL reconocer `Float16`, `Float32`, `Float64`, `Float128`, con `Float` como alias de `Float64`. Una operación que produciría `NaN` bajo IEEE 754 SHALL ser, en cambio, un error controlado en tiempo de ejecución en el punto donde ocurre. Los infinitos positivo y negativo SHALL ser valores válidos y observables.

#### Scenario: División por cero de punto flotante
- **WHEN** un `Float64` se divide por `0.0`
- **THEN** el resultado es infinito, no un error, si el dividendo no es cero

#### Scenario: Operación indeterminada
- **WHEN** una operación produciría `NaN` bajo la semántica IEEE 754 estándar (por ejemplo, `0.0 / 0.0`)
- **THEN** el programa termina con un error controlado en el punto de esa operación, no propaga un valor `NaN`

#### Scenario: Literal fraccionario sin contexto
- **WHEN** un literal fraccionario aparece sin una anotación o contexto que fije su ancho
- **THEN** su tipo es `Float64`

### Requirement: `Char` como un grapheme Unicode

El sistema de tipos SHALL reconocer `Char` como exactamente un grapheme Unicode extendido, incluso cuando este abarca más de un code point.

#### Scenario: Literal de un carácter simple
- **WHEN** un literal `Char` contiene un único code point ASCII
- **THEN** su valor es ese grapheme

#### Scenario: Literal de un grapheme compuesto
- **WHEN** un literal `Char` contiene un grapheme extendido compuesto por varios code points (por ejemplo, una base con marcas combinantes)
- **THEN** el chequeo tiene éxito y el valor conserva el grapheme completo como una unidad

#### Scenario: Iteración de `String` por grapheme
- **WHEN** un `for ... in` recorre un `String`
- **THEN** cada elemento bound es un `Char`, uno por grapheme, no por byte ni por code point

### Requirement: Conversión contextual profunda

Un constructor explícito de un tipo escalar (`Float(expr)`, `String(expr)`, etc.) SHALL establecer un dominio de conversión para el árbol de operadores compatible que contiene directamente, convirtiendo cada operando antes de que la operación se evalúe. El contexto NO SHALL mutar los operandos originales ni cruzar hacia el cuerpo de una función llamada dentro de la expresión.

#### Scenario: División convertida antes de operar
- **WHEN** se escribe `Float(3 / 4)`
- **THEN** el resultado es `0.75`, no `0` truncado y luego convertido

#### Scenario: El contexto no cruza una llamada de función
- **WHEN** una expresión dentro de un constructor contextual llama a una función que internamente realiza una división entera
- **THEN** esa división interna no adopta el contexto del constructor externo

### Requirement: Operadores bitwise y de shift

El chequeador SHALL aceptar `&`, `|`, `^`, `~`, `<<`, `>>` sobre operandos enteros, de cualquier ancho y señal compatibles entre sí, en los niveles de precedencia que `ZIRK_LANGUAGE_SPEC.md` fija para ellos.

#### Scenario: Operación bitwise entre anchos distintos
- **WHEN** se aplica `&` entre un `Int8` y un `Int32` sin conversión explícita
- **THEN** se emite el mismo diagnóstico de tipos que cualquier otra operación entre anchos incompatibles

#### Scenario: Shift por una cantidad negativa
- **WHEN** el operando derecho de `<<` o `>>` es negativo en tiempo de ejecución
- **THEN** el programa termina con un error controlado

