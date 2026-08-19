## ADDED Requirements

### Requirement: Tipo entero parametrizado por ancho y señal en la IR

La IR SHALL representar cada tipo entero con un único tipo parametrizado por ancho y señal, no con una variante distinta por ancho, de forma que una instrucción aritmética o de conversión existente siga siendo válida para cualquier ancho sin duplicarse.

#### Scenario: Instrucción aritmética independiente del ancho
- **WHEN** se baja una suma entre dos operandos `Int8`
- **THEN** se emite la misma forma de instrucción que para `Int32`, con el ancho como dato del tipo, no una instrucción distinta

### Requirement: Comprobación de overflow por ancho y señal

El lowering SHALL emitir la comprobación de overflow correspondiente al ancho y la señal del tipo entero de la operación, reutilizando el mecanismo que la Fase 1 estableció para `Int32`.

#### Scenario: Overflow comprobado en un ancho angosto
- **WHEN** se baja una operación aritmética sobre `Int8`
- **THEN** la IR incluye la comprobación de overflow para ese ancho, no la de `Int32`

### Requirement: `NaN` como fallo controlado, no como valor propagado

El lowering de una operación de `Float` que puede producir `NaN` bajo la semántica del backend SHALL incluir la comprobación que la convierte en un fallo controlado antes de que el valor se use.

#### Scenario: División de punto flotante potencialmente indeterminada
- **WHEN** se baja `a / b` sobre `Float64` sin que el checker pueda descartar `a == 0.0 && b == 0.0`
- **THEN** la IR incluye la comprobación correspondiente antes de producir el resultado

### Requirement: Desazúcar de la conversión contextual profunda

El lowering SHALL insertar la conversión de cada operando de un árbol de operadores contextual antes de bajar la operación misma, en vez de bajar la operación con su tipo original y convertir el resultado después.

#### Scenario: División bajada con el contexto ya aplicado
- **WHEN** se baja `Float(3 / 4)`
- **THEN** los operandos de la división ya son `Float64` en la IR resultante, no `Int32` convertidos después

### Requirement: Desazúcar de la interpolación de strings

El lowering SHALL desazucarar un literal con expresiones interpoladas en una secuencia de llamadas a `to_string()` concatenadas con el texto literal, en el orden de aparición.

#### Scenario: Interpolación con una expresión
- **WHEN** se baja `"Hola, {nombre}"`
- **THEN** la IR resultante llama a `to_string()` sobre `nombre` y concatena con el texto literal
