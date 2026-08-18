# editor-tooling

## Purpose

Define el soporte de editor para escribir Zirk: reconocimiento de archivos `.zrk`, resaltado de sintaxis y configuración del lenguaje.

El resaltador cubre el lenguaje completo que definen las specs, no solo el subset que el compilador implementa. La división es deliberada: **el editor muestra el lenguaje; el compilador dice qué está disponible.**

No incluye servidor de lenguaje —diagnósticos en vivo, autocompletado, navegación—, que requieren el frontend incremental y son de la Fase 9.

## Requirements

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

Cubrir el lenguaje completo es deliberado: el editor muestra el lenguaje tal como lo definen las specs, y es el compilador quien indica qué construcción no está disponible todavía y en qué fase llega.

#### Scenario: Palabra clave del subset implementado
- **WHEN** el source contiene `fn`, `mut`, `inmut`, `if`, `else` o `return`
- **THEN** se resaltan como palabras clave

#### Scenario: Palabra clave de concurrencia y match con recursos
- **WHEN** el source contiene `task`, `await`, `select`, `parallel`, `thread`, `match`, `with`, `do`, `yield`, `throw`, `throws` o `commit`
- **THEN** se resaltan como palabras clave de control o almacenamiento

#### Scenario: Palabra clave del manifiesto init.zrk
- **WHEN** el source contiene `project`, `build_targets`, `globals`, `permissions`, `requires` o `during`
- **THEN** se resaltan como palabras clave o bloques estructurales

#### Scenario: Correspondencia con el lexer
- **WHEN** se comparan las palabras clave del resaltador con las de `zirk-lexer`
- **THEN** toda palabra que el lexer reconoce está cubierta por el resaltador

### Requirement: Distinción por rol

El resaltado SHALL distinguir las categorías léxicas entre sí: control de flujo, modificadores, tipos (incluyendo flotantes y tipos temporales), literales (incluyendo caracteres Unicode y expresiones regulares), operadores (incluyendo `is` y el pipe `|>`), marcadores de formato en cadenas, y comentarios.

#### Scenario: Categorías diferenciadas
- **WHEN** se resalta un archivo con construcciones de varias categorías
- **THEN** el control de flujo, los modificadores y los tipos reciben scopes distintos

#### Scenario: La instancia actual no es control de flujo
- **WHEN** el source contiene `this`
- **THEN** se resalta como referencia al valor actual, no como palabra de control

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

### Requirement: Configuración del lenguaje

El editor SHALL conocer los delimitadores de comentario y los pares de apertura y cierre del lenguaje.

#### Scenario: Comentar una selección
- **WHEN** se usa el comando de comentar
- **THEN** se insertan los delimitadores `//` o `/* */` de `ZIRK_LANGUAGE_SPEC.md` sección 1

#### Scenario: Cierre automático
- **WHEN** se escribe `{`, `(`, `[` o una comilla
- **THEN** el editor ofrece el cierre correspondiente

### Requirement: El formateador de la extensión es provisional

Mientras `zirk format` no exista, el soporte de editor MAY ofrecer un formateador propio, que NO SHALL considerarse la definición del estilo de Zirk.

`ZIRK_COMPILER_SPEC.md` sección 10 define el formateador oficial como canónico, idempotente y sin configuración que fragmente el estilo. Un formateador de editor que respeta la configuración del usuario no cumple ese contrato.

#### Scenario: Naturaleza provisional documentada
- **WHEN** se consulta la documentación de la extensión
- **THEN** indica que su formateador es provisional y que el canónico llega con `zirk format`

#### Scenario: Llegada del formateador canónico
- **WHEN** `zirk format` esté disponible
- **THEN** la extensión SHALL delegar en él y retirar su formateador propio

### Requirement: Alcance sin servidor de lenguaje

Esta capacidad NO SHALL incluir diagnósticos dentro del editor, autocompletado, navegación ni renombrado.

Todo eso requiere el LSP sobre un frontend incremental, que `ZIRK_COMPILER_SPEC.md` sección 10 define y el roadmap ubica en la Fase 9.

#### Scenario: Diagnósticos durante la edición
- **WHEN** se escribe código con un error de tipos
- **THEN** el editor no lo señala por sí solo
- **AND** el error aparece al compilar con `zirk build` o `zirk run`
