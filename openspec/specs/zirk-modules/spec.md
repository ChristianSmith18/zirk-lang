# zirk-modules

## Purpose

Defines how the files of one crate see each other: `share`, `import` and `use`.

Which files make up a crate is decided by walking `import` from the entry file, so a `.zrk` nobody imports is not part of the program. Visibility is binary here — shared or private to its file. The three levels of `ZIRK_LANGUAGE_SPEC.md` section 7 depend on classes and arrive with them, and per-module namespacing belongs with the project system.

## Requirements

### Requirement: Visibilidad binaria entre archivos de un crate

Una declaración de nivel superior SHALL ser visible únicamente dentro del archivo que la define, salvo que esté marcada `share`, en cuyo caso SHALL ser visible desde cualquier otro archivo del mismo crate que la importe.

Esta fase no implementa los tres niveles de `public`/`private`/`protected` de `ZIRK_LANGUAGE_SPEC.md` sección 7 — eso depende de clases y es Fase 3. La visibilidad aquí es binaria: compartida o privada al archivo.

#### Scenario: Declaración privada al archivo
- **WHEN** una función sin `share` se referencia desde otro archivo del mismo crate
- **THEN** se emite un diagnóstico indicando que la declaración no es accesible desde fuera de su archivo

#### Scenario: Declaración compartida
- **WHEN** una función marcada `share` se importa desde otro archivo
- **THEN** la referencia resuelve a esa declaración

### Requirement: Resolución de `import` con rutas locales

`import { nombres } from "ruta"` con una ruta entre comillas SHALL resolver a un archivo del mismo crate, localizado en forma relativa al archivo que importa, sin la extensión `.zrk` en la ruta escrita.

#### Scenario: Ruta relativa resuelta
- **WHEN** se escribe `import { Usuario } from "./dominio/usuario";` en un archivo dado
- **THEN** se resuelve al archivo `dominio/usuario.zrk` relativo a ese archivo

#### Scenario: Archivo inexistente
- **WHEN** la ruta de un `import` no corresponde a ningún archivo del crate
- **THEN** se emite un diagnóstico que nombra la ruta no encontrada
- **AND** señala el `import` que la pidió, no el inicio del archivo

#### Scenario: Nombre no compartido en el archivo de destino
- **WHEN** se importa un nombre que existe en el archivo de destino pero no está marcado `share`
- **THEN** se emite el mismo diagnóstico de visibilidad que una referencia directa

### Requirement: Alias de importación

`import` SHALL admitir renombrar un nombre importado con la sintaxis `nombre -> alias`, y el nombre resuelto SHALL usarse bajo el alias dentro del archivo que importa.

#### Scenario: Importación con alias
- **WHEN** se escribe `import { Rol -> RolDeDominio } from "./dominio/usuario";`
- **THEN** dentro del archivo, `RolDeDominio` resuelve a la declaración `Rol` del archivo importado
- **AND** el nombre `Rol` sin alias no queda disponible en el archivo que importa

### Requirement: Importaciones mutuas

La resolución de módulos SHALL admitir que dos archivos se importen entre sí, y SHALL leer cada archivo del crate una sola vez.

Una versión anterior de este requisito exigía rechazar los ciclos. Se corrigió al implementarlo: `import` trae nombres al scope y nada en esta fase depende del orden en que se leen los archivos —las firmas se recogen antes de chequear cualquier cuerpo—, así que dos archivos que se referencian mutuamente son un programa normal. El peligro real es recorrer el ciclo indefinidamente, y eso lo resuelve leer cada archivo una vez, no rechazar el programa.

#### Scenario: Ciclo directo
- **WHEN** el archivo A importa del archivo B y el archivo B importa del archivo A
- **THEN** el crate compila
- **AND** cada archivo se lee una sola vez

#### Scenario: Rombo de importaciones
- **WHEN** A importa de B y de C, y ambos importan de D
- **THEN** `D` se lee una sola vez

### Requirement: Colisión de nombres compartidos

La resolución de módulos SHALL rechazar dos declaraciones `share` con el mismo nombre dentro del mismo crate, sin desempate implícito por orden de archivo.

#### Scenario: Dos archivos comparten el mismo nombre
- **WHEN** dos archivos distintos del crate declaran `share fn ayuda(): Void {}` con el mismo nombre
- **THEN** se emite un diagnóstico que señala ambas declaraciones

### Requirement: `use` habilita globals sin nombre calificado

`use` SHALL habilitar el acceso sin calificar a los globals de un módulo ya importado, sin traer nuevos nombres al scope que no hayan sido importados primero con `import`.

#### Scenario: `use` sobre un import existente
- **WHEN** un archivo importa `stdout` desde `std.io` y luego escribe `use stdout;`
- **THEN** `println(...)` sin el calificador `stdout.` resuelve a la misma declaración

#### Scenario: `use` sin `import` previo
- **WHEN** se escribe `use` sobre un nombre que el archivo no importó
- **THEN** se emite un diagnóstico indicando que el nombre no está disponible en este archivo

### Requirement: Módulos estándar sin comillas

`import { nombres } from modulo.estandar;` con un nombre sin comillas SHALL resolver contra los módulos que el compilador reconoce como parte de la biblioteca estándar, según `ZIRK_LANGUAGE_SPEC.md` sección 10.

Esta fase solo reconoce `std.io` con `stdout`, `stdin` y `stderr`, ya existentes como intrínseco desde Fase 1. El resto de la biblioteca estándar es Fase 7.

#### Scenario: Importación de `std.io`
- **WHEN** se escribe `import { stdout, stderr } from std.io;`
- **THEN** ambos nombres resuelven a los intrínsecos ya existentes

#### Scenario: Módulo estándar no reconocido
- **WHEN** se importa desde un módulo estándar distinto de `std.io`
- **THEN** se emite un diagnóstico indicando que ese módulo llega en una fase posterior
