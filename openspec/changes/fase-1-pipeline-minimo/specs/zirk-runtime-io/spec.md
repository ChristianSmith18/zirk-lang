## ADDED Requirements

### Requirement: Representación opaca de String

El runtime SHALL exponer `String` como un handle opaco cuyo layout es privado, según `docs/decisions/ADR-005-representacion-string.md`.

#### Scenario: Opacidad para el compilador
- **WHEN** el codegen manipula un valor `String`
- **THEN** lo trata como handle opaco
- **AND** NO inspecciona ni asume su representación interna

#### Scenario: Construcción desde un literal
- **WHEN** el código generado materializa un literal de cadena
- **THEN** invoca la función del runtime que construye un `String` a partir de bytes UTF-8 y su longitud

### Requirement: Salida estándar

El runtime SHALL exponer una función `extern "C"` que escriba un `String` en la salida estándar seguido de un salto de línea.

#### Scenario: Impresión de una cadena
- **WHEN** un programa invoca la función de impresión con un `String`
- **THEN** el contenido aparece en la salida estándar seguido de un salto de línea

#### Scenario: Contenido no ASCII
- **WHEN** la cadena contiene caracteres Unicode fuera de ASCII
- **THEN** se escriben correctamente codificados en UTF-8

#### Scenario: Vaciado antes de terminar
- **WHEN** el programa termina
- **THEN** la salida estándar se vacía antes de que el proceso finalice

### Requirement: Estabilidad de los símbolos del runtime

Todo símbolo del runtime destinado al código generado SHALL declararse `extern "C"` sin mangling y con nombre estable.

Son superficie de compatibilidad: cambiarlos rompe binarios ya compilados.

#### Scenario: Símbolos sin mangling
- **WHEN** se inspecciona la biblioteca estática producida
- **THEN** los símbolos destinados al código generado aparecen con su nombre literal

### Requirement: Ausencia de dependencia con la stdlib de Zirk

El runtime de esta fase NO SHALL requerir que exista `std.io` como módulo de Zirk.

`stdout.println` se resuelve como intrínseco del compilador. Es deuda deliberada que se retira en Fase 7.

#### Scenario: Programa sin importaciones
- **WHEN** un programa usa `stdout.println` sin ninguna sentencia `import`
- **THEN** compila y ejecuta correctamente
