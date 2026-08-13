# zirk-cli-commands

## Purpose

Defines the commands that compile and run a program, and how diagnostics are presented.

## Requirements

### Requirement: Compilación de un archivo

La CLI SHALL exponer un subcomando que compile un archivo `.zrk` y produzca un ejecutable nativo.

#### Scenario: Compilación exitosa
- **WHEN** se compila un archivo válido del subset
- **THEN** se produce un ejecutable
- **AND** la CLI termina con código de salida 0

#### Scenario: Compilación con errores
- **WHEN** el archivo contiene errores
- **THEN** se emiten los diagnósticos correspondientes
- **AND** NO se produce ejecutable
- **AND** la CLI termina con código de salida distinto de 0

#### Scenario: Archivo inexistente
- **WHEN** se indica una ruta que no existe
- **THEN** se emite un diagnóstico que nombra la ruta

### Requirement: Compilar y ejecutar

La CLI SHALL exponer un subcomando que compile y ejecute el programa en un solo paso.

#### Scenario: Ejecución tras compilar
- **WHEN** se ejecuta un programa válido del subset
- **THEN** el programa corre y su salida aparece en la salida estándar

#### Scenario: Propagación del código de salida
- **WHEN** el programa compilado termina con un código de salida
- **THEN** la CLI termina con ese mismo código

### Requirement: Presentación de diagnósticos

La CLI SHALL presentar los diagnósticos en el formato de `ZIRK_COMPILER_SPEC.md` sección 8, y ofrecer salida estructurada para herramientas.

#### Scenario: Formato legible
- **WHEN** se emite un diagnóstico sin solicitar salida estructurada
- **THEN** se imprime con severidad, código, ubicación, fragmento de source, causa y ayuda

#### Scenario: Salida estructurada
- **WHEN** se solicita salida estructurada
- **THEN** los diagnósticos se emiten en formato legible por máquina

#### Scenario: Los diagnósticos van a la salida de error
- **WHEN** se emiten diagnósticos
- **THEN** se escriben en la salida de error estándar
- **AND** NO se mezclan con la salida del programa compilado

### Requirement: Alcance de un solo archivo

La CLI de esta fase SHALL operar sobre un único archivo, sin manifiesto de proyecto.

#### Scenario: Ausencia de manifiesto
- **WHEN** se compila un archivo sin que exista `init.zrk`
- **THEN** la compilación procede normalmente

#### Scenario: Múltiples archivos
- **WHEN** se indican varios archivos fuente
- **THEN** se emite un diagnóstico indicando que los proyectos multi-archivo llegan en una fase posterior
