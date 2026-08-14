## ADDED Requirements

### Requirement: Reconocimiento de archivos fuente

El soporte de editor SHALL asociarse a la extensión `.zrk` que define `ZIRK_SPEC_FINAL.md`.

#### Scenario: Apertura de un archivo Zirk
- **WHEN** se abre un archivo con extensión `.zrk`
- **THEN** el editor lo reconoce como lenguaje Zirk

### Requirement: Cobertura completa del léxico

El resaltado SHALL cubrir **todas** las palabras clave que reconoce el lexer, no solo las del subset implementado.

Cubrir el lenguaje completo es deliberado: el editor muestra el lenguaje tal como lo definen las specs, y es el compilador quien indica qué construcción no está disponible todavía y en qué fase llega.

#### Scenario: Palabra clave del subset implementado
- **WHEN** el source contiene `fn`, `mut`, `inmut`, `if`, `else` o `return`
- **THEN** se resaltan como palabras clave

#### Scenario: Palabra clave de una fase posterior
- **WHEN** el source contiene `class`, `match`, `task`, `parallel` u otra construcción todavía no implementada
- **THEN** se resaltan igual que las del subset

#### Scenario: Correspondencia con el lexer
- **WHEN** se comparan las palabras clave del resaltador con las de `zirk-lexer`
- **THEN** toda palabra que el lexer reconoce está cubierta por el resaltador

### Requirement: Distinción por rol

El resaltado SHALL distinguir las categorías léxicas entre sí: control de flujo, modificadores, tipos, literales, operadores y comentarios.

#### Scenario: Categorías diferenciadas
- **WHEN** se resalta un archivo con construcciones de varias categorías
- **THEN** el control de flujo, los modificadores y los tipos reciben scopes distintos

#### Scenario: La instancia actual no es control de flujo
- **WHEN** el source contiene `this`
- **THEN** se resalta como referencia al valor actual, no como palabra de control

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
