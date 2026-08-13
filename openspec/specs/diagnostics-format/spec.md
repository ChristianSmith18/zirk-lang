# diagnostics-format

## Purpose

Define el contrato de los diagnósticos del compilador: severidad, código estable, ubicación, causa y ayuda, según `ZIRK_COMPILER_SPEC.md` sección 8.

El formato se fija desde el día uno a propósito: migrarlo después de que varias capas ya lo usen es mucho más caro que empezar bien.

## Requirements

### Requirement: Estructura mínima de un diagnóstico

Todo diagnóstico SHALL incluir severidad, código estable, ubicación en el source, causa y, cuando exista una reparación clara, una ayuda accionable. Corresponde a `ZIRK_COMPILER_SPEC.md` sección 8.

#### Scenario: Diagnóstico completo
- **WHEN** se construye un diagnóstico de error con ubicación conocida
- **THEN** expone severidad, código, archivo, línea, columna, causa y ayuda

#### Scenario: Ayuda ausente
- **WHEN** no existe una reparación clara para el error
- **THEN** el diagnóstico SHALL emitirse sin ayuda
- **AND** NO SHALL emitirse una ayuda genérica sin valor accionable

### Requirement: Formato de presentación

Un diagnóstico renderizado SHALL seguir el formato de `ZIRK_COMPILER_SPEC.md` sección 8: encabezado con severidad y código, ubicación, fragmento de source con marcador de la posición, y líneas de causa y ayuda.

#### Scenario: Renderizado con fragmento de source
- **WHEN** se renderiza un diagnóstico cuya ubicación tiene source disponible
- **THEN** la salida incluye la línea de source y un marcador bajo la columna señalada

#### Scenario: Renderizado sin source disponible
- **WHEN** el source no está disponible
- **THEN** la salida conserva encabezado, ubicación, causa y ayuda, omitiendo el fragmento

### Requirement: Estabilidad de los códigos de diagnóstico

Cada diagnóstico SHALL tener un código estable. Un código publicado NO SHALL reutilizarse para un error semánticamente distinto.

#### Scenario: Código único por clase de error
- **WHEN** se define un nuevo diagnóstico
- **THEN** recibe un código no usado previamente

### Requirement: Severidades diferenciadas

El sistema SHALL distinguir al menos error y warning. Los warnings NO SHALL alterar la semántica del programa.

#### Scenario: Warnings como errores
- **WHEN** se activa el modo `--warnings-as-errors`
- **THEN** los warnings se elevan a errores y la compilación falla

### Requirement: Salida estructurada

El sistema de diagnósticos SHALL poder emitir en formato legible por humanos y en formato estructurado para herramientas.

#### Scenario: Modo JSON
- **WHEN** se solicita salida estructurada
- **THEN** cada diagnóstico se serializa conservando severidad, código, ubicación, causa y ayuda
