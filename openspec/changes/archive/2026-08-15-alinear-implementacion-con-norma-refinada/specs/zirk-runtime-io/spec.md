## MODIFIED Requirements

### Requirement: Representación opaca de String

El runtime SHALL exponer `String` como un handle opaco cuyo layout es privado,
según `docs/decisions/ADR-005-representacion-string.md`.

El handle SHALL ser además la identidad observable del `String`: es lo que `is`
compara. Esto es lo que permite que la indexación por grafemas, la normalización
y cualquier caché interna vivan enteras dentro del runtime, sin que el
compilador tenga que conocerlas.

#### Scenario: Opacidad para el compilador
- **WHEN** el codegen manipula un valor `String`
- **THEN** lo trata como handle opaco
- **AND** NO inspecciona ni asume su representación interna

#### Scenario: Construcción desde un literal
- **WHEN** el código generado materializa un literal de cadena
- **THEN** invoca la función del runtime que construye un `String` a partir de bytes UTF-8 y su longitud

#### Scenario: El handle es la identidad
- **WHEN** dos valores `String` provienen del mismo handle
- **THEN** son idénticos para el operador `is`
- **AND** dos handles distintos NO son idénticos aunque su contenido coincida

## ADDED Requirements

### Requirement: Igualdad de contenido indiferente a la normalización

La función de igualdad del runtime SHALL comparar el contenido de dos `String`
tratando como iguales dos secuencias canónicamente equivalentes, aunque sus
bytes difieran.

La comparación SHALL resolverse sin asignar memoria en el caso frecuente: mismo
handle, o bytes idénticos, deciden el resultado de inmediato.

#### Scenario: Formas de normalización distintas
- **WHEN** se comparan una cadena en NFC y otra en NFD con el mismo contenido percibido
- **THEN** la igualdad devuelve verdadero

#### Scenario: Contenido distinto
- **WHEN** se comparan dos cadenas cuyo contenido percibido difiere
- **THEN** la igualdad devuelve falso

#### Scenario: Camino rápido
- **WHEN** dos handles coinciden, o sus bytes son idénticos
- **THEN** el resultado se decide sin normalizar ni asignar memoria

### Requirement: Literales normalizados en compilación

El compilador SHALL emitir los literales de cadena en forma canónica, de modo
que la comparación entre literales se resuelva por comparación de bytes.

Normalizar una vez en compilación es lo que mantiene barata la regla de igualdad
en ejecución.

#### Scenario: Literal en forma descompuesta en el source
- **WHEN** un literal de cadena aparece en el source en forma descompuesta
- **THEN** el runtime lo recibe ya en forma canónica

#### Scenario: Comparación entre literales
- **WHEN** se comparan dos literales de igual contenido percibido
- **THEN** la comparación se resuelve por bytes, sin normalizar en ejecución

### Requirement: Hash coherente con la igualdad

Cuando el runtime exponga el hash de un `String`, este SHALL derivarse de su
forma canónica.

Dos cadenas iguales según la función de igualdad SHALL producir siempre el mismo
hash.

#### Scenario: Hash de formas equivalentes
- **WHEN** se calcula el hash de una cadena en NFC y el de su equivalente en NFD
- **THEN** ambos hashes coinciden
