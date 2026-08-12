## ADDED Requirements

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

### Requirement: Ausencia de sintaxis de Zirk en esta fase

El workspace de esta fase NO SHALL implementar análisis léxico, sintáctico ni semántico de código Zirk.

#### Scenario: Intento de compilar un archivo fuente
- **WHEN** se busca funcionalidad para procesar un archivo `.zrk`
- **THEN** no existe en el workspace de esta fase
