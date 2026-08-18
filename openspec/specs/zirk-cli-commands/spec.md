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

### Requirement: Authority-bearing commands validate signed approval incrementally
Before build-time or runtime code executes, relevant CLI commands SHALL compare project name/location, manifest, lockfile, permission, requester, phase, and approval fingerprints. They SHALL take a fast path when unchanged and recompute only affected requester graph segments when changed.

#### Scenario: Dependency requester updated
- **WHEN** an approved dependency receiving filesystem authority changes version or integrity
- **THEN** the CLI stops and requests new approval even when textual permission scope is unchanged

### Requirement: Permission management commands are auditable
The CLI SHALL provide `zirk permissions show`, `diff`, `approve`, `revoke`, and `history`. Interactive approval SHALL show exact scope, phase, call/requester path, and manifest diff. CI SHALL consume a protected explicit policy and fail with a diff when authority widens.

#### Scenario: Noninteractive build lacks approval
- **WHEN** CI encounters a permission fingerprint absent from its protected policy
- **THEN** the command fails without prompting and prints a machine-readable authorization diff

### Requirement: Test selection and reporting are reproducible
The CLI MUST provide `zirk test` unit, E2E, all, file, tag, seed, and bounded-job
selection together with human, JSON, and JUnit reports. Filtering SHALL NOT
change test semantics or grant permissions. Every failure affected by
runner-controlled randomness SHALL report a reproduction seed; that seed SHALL
NOT control cryptographic randomness.

#### Scenario: Seeded test fails
- **WHEN** a test using runner-controlled randomization fails under `--seed 48291`
- **THEN** the report includes `48291` and a reproducible command

#### Scenario: CI requests JUnit output
- **WHEN** `zirk test --report junit` is executed
- **THEN** the runner emits stable interoperable test records with the same secret redaction as human output

### Requirement: Snapshot updates are explicit
The test runner SHALL compare snapshots without rewriting them by default.
Snapshot writes MUST require `--update-snapshots`, show changes, enforce test
filesystem permission, and preserve secret redaction.

#### Scenario: Snapshot differs in a normal test run
- **WHEN** an observed snapshot differs without the update flag
- **THEN** the test fails with a diff
- **AND** the stored snapshot remains unchanged
