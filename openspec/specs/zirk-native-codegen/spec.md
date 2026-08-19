# zirk-native-codegen

## Purpose

Defines the translation from the IR into LLVM and the production of the linked executable.

It includes the runtime safety guarantees the spec requires: ordinary overflow is a controlled error, and a division by zero never becomes undefined behaviour.
## Requirements
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

### Requirement: Overflow aritmético controlado

Las operaciones aritméticas sobre enteros SHALL detectar el overflow y producir un error controlado en tiempo de ejecución, no envolvimiento silencioso.

Lo exige `ZIRK_LANGUAGE_SPEC.md` sección 3: las variantes wrapping, saturating o checked deben ser operaciones explícitas, que no existen en este subset.

#### Scenario: Suma que desborda
- **WHEN** una suma de `Int32` excede el rango del tipo en tiempo de ejecución
- **THEN** el programa termina con un error de runtime diagnosticado
- **AND** NO produce un resultado envuelto

#### Scenario: División por cero
- **WHEN** se divide por cero en tiempo de ejecución
- **THEN** el programa termina con un error de runtime diagnosticado
- **AND** NO incurre en comportamiento indefinido

### Requirement: Enlace del ejecutable

El pipeline SHALL producir un ejecutable nativo enlazando el objeto generado con el runtime de Zirk.

#### Scenario: Ejecutable producido
- **WHEN** se compila un programa válido del subset
- **THEN** se produce un ejecutable para la plataforma del host
- **AND** el ejecutable enlaza la biblioteca estática del runtime

#### Scenario: Fallo del enlace
- **WHEN** el linker falla
- **THEN** se emite un diagnóstico que incluye la salida del linker
- **AND** la ayuda indica cómo verificar el toolchain

### Requirement: Ciclo de vida del programa generado

El código generado SHALL invocar la inicialización del runtime antes del cuerpo de `main` y su cierre después, según `ZIRK_RUNTIME_SPEC.md` sección 2.

#### Scenario: Orden de invocación
- **WHEN** se inspecciona el entrypoint generado
- **THEN** invoca `zirk_rt_init` antes del cuerpo de `main`
- **AND** invoca `zirk_rt_shutdown` después de que `main` retorna

#### Scenario: Código de salida
- **WHEN** un programa del subset termina normalmente
- **THEN** el proceso termina con código de salida 0

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

El backend SHALL traducir la comprobación explícita de nulidad producida por el lowering de `??` a una comparación contra el valor nulo de la representación del tipo, sin costo adicional para valores que el chequeo de tipos ya probó no nulos.

#### Scenario: Coalescencia traducida
- **WHEN** se traduce `nombre ?? "anónimo"`
- **THEN** el código generado compara el valor de `nombre` contra nulo antes de elegir la rama

#### Scenario: Sin comprobación cuando el tipo no es nulable
- **WHEN** se traduce una operación sobre un valor de tipo no nulable
- **THEN** el código generado no incluye ninguna comparación de nulidad

### Requirement: Layout de objetos

El backend SHALL traducir un tipo con identidad a una estructura cuya cabecera precede a sus campos, y cuyos campos heredados preceden a los propios.

#### Scenario: Prefijo compartido
- **WHEN** se traducen una clase base y una subclase
- **THEN** el prefijo de la estructura de la subclase coincide con la de la base

#### Scenario: Value class inline
- **WHEN** una value class es campo de otra declaración
- **THEN** se traduce sin puntero intermedio

### Requirement: Tablas de métodos

El backend SHALL emitir una tabla de métodos por tipo con métodos virtuales, y una por interfaz implementada.

#### Scenario: Índice estable al heredar
- **WHEN** una subclase hereda un método virtual
- **THEN** ocupa el mismo índice que en la tabla de su base

#### Scenario: Despacho por interfaz
- **WHEN** se llama a un método a través de una interfaz
- **THEN** el código generado busca la tabla de esa interfaz en el descriptor y despacha por ella

### Requirement: Cast comprobado

El backend SHALL traducir un cast comprobable a una comparación del descriptor de tipo del valor contra el esperado, transfiriendo al runtime cuando no coincide.

#### Scenario: Cast que falla
- **WHEN** el descriptor no corresponde al tipo pedido
- **THEN** el programa termina con un error de runtime diagnosticado
- **AND** NO incurre en comportamiento indefinido

#### Scenario: Cast que no necesita comprobación
- **WHEN** el cast asciende en la jerarquía, donde el chequeador ya lo probó
- **THEN** el código generado no incluye comparación alguna

