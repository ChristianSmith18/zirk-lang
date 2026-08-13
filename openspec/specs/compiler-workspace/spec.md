# compiler-workspace

## Purpose

Define la estructura de crates del compilador, el límite de responsabilidad de cada uno y la regla de dependencias entre etapas del pipeline.

La estructura refleja directamente el pipeline de `ZIRK_COMPILER_SPEC.md` sección 2, de modo que cada etapa se pueda testear de forma aislada y el backend quede contenido en un solo crate.

## Requirements

### Requirement: Estructura de crates por etapa del pipeline

El compilador SHALL organizarse como un workspace de Cargo con un crate por etapa del pipeline de `ZIRK_COMPILER_SPEC.md` sección 2, más un crate de diagnósticos y un crate de runtime.

Los crates son: `zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-diagnostics`, `zirk-cli` y `zirk-runtime`.

#### Scenario: Workspace compila
- **WHEN** se ejecuta `cargo build` en la raíz del repositorio
- **THEN** los nueve crates compilan sin errores
- **AND** `cargo clippy` no reporta advertencias

#### Scenario: Responsabilidad declarada
- **WHEN** se inspecciona el `lib.rs` de cualquier crate del workspace
- **THEN** contiene documentación de módulo que declara su responsabilidad y su límite

### Requirement: Dirección única de dependencias

Las dependencias entre crates SHALL fluir en un solo sentido a lo largo del pipeline. `zirk-diagnostics` es la única dependencia transversal permitida.

#### Scenario: Dependencia hacia atrás
- **WHEN** un crate de una etapa temprana declara dependencia sobre un crate de una etapa posterior
- **THEN** se considera una violación de esta especificación

#### Scenario: Dependencia sobre diagnósticos
- **WHEN** cualquier crate del pipeline declara dependencia sobre `zirk-diagnostics`
- **THEN** es válido, independientemente de su posición en el pipeline

### Requirement: Independencia del runtime respecto del compilador

`zirk-runtime` SHALL compilarse como `staticlib` y NO SHALL ser dependencia de ningún crate del compilador. Se enlaza en los binarios que Zirk produce, no en el compilador mismo.

#### Scenario: Artefacto producido
- **WHEN** se construye `zirk-runtime`
- **THEN** produce una biblioteca estática enlazable (`.a` en Unix, `.lib` en Windows)

#### Scenario: Frontera ABI C
- **WHEN** `zirk-runtime` expone un símbolo destinado al código generado
- **THEN** el símbolo SHALL declararse `extern "C"` con nombre estable y sin mangling

#### Scenario: Ciclo de vida de la aplicación
- **WHEN** se inspecciona la superficie pública de `zirk-runtime`
- **THEN** expone `zirk_rt_init` y `zirk_rt_shutdown`, correspondientes al ciclo de vida de `ZIRK_RUNTIME_SPEC.md` sección 2

### Requirement: Cobertura del pipeline por crate

Cada etapa del pipeline de `ZIRK_COMPILER_SPEC.md` sección 2 SHALL implementarse en el crate que le corresponde, sin que una etapa asuma responsabilidades de otra.

#### Scenario: El lexer no conoce la gramática
- **WHEN** se inspecciona `zirk-lexer`
- **THEN** produce tokens
- **AND** NO decide si una secuencia de tokens es válida

#### Scenario: El parser no chequea tipos
- **WHEN** se parsea una expresión con tipos incompatibles pero sintaxis correcta
- **THEN** el parser produce el árbol sin error
- **AND** el error de tipos lo emite el chequeador

#### Scenario: El backend es el único que conoce LLVM
- **WHEN** se inspeccionan los crates del workspace
- **THEN** solo `zirk-codegen-llvm` depende de `inkwell`

#### Scenario: El enlace no ocurre en el backend
- **WHEN** se produce un ejecutable
- **THEN** el backend emite el objeto
- **AND** la invocación del linker ocurre en la CLI
