## MODIFIED Requirements

### Requirement: Traducción de IR a LLVM

El backend SHALL traducir la IR tipada a LLVM IR, preservando la semántica de tipos y el flujo de control, incluyendo grafos de bloques con ciclos.

#### Scenario: Módulo verificable
- **WHEN** se traduce una IR bien formada
- **THEN** el módulo LLVM resultante pasa la verificación de LLVM

#### Scenario: Correspondencia de bloques
- **WHEN** se traduce una función con condicional
- **THEN** los bloques básicos de la IR corresponden a bloques básicos de LLVM

#### Scenario: Correspondencia de un ciclo
- **WHEN** se traduce una función con un bucle
- **THEN** el bloque LLVM del cuerpo del bucle salta de vuelta al bloque de condición
- **AND** el módulo resultante pasa la verificación de LLVM

## ADDED Requirements

### Requirement: Codegen de closures

El backend SHALL traducir una closure a una función LLVM independiente que recibe el entorno como primer argumento implícito, más un valor agregado (puntero a función, puntero a entorno) para su uso como valor de primera clase.

#### Scenario: Llamada directa a una closure
- **WHEN** se traduce una llamada a un valor de closure
- **THEN** el código generado extrae el puntero a función y el puntero a entorno del valor agregado
- **AND** invoca la función con el entorno como primer argumento

#### Scenario: Closure sin capturas
- **WHEN** se traduce una lambda que no captura ninguna variable
- **THEN** el entorno generado no ocupa espacio observable para el programa

### Requirement: Codegen de `match`

El backend SHALL traducir el `match` bajado por la IR a una secuencia de comparaciones y saltos condicionales sobre el discriminante, o a una instrucción de `switch` de LLVM cuando el `match` es exhaustivo sobre un `enum`.

#### Scenario: `match` sobre `enum` exhaustivo
- **WHEN** se traduce un `match` que cubre todos los constructores de un `enum`
- **THEN** el código generado usa `switch` de LLVM sobre el discriminante
- **AND** no incluye una rama por defecto observable en tiempo de ejecución cuando no hay `_`

### Requirement: Comprobación de nulidad

El backend SHALL traducir la comprobación explícita de nulidad producida por el lowering de `?.` y `??` a una comparación contra el valor nulo de la representación del tipo, sin costo adicional para valores que el chequeo de tipos ya probó no nulos.

#### Scenario: Acceso seguro traducido
- **WHEN** se traduce `usuario?.nombre`
- **THEN** el código generado compara el valor de `usuario` contra nulo antes de acceder al miembro

#### Scenario: Sin comprobación cuando el tipo no es nulable
- **WHEN** se traduce un acceso `.` sobre un valor de tipo no nulable
- **THEN** el código generado no incluye ninguna comparación de nulidad
