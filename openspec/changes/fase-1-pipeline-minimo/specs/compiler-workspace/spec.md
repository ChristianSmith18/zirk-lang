## REMOVED Requirements

### Requirement: Ausencia de sintaxis de Zirk en esta fase

**Reason**: Era una restricción propia de la Fase 0, cuyo objetivo era montar los cimientos sin implementar lenguaje. Esta fase implementa precisamente el análisis léxico, sintáctico y semántico que aquel requisito prohibía.

**Migration**: Se reemplaza por el requisito *Cobertura del pipeline por crate*, que fija qué etapa vive en cada crate en vez de exigir que estén vacíos.

## ADDED Requirements

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
