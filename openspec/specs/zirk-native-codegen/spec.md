# zirk-native-codegen

## Purpose

Defines the translation from the IR into LLVM and the production of the linked executable.

It includes the runtime safety guarantees the spec requires: ordinary overflow is a controlled error, and a division by zero never becomes undefined behaviour.

## Requirements

### Requirement: Traducción de IR a LLVM

El backend SHALL traducir la IR tipada a LLVM IR, preservando la semántica de tipos y el flujo de control.

#### Scenario: Módulo verificable
- **WHEN** se traduce una IR bien formada
- **THEN** el módulo LLVM resultante pasa la verificación de LLVM

#### Scenario: Correspondencia de bloques
- **WHEN** se traduce una función con condicional
- **THEN** los bloques básicos de la IR corresponden a bloques básicos de LLVM

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
