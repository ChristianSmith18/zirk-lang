## MODIFIED Requirements

### Requirement: Reconocimiento de archivos fuente
El soporte de editor SHALL asociarse a la extensión `.zrk` y a los archivos de manifiesto `init.zrk`.

#### Scenario: Apertura de un archivo Zirk
- **WHEN** se abre un archivo con extensión `.zrk`
- **THEN** el editor lo reconoce como lenguaje Zirk

#### Scenario: Apertura del manifiesto del proyecto
- **WHEN** se abre el archivo `init.zrk`
- **THEN** el editor lo reconoce como lenguaje Zirk

### Requirement: Cobertura completa del léxico
El resaltado SHALL cubrir **todas** las palabras clave que reconoce el lexer de la Fase 3, no solo las del subset implementado. Esto incluye palabras clave de concurrencia estructurada, emparejamiento de patrones con recursos, modificadores de decoración y configuración de manifiesto.

#### Scenario: Palabra clave del subset implementado
- **WHEN** el source contiene `fn`, `mut`, `inmut`, `if`, `else` o `return`
- **THEN** se resaltan como palabras clave

#### Scenario: Palabra clave de concurrencia y match con recursos
- **WHEN** el source contiene `task`, `await`, `select`, `parallel`, `thread`, `match`, `with`, `do`, `yield`, `throw`, `throws` o `commit`
- **THEN** se resaltan como palabras clave de control o almacenamiento

#### Scenario: Palabra clave del manifiesto init.zrk
- **WHEN** el source contiene `project`, `build_targets`, `globals`, `permissions`, `requires` o `during`
- **THEN** se resaltan como palabras clave o bloques estructurales

### Requirement: Distinción por rol
El resaltado SHALL distinguir las categorías léxicas entre sí: control de flujo, modificadores, tipos (incluyendo flotantes y tipos temporales), literales (incluyendo caracteres Unicode y expresiones regulares), operadores (incluyendo `is` y el pipe `|>`), marcadores de formato en cadenas, y comentarios.

#### Scenario: Marcador de formato en cadena
- **WHEN** el source contiene una cadena con `:variable|modificador`
- **THEN** se distingue el dos puntos, el nombre del placeholder y el modificador de formato del texto normal de la cadena

#### Scenario: Importaciones de librerías nativas y de usuario
- **WHEN** el source contiene `import { stdin } from std.io;` y `import { User } from "./user";`
- **THEN** `std.io` se resalta como un espacio de nombres nativo
- **AND** `from` se resalta como control
- **AND** `"./user"` se resalta como texto literal

#### Scenario: Argumentos nombrados y abreviación
- **WHEN** el source contiene `host: "localhost"` o el shorthand `timeout:`
- **THEN** los identificadores antes del dos puntos se resaltan como argumentos de llamada
