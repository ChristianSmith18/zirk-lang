# zirk-runtime-io

## Purpose

Defines the C ABI contract of the runtime for standard output and for the representation of `String`.

The layout of a `String` is private to the runtime, which is what allows adding grapheme indexing later without touching the compiler.
## Requirements
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

### Requirement: Runtime preserves typed failure and cleanup
The runtime SHALL represent implicit safety failures as typed `RuntimeError` exceptions, preserve exact rethrows, lazily materialize structured traces, redact secrets, attach suppressed cleanup failures, and execute managed resource close exactly once on every exit path.

#### Scenario: Exception and close both fail
- **WHEN** a throwable is propagating and resource close reports an error
- **THEN** the throwable remains primary and the close failure appears in its suppressed list

### Requirement: Environment and external I/O enforce effective grants
Runtime I/O, environment, secret, network, process, and shell operations SHALL validate the signed effective policy and normalized dynamic target before performing an external effect. Denial SHALL produce the operation's typed permission error and SHALL NOT prompt or modify project files.

#### Scenario: Redirect leaves network scope
- **WHEN** an authorized HTTP request redirects to an unauthorized host
- **THEN** the redirect is denied before connecting to the new host

### Requirement: Task-aware I/O and blocking isolation
Runtime I/O SHALL suspend tasks without occupying scheduler threads where the platform permits, SHALL support cancellation-safe cleanup, and SHALL route explicitly wrapped legacy blocking work to a separate pool.

#### Scenario: Task waits for file or socket readiness
- **WHEN** an authorized asynchronous I/O operation cannot complete immediately
- **THEN** the task suspends and the scheduler thread remains available for other work

### Requirement: Irreversible effects preserve permission enforcement
External effects issued from an unsafe commit region MUST still satisfy normal project permissions and operating-system validation.

#### Scenario: Unsafe network send lacks permission
- **WHEN** an unsafe commit region attempts a network send outside the approved scope
- **THEN** permission enforcement rejects it before the external effect occurs
